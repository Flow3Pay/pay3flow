//! Acquirer discovery for Pay3Flow.
//!
//! Runs a battery of web searches against the configured SearXNG backend,
//! normalizes the hits (URL -> registrable domain -> slug, name from title),
//! de-duplicates them, and compares the result against what the backend
//! database already stores. The backend DB is read DIRECTLY (env
//! `ACQUIRER_DB_DSN`), so the diff (added / updated / deleted) is computed
//! here and returned to the backend over gRPC — the backend only applies it.
//!
//! The module is self-contained on purpose: it never touches `AppState`
//! beyond reading the existing `searxng` client. The DB pool is lazily built
//! once from the environment and kept in a `OnceLock`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use deadpool_postgres::{Config as PoolCfg, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

use crate::state::AppState;
use crw_search::{SearxngClient, SearxngParams};
use futures::StreamExt;

/// Env var holding the backend Postgres DSN. When unset/parse-failing the scan
/// still runs: everything is reported as `added` and nothing is ever deleted.
const DB_ENV: &str = "ACQUIRER_DB_DSN";
/// The scan must clear this many unique acquirers before `deleted` is emitted,
/// so a failed/empty search round can never wipe the acquirer table.
const DELETE_MIN_FOUND_DEFAULT: usize = 20;

static POOL: OnceLock<Option<Pool>> = OnceLock::new();

fn pool() -> Option<&'static Pool> {
    POOL.get_or_init(|| {
        let url = std::env::var(DB_ENV).ok()?;
        let mut cfg = PoolCfg::new();
        cfg.url = Some(url);
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        cfg.create_pool(Some(Runtime::Tokio1), NoTls).ok()
    })
    .as_ref()
}

/// A single normalized acquirer candidate produced by an internet scan.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredAcquirer {
    pub slug: String,
    pub name: String,
    pub geo: String,
    pub currencies: String,
    pub fee_percent: f64,
    pub fee_fixed: i64,
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    pub amount_currency: Option<String>,
    pub website_url: String,
    pub api_docs_url: String,
    pub description: String,
    pub source: String,
    pub api_json: String,
}

/// Outcome of one scan: the add/update/delete diff against the backend DB.
#[derive(Debug, Clone, Default)]
pub struct DiscoverResult {
    pub added: Vec<DiscoveredAcquirer>,
    pub updated: Vec<DiscoveredAcquirer>,
    pub deleted: Vec<String>,
    pub found: usize,
    pub scanned_at: String,
    pub search_disabled: bool,
    /// Set when the DB could not be reached; the scan degrades to "all adds".
    pub error: Option<String>,
}

/// Search battery used to surface acquirer / payment-processor candidates.
/// Each query hits the search backend concurrently (bounded below), so a full
/// scan finishes in a handful of round-trips (~seconds), well under the
/// one-minute budget the loop runs on.
const QUERIES: &[&str] = &[
    "payment gateway api documentation",
    "card acquiring bank developer portal",
    "payment processor rest api integration",
    "online payment acquirer api reference",
    "merchant acquiring solution provider",
    "payment service provider acquiring network",
    "bank card acquiring api",
    "payment gateway developer docs",
    "acquiring bank payment processing api",
    "international payment gateway api",
    "cross border payments api provider",
    "open banking payment provider api",
    "ecommerce payment processor developer",
    "payment gateway companies list",
    "merchant acquirer bank list",
    "psp payment gateway api",
    "card not present acquiring provider",
    "digital wallet payment api provider",
    "recurring billing payment api",
    "payout api payment provider",
    "local payments acquiring network",
    "pos acquiring terminal api provider",
    "alternative payment methods acquiring",
    "bank transfer payment api gateway",
];

/// Hostname substrings that never represent an acquirer (engines, social,
/// aggregators, review sites, encyclopedias). Matched case-insensitively.
const JUNK_HOSTS: &[&str] = &[
    "google", "bing", "duckduckgo", "yandex", "brave.", "yahoo.",
    "github", "stackoverflow", "stackexchange", "gitlab",
    "linkedin", "facebook", "twitter", "instagram", "youtube", "reddit",
    "medium.com", "substack", "quora", "tiktok", "x.com",
    "g2.com", "capterra", "trustradius", "getapp", "glassdoor", "crunchbase",
    "zoominfo", "wikipedia", "investopedia", "coursera", "udemy",
];

/// Registrable domains that should map to a canonical acquirer slug.
const SLUG_ALIASES: &[(&str, &str)] = &[
    ("braintreepayments.com", "braintree"),
    ("checkout.com", "checkout"),
    ("paypal.com", "paypal"),
    ("stripe.com", "stripe"),
    ("adyen.com", "adyen"),
    ("klarna.com", "klarna"),
    ("payoneer.com", "payoneer"),
    ("wise.com", "wise"),
    ("mollie.com", "mollie"),
];

/// Two-part public suffixes; the registrable domain then has three labels.
const TWO_PART_SUFFIXES: &[&str] = &[
    "co.uk", "org.uk", "com.au", "net.au", "co.nz", "co.jp", "co.in", "com.br",
    "com.cn", "com.tw", "co.kr", "com.mx", "com.sg", "co.za", "com.tr", "co.il",
    "com.pl", "com.ua", "com.ar", "com.pe", "com.co", "co.th", "com.vn",
    "com.eg", "com.sa",
];

/// Subdomain prefixes that identify a docs / developer entry so a docs URL can
/// be peeled off during the registrable-domain computation.
const SUBDOMAIN_PREFIXES: &[&str] = &[
    "www", "m", "api", "docs", "dev", "developer", "developers", "app",
];

/// Words that end an acquirer brand name pulled from a search-result title.
const NAME_STOP: &[&str] = &[
    "docs", "documentation", "developer", "developers", "api", "reference",
    "gateway", "payment", "payments", "processing", "processor", "merchant",
    "acquiring", "software", "solutions", "solution", "help", "support",
    "home", "homepage", "site", "official", "landing", "login", "sign",
    "register", "services", "service", "platform", "blog", "news", "contact",
    "about", "connect", "pages",
];

/// Run the full scan and produce the diff against the backend DB.
pub async fn discover(state: &AppState, target_count: u32) -> DiscoverResult {
    let scanned_at = chrono::Utc::now().to_rfc3339();
    let Some(client) = state.searxng.clone() else {
        return DiscoverResult {
            search_disabled: true,
            scanned_at,
            ..Default::default()
        };
    };

    let target = if target_count == 0 { 500 } else { target_count as usize };

// All queries in flight at once, capped so a slow/rate-limited SearXNG
    // cannot stall the whole scan (per-query timeout is enforced by the client).
    // Each task owns its query string + an Arc<client>, making every future
    // `'static` so `buffered` can poll them freely.
    let queries: Vec<String> = QUERIES.iter().map(|q| q.to_string()).collect();
    let pools: Vec<Vec<DiscoveredAcquirer>> =
        futures::stream::iter(queries)
            .map(|q| {
                let client = client.clone();
                async move { run_query(&client, &q).await }
            })
            .buffered(6)
            .collect()
            .await;

    // Dedup by slug; prefer a hit that carries a docs URL over a plain landing
    // page that got to the slug first.
    let mut by_slug: HashMap<String, DiscoveredAcquirer> = HashMap::new();
    for hit in pools.into_iter().flatten() {
        let slot = by_slug.entry(hit.slug.clone()).or_insert_with(|| hit.clone());
        if slot.api_docs_url.is_empty() && !hit.api_docs_url.is_empty() {
            *slot = hit;
        }
    }

    let mut found: Vec<DiscoveredAcquirer> = by_slug.into_values().collect();
    found.sort_by(|a, b| {
        // Docs-bearing rows first, then stable slug order.
        b.api_docs_url
            .is_empty()
            .cmp(&a.api_docs_url.is_empty())
            .then_with(|| a.slug.cmp(&b.slug))
    });
    found.truncate(target);

    let mut result = DiscoverResult {
        found: found.len(),
        scanned_at,
        ..Default::default()
    };

    match load_existing().await {
        Ok(existing) => {
            let (added, updated, deleted) = compute_diff(&found, &existing, delete_min_found());
            result.added = added;
            result.updated = updated;
            result.deleted = deleted;
        }
        Err(e) => {
            // DB unreachable: never destroy data, just report everything as new
            // (the backend upsert is `ON CONFLICT DO UPDATE`, so re-inserting an
            // existing slug is harmless).
            result.error = Some(e);
            result.added = found;
        }
    }

    result
}

async fn run_query(client: &SearxngClient, query: &str) -> Vec<DiscoveredAcquirer> {
    let params = SearxngParams {
        q: query.to_string(),
        categories: None,
        language: Some("en".into()),
        time_range: None,
        engines: None,
        pageno: None,
        safesearch: Some(1),
        paid_rescue: false,
    };
    match client.fetch(&params).await {
        Ok(resp) => resp
            .results
            .iter()
            .filter_map(|r| {
                let url = r.url.as_deref()?;
                let title = r.title.as_deref()?;
                let content = r.content.as_deref().unwrap_or_default();
                normalize_hit(url, title, content, query)
            })
            .collect(),
        Err(e) => {
            tracing::debug!(error = %e, query, "acquirer discovery query failed");
            Vec::new()
        }
    }
}

fn normalize_hit(url: &str, title: &str, content: &str, query: &str) -> Option<DiscoveredAcquirer> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if JUNK_HOSTS.iter().any(|j| host.contains(*j)) {
        return None;
    }
    let reg = registrable_domain(&host);
    if reg.is_empty() {
        return None;
    }
    let slug = slug_for(&reg);
    if slug.is_empty() {
        return None;
    }
    let path = &parsed.path().to_ascii_lowercase();
    let docs_url = if is_docs_url(&host, path) {
        url.to_string()
    } else {
        String::new()
    };

    Some(DiscoveredAcquirer {
        slug: slug.clone(),
        name: derive_name(title, &slug),
        geo: String::new(),
        currencies: String::new(),
        fee_percent: 0.0,
        fee_fixed: 0,
        min_amount: None,
        max_amount: None,
        amount_currency: None,
        website_url: format!("https://{reg}"),
        api_docs_url: docs_url,
        description: truncate(content, 220),
        source: query.to_string(),
        api_json: String::new(),
    })
}

/// Peel `www`/`docs`/`developers`... prefixes and scheme off a host, then
/// return the registrable domain (two or three labels depending on the
/// suffix). Empty when the host is too short to be meaningful.
fn registrable_domain(host: &str) -> String {
    let mut labels: Vec<&str> = host.split('.').collect();
    while labels.len() > 1 && SUBDOMAIN_PREFIXES.contains(&labels[0]) {
        labels.remove(0);
    }
    if labels.len() < 2 {
        return String::new();
    }
    let last_two = labels[labels.len() - 2..].join(".");
    if TWO_PART_SUFFIXES.contains(&last_two.as_str()) && labels.len() >= 3 {
        labels[labels.len() - 3..].join(".")
    } else {
        last_two
    }
}

fn slug_for(reg: &str) -> String {
    for (host, alias) in SLUG_ALIASES {
        if reg == *host || reg.ends_with(&format!(".{host}")) {
            return alias.to_string();
        }
    }
    reg.split('.').next().unwrap_or(reg).to_string()
}

/// Brand name from a search title: cut at the first separator, drop trailing
/// stopwords, cap at 4 tokens, fall back to the title-cased slug.
fn derive_name(title: &str, slug: &str) -> String {
    let trimmed = title.trim();
    let head = trimmed
        .split(['|', '–', '—', '®', '™'])
        .next()
        .unwrap_or(trimmed)
        .split(['-', ':'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    let mut words: Vec<&str> = Vec::new();
    for w in head.split_whitespace() {
        let cleaned = w.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.is_empty() {
            continue;
        }
        if NAME_STOP.contains(&cleaned.to_ascii_lowercase().as_str()) {
            break;
        }
        words.push(w);
        if words.len() >= 4 {
            break;
        }
    }
    if words.is_empty() {
        return slug_title(slug);
    }
    words.join(" ")
}

fn slug_title(slug: &str) -> String {
    let mut chars = slug.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => slug.to_string(),
    }
}

fn is_docs_url(host: &str, path: &str) -> bool {
    host.starts_with("docs.")
        || host.starts_with("developers.")
        || host.starts_with("developer.")
        || path.split(['/', '.']).any(|seg| {
            matches!(
                seg,
                "docs"
                    | "documentation"
                    | "developers"
                    | "developer"
                    | "api"
                    | "apireference"
                    | "apidocs"
                    | "reference"
                    | "integrations"
                    | "developer-center"
                    | "developercenter"
            )
        })
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.trim().chars().collect();
    if chars.len() <= max {
        return chars.into_iter().collect();
    }
    let mut cut: Vec<char> = chars[..max].to_vec();
    cut.truncate(max.saturating_sub(3));
    let mut out: String = cut.into_iter().collect();
    out.push_str("...");
    out
}

// --- DB diff ---

struct ExistingAcquirer {
    slug: String,
    name: String,
    website_url: String,
    api_docs_url: String,
    description: String,
    fee_percent: f64,
    source: String,
}

async fn load_existing() -> Result<Vec<ExistingAcquirer>, String> {
    let p = pool().ok_or_else(|| format!("{DB_ENV} unset or unparseable"))?;
    let client = p.get().await.map_err(|e| e.to_string())?;
    let rows = client
        .query(
            "SELECT slug, name, website_url, api_docs_url, description, fee_percent, source FROM acquirers",
            &[],
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| ExistingAcquirer {
            slug: r.get(0),
            name: r.get(1),
            website_url: r.get(2),
            api_docs_url: r.get(3),
            description: r.get(4),
            fee_percent: r.get(5),
            source: r.get(6),
        })
        .collect())
}

fn compute_diff(
    found: &[DiscoveredAcquirer],
    existing: &[ExistingAcquirer],
    delete_min: usize,
) -> (Vec<DiscoveredAcquirer>, Vec<DiscoveredAcquirer>, Vec<String>) {
    let existing_map: HashMap<&str, &ExistingAcquirer> =
        existing.iter().map(|e| (e.slug.as_str(), e)).collect();

    let mut added = Vec::new();
    let mut updated = Vec::new();
    for a in found {
        match existing_map.get(a.slug.as_str()) {
            None => added.push(a.clone()),
            Some(e) if is_changed(a, e) => updated.push(a.clone()),
            Some(_) => {}
        }
    }

    let found_slugs: HashSet<&str> = found.iter().map(|a| a.slug.as_str()).collect();
    let mut deleted = Vec::new();
    if found.len() >= delete_min {
        for e in existing {
            if !found_slugs.contains(e.slug.as_str()) {
                deleted.push(e.slug.clone());
            }
        }
    }

    (added, updated, deleted)
}

/// A found acquirer counts as "updated" when it carries something the stored
/// row does not yet have, or its name should not override a curated one. Fee
/// fields only count when crw actually knows a nonzero fee (so a scan that
/// finds no pricing data never clobbers curated fees with 0).
fn is_changed(a: &DiscoveredAcquirer, e: &ExistingAcquirer) -> bool {
    let name_differs = a.name != e.name && (e.source.is_empty() || e.source == "discovery");
    let fills_gap = (e.website_url.is_empty() && !a.website_url.is_empty())
        || (e.api_docs_url.is_empty() && !a.api_docs_url.is_empty())
        || (e.description.is_empty() && !a.description.is_empty());
    let fee_improves = e.fee_percent == 0.0 && a.fee_percent > 0.0;
    name_differs || fills_gap || fee_improves
}

fn delete_min_found() -> usize {
    std::env::var("ACQUIRER_DELETE_MIN_FOUND")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DELETE_MIN_FOUND_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(url: &str, title: &str) -> DiscoveredAcquirer {
        normalize_hit(url, title, "some description", "test query").unwrap()
    }

    #[test]
    fn slug_from_plain_domain() {
        let a = hit("https://stripe.com/payments", "Stripe | Payments infrastructure");
        assert_eq!(a.slug, "stripe");
        assert_eq!(a.website_url, "https://stripe.com");
        assert_eq!(a.api_docs_url, "");
    }

    #[test]
    fn docs_suffix_detected_on_subdomain_and_path() {
        let a = hit("https://docs.stripe.com/api", "Stripe API Reference");
        assert_eq!(a.slug, "stripe");
        assert_eq!(a.api_docs_url, "https://docs.stripe.com/api");
    }

    #[test]
    fn alias_collapses_braintree_domain() {
        let a = hit(
            "https://www.braintreepayments.com/developers",
            "Braintree Payments | Developer Docs",
        );
        assert_eq!(a.slug, "braintree");
        assert!(a.api_docs_url.starts_with("https://www.braintreepayments.com/"));
    }

    #[test]
    fn junk_hosts_are_dropped() {
        assert!(normalize_hit(
            "https://en.wikipedia.org/wiki/Stripe_Inc.",
            "Stripe Inc",
            "",
            "q"
        )
        .is_none());
        assert!(normalize_hit(
            "https://www.google.com/search",
            "payments",
            "",
            "q"
        )
        .is_none());
    }

    #[test]
    fn name_drops_docs_stopwords() {
        let a = hit("https://adyen.com/api", "Adyen API Reference");
        assert_eq!(a.name, "Adyen");
    }

    #[test]
    fn name_falls_back_to_slug_title() {
        let a = hit("https://khipu.com", "Khipu");
        assert_eq!(a.name, "Khipu");
    }

    #[test]
    fn two_part_suffix_keeps_three_labels() {
        assert_eq!(registrable_domain("www.idealo.co.uk"), "idealo.co.uk");
        assert_eq!(slug_for(&registrable_domain("www.idealo.co.uk")), "idealo");
    }

    #[test]
    fn diff_reports_add_keep_and_delete() {
        let found = vec![
            DiscoveredAcquirer {
                slug: "stripe".into(),
                website_url: "https://stripe.com".into(),
                ..Default::default()
            },
            DiscoveredAcquirer {
                slug: "neatpay".into(),
                website_url: "https://neatpay.com".into(),
                ..Default::default()
            },
        ];
        let existing = vec![
            ExistingAcquirer {
                slug: "stripe".into(),
                name: "Stripe".into(),
                website_url: "https://stripe.com".into(),
                api_docs_url: String::new(),
                description: String::new(),
                fee_percent: 0.0,
                source: String::new(),
            },
            ExistingAcquirer {
                slug: "deadbeef".into(),
                name: "Dead Beep".into(),
                website_url: String::new(),
                api_docs_url: String::new(),
                description: String::new(),
                fee_percent: 0.0,
                source: "old query".into(),
            },
        ];
        let (added, updated, deleted) = compute_diff(&found, &existing, 0);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].slug, "neatpay");
        // stripe stored without a docs URL but found now has one => updated.
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].slug, "stripe");
        assert_eq!(deleted, vec!["deadbeef".to_string()]);
    }

    #[test]
    fn delete_guard_suppresses_when_scan_is_thin() {
        let found: Vec<DiscoveredAcquirer> = (0..5)
            .map(|i| DiscoveredAcquirer {
                slug: format!("p{i}"),
                website_url: format!("https://p{i}.com"),
                ..Default::default()
            })
            .collect();
        let existing = vec![ExistingAcquirer {
            slug: "gone".into(),
            name: String::new(),
            website_url: String::new(),
            api_docs_url: String::new(),
            description: String::new(),
            fee_percent: 0.0,
            source: "old".into(),
        }];
        // Requirement is met => emit deletions.
        let (_added, _updated, deleted) = compute_diff(&found, &existing, 6);
        assert!(deleted.is_empty());
        // Below the required minimum => never wipe anything.
        let (_added, _updated, deleted) = compute_diff(&found, &existing, 0);
        assert_eq!(deleted, vec!["gone".to_string()]);
    }
}
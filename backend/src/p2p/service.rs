use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Semaphore};

use crate::activitypub::Service as ActivityPubService;
use crate::compiled_provider_code::bestchange::BestChangeSource;
use crate::compiled_provider_code::papa_change::PapaChangeSource;
use crate::compiled_provider_code::skylabs::SkyLabsSource;
use crate::config::Config;
use crate::db::DbPool;
use crate::networks::NetworkCatalog;
use crate::p2p::declarative::{DeclarativeMarketSource, DeclarativeP2pSource};
use crate::p2p::latency::ServiceLatencyTracker;
pub(crate) use crate::p2p::models::P2pOfferMarket;
pub(crate) use crate::p2p::models::{
    Advertiser, FiatRouteQuote, P2pOffer, P2pSearchQuery, P2pSearchResponse, P2pSide, SourceStatus,
};
use crate::p2p::spot::{CryptoMarketSource, CryptoTicker};
use crate::p2p::workflow::WorkflowP2pSource;
use crate::route_engine::{Asset, PublicRouteProvider, PublicRouteQuote};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

impl P2pSearchQuery {
    fn normalize(mut self) -> Result<Self> {
        self.fiat = normalized_code(&self.fiat, "fiat")?;
        self.asset = normalized_code(&self.asset, "asset")?;
        if self
            .amount
            .is_some_and(|amount| !amount.is_finite() || amount <= 0.0)
        {
            bail!("amount must be a positive finite number");
        }
        if self
            .min_completion_rate
            .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
        {
            bail!("min_completion_rate must be between 0 and 1");
        }
        self.payment_method = self
            .payment_method
            .map(|method| method.trim().to_string())
            .filter(|method| !method.is_empty());
        self.sources = normalize_sources(self.sources)?;
        self.limit = Some(self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT));
        Ok(self)
    }

    pub(crate) fn fetch_limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_LIMIT)
            .saturating_mul(3)
            .clamp(20, 100)
    }
}

fn normalized_code(value: &str, field: &str) -> Result<String> {
    let code = value.trim().to_ascii_uppercase();
    if !(2..=12).contains(&code.len()) || !code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        bail!("{field} must be a 2-12 character alphanumeric code");
    }
    Ok(code)
}

pub(crate) fn normalize_sources(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut sources = value
        .split(',')
        .map(|source| source.trim().to_ascii_lowercase())
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>();
    if sources.is_empty() {
        bail!("sources must contain at least one source");
    }
    if sources.iter().any(|source| {
        !(1..=64).contains(&source.len())
            || !source
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    }) {
        bail!("sources must contain only valid 1-64 character provider slugs");
    }
    sources.sort_unstable();
    sources.dedup();
    Ok(Some(sources.join(",")))
}

impl P2pOffer {
    pub(crate) fn with_market(mut self, market: P2pOfferMarket) -> Self {
        self.market = market;
        self
    }

    /// Extract normalized P2P offers from the candidate-list shapes returned
    /// by Fmatch. Fmatch may return the offer directly or wrap it in an
    /// `offer`, `p2pOffer`, or `value` property depending on its response mode.
    /// A missing market filter preserves the market advertised by each offer.
    pub(crate) fn from_fmatch_reply(reply: &Value, market: Option<P2pOfferMarket>) -> Vec<Self> {
        let values = reply
            .get("offers")
            .or_else(|| reply.get("candidates"))
            .or_else(|| reply.get("object").and_then(|object| object.get("offers")))
            .or_else(|| {
                reply
                    .get("object")
                    .and_then(|object| object.get("candidates"))
            })
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        values
            .into_iter()
            .filter_map(|raw| {
                let offer = raw
                    .get("p2pOffer")
                    .or_else(|| raw.get("offer"))
                    .or_else(|| raw.get("value"))
                    .or_else(|| raw.get("raw"))
                    .or_else(|| {
                        raw.get("attachment")
                            .and_then(Value::as_array)
                            .and_then(|attachments| attachments.first())
                            .and_then(|attachment| attachment.get("value"))
                    })
                    .unwrap_or(&raw);
                serde_json::from_value::<P2pOffer>(offer.clone())
                    .ok()
                    .map(|offer| match market {
                        Some(market) => offer.with_market(market),
                        None => offer,
                    })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaymentMethodMatch {
    Exact,
    Unknown,
    No,
}

impl P2pOffer {
    fn price_number(&self) -> Option<f64> {
        self.price.parse().ok()
    }

    fn covers_amount(&self, amount: f64) -> bool {
        let min = self.min_fiat.parse::<f64>().unwrap_or(f64::INFINITY);
        let max = self.max_fiat.parse::<f64>().unwrap_or(f64::NEG_INFINITY);
        min <= amount && amount <= max
    }

    pub(crate) fn payment_method_match(&self, requested: &str) -> PaymentMethodMatch {
        let requested = canonical_payment_method(requested);
        let is_settlement_mode = matches!(requested.as_str(), "cash" | "noncash");
        let has_settlement_mode = self
            .payment_methods
            .iter()
            .any(|method| is_settlement_mode_label(method));

        if is_settlement_mode && has_settlement_mode {
            return if self
                .payment_methods
                .iter()
                .any(|method| canonical_payment_method(method) == requested)
            {
                PaymentMethodMatch::Exact
            } else {
                PaymentMethodMatch::No
            };
        }

        if self.payment_methods.iter().any(|method| {
            let method = canonical_payment_method(method);
            !method.is_empty() && (method.contains(&requested) || requested.contains(&method))
        }) {
            return PaymentMethodMatch::Exact;
        }

        // Some public venue responses (notably Bybit) expose only internal
        // numeric payment IDs. Keep these offers as unverified estimates rather
        // than incorrectly claiming that the requested bank is unavailable.
        if self.payment_methods.is_empty()
            || self.payment_methods.iter().all(|method| {
                method.bytes().all(|byte| byte.is_ascii_digit()) || is_settlement_mode_label(method)
            })
        {
            return PaymentMethodMatch::Unknown;
        }

        PaymentMethodMatch::No
    }

    fn matches(&self, query: &P2pSearchQuery) -> bool {
        if !self.fiat.eq_ignore_ascii_case(&query.fiat)
            || !self.asset.eq_ignore_ascii_case(&query.asset)
            || self.side != query.side
            || query.sources.as_deref().is_some_and(|requested| {
                !requested
                    .split(',')
                    .any(|source| self.source.eq_ignore_ascii_case(source))
            })
        {
            return false;
        }
        if query
            .amount
            .is_some_and(|amount| !self.covers_amount(amount))
        {
            return false;
        }
        if query.merchant_only.unwrap_or(false) && !self.advertiser.is_merchant {
            return false;
        }
        if query
            .min_orders
            .zip(self.advertiser.completed_orders_30d)
            .is_some_and(|(minimum, actual)| actual < minimum)
        {
            return false;
        }
        if query
            .min_completion_rate
            .zip(self.advertiser.completion_rate_30d)
            .is_some_and(|(minimum, actual)| actual < minimum)
        {
            return false;
        }
        if let Some(payment_method) = &query.payment_method {
            if self.payment_method_match(payment_method) == PaymentMethodMatch::No {
                return false;
            }
        }
        true
    }
}

fn is_settlement_mode_label(value: &str) -> bool {
    matches!(canonical_payment_method(value).as_str(), "cash" | "noncash")
}

fn canonical_payment_method(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn canonicalize_offer_payment_methods(
    offer: &mut P2pOffer,
    aliases: &BTreeMap<String, Vec<String>>,
) {
    for method in &mut offer.payment_methods {
        let normalized = canonical_payment_method(method);
        if let Some(canonical) = aliases.iter().find_map(|(canonical, variants)| {
            let matches_canonical = canonical_payment_method(canonical) == normalized;
            let matches_alias = variants
                .iter()
                .any(|alias| canonical_payment_method(alias) == normalized);
            (matches_canonical || matches_alias).then_some(canonical)
        }) {
            method.clone_from(canonical);
        }
    }
    offer.payment_methods.sort();
    offer.payment_methods.dedup();
}

#[async_trait]
pub(crate) trait P2pSource: Send + Sync {
    fn name(&self) -> &str;
    fn market(&self) -> P2pOfferMarket {
        P2pOfferMarket::P2p
    }
    fn timeout(&self, default: Duration) -> Duration {
        default
    }
    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>>;
}

#[async_trait]
pub trait PublicFiatRouteProvider: Send + Sync {
    fn name(&self) -> &str;
    fn supports_pair(&self, source_currency: &str, target_currency: &str) -> bool;
    async fn quote(
        &self,
        source_currency: &str,
        target_currency: &str,
        source_amount: f64,
    ) -> Result<FiatRouteQuote>;
}

#[derive(Clone)]
pub struct P2pSearchService {
    enabled: bool,
    timeout: Duration,
    cache_ttl: Duration,
    cache: Arc<RwLock<HashMap<String, CachedSearch>>>,
    sources: Arc<[Arc<dyn P2pSource>]>,
    payment_method_aliases: Arc<HashMap<String, BTreeMap<String, Vec<String>>>>,
    market_sources: Arc<[Arc<dyn CryptoMarketSource>]>,
    pub(crate) default_assets: Arc<[String]>,
    pub(crate) fiat_intermediaries: Arc<[String]>,
    pub(crate) networks: NetworkCatalog,
    pub(crate) route_providers: Arc<[Arc<dyn PublicRouteProvider>]>,
    pub(crate) fiat_route_providers: Arc<[Arc<dyn PublicFiatRouteProvider>]>,
    pub(crate) quote_semaphore: Arc<Semaphore>,
    pub(crate) provider_quote_cache: Arc<RwLock<HashMap<String, CachedProviderQuote>>>,
    pub(crate) fiat_quote_cache: Arc<RwLock<HashMap<String, CachedFiatQuote>>>,
    pub(crate) quote_refreshes: Arc<Mutex<HashSet<String>>>,
    pub(crate) provider_capabilities_cache: Arc<RwLock<Option<ProviderCapabilitiesSnapshot>>>,
    pub(crate) service_latencies: ServiceLatencyTracker,
    fmatch: Option<FmatchP2pBackend>,
}

#[derive(Clone)]
pub(crate) struct CachedProviderQuote {
    pub(crate) inserted_at: Instant,
    pub(crate) quote: PublicRouteQuote,
}

#[derive(Clone)]
pub(crate) struct CachedFiatQuote {
    pub(crate) inserted_at: Instant,
    pub(crate) quote: FiatRouteQuote,
}

pub(crate) type ProviderCapabilitiesSnapshot = HashMap<String, HashSet<Asset>>;

#[derive(Clone)]
struct FmatchP2pBackend {
    pool: DbPool,
    ap: ActivityPubService,
    stale_window: Duration,
    answer_ttl: Duration,
}

// Keep all route combinations, but avoid opening an unbounded number of
// external quote requests at the same time.
const MAX_CONCURRENT_PROVIDER_QUOTES: usize = 16;

#[derive(Clone)]
struct CachedSearch {
    inserted_at: Instant,
    response: P2pSearchResponse,
}

impl P2pSearchService {
    pub fn from_config(config: &Config, networks: NetworkCatalog) -> Result<Self> {
        Self::from_provider_records(config, networks, Vec::new(), Vec::new(), Vec::new())
    }

    pub async fn from_database(
        config: &Config,
        networks: NetworkCatalog,
        pool: &DbPool,
        route_providers: Vec<Arc<dyn PublicRouteProvider>>,
        fiat_route_providers: Vec<Arc<dyn PublicFiatRouteProvider>>,
    ) -> Result<Self> {
        let records = crate::providers::adapters(pool).await?;
        Self::from_provider_records(
            config,
            networks,
            records,
            route_providers,
            fiat_route_providers,
        )
    }

    pub async fn from_database_with_fmatch(
        config: &Config,
        networks: NetworkCatalog,
        pool: &DbPool,
        route_providers: Vec<Arc<dyn PublicRouteProvider>>,
        fiat_route_providers: Vec<Arc<dyn PublicFiatRouteProvider>>,
        ap: ActivityPubService,
    ) -> Result<Self> {
        let mut service = Self::from_database(
            config,
            networks,
            pool,
            route_providers,
            fiat_route_providers,
        )
        .await?;
        service.fmatch = Some(FmatchP2pBackend {
            pool: pool.clone(),
            ap,
            stale_window: Duration::from_secs(config.p2p_fmatch_stale_secs),
            answer_ttl: Duration::from_millis(config.p2p_search_cache_ttl_ms.max(1)),
        });
        service.warm_provider_capabilities();
        Ok(service)
    }

    fn from_provider_records(
        config: &Config,
        networks: NetworkCatalog,
        records: Vec<crate::providers::ProviderAdapterRecord>,
        mut route_providers: Vec<Arc<dyn PublicRouteProvider>>,
        fiat_route_providers: Vec<Arc<dyn PublicFiatRouteProvider>>,
    ) -> Result<Self> {
        if records.iter().any(|record| record.workflow.is_some()) {
            playwright_rs::server::driver::get_driver_executable()
                .map_err(|error| anyhow::anyhow!("Playwright driver is unavailable: {error}"))?;
            if let Some(executable) = config.playwright_chromium_executable.as_deref() {
                if !Path::new(&executable).is_file() {
                    bail!("PLAYWRIGHT_CHROMIUM_EXECUTABLE points to missing file `{executable}`");
                }
            }
        }
        let timeout = Duration::from_millis(config.p2p_search_timeout_ms.clamp(250, 30_000));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Pay3Flow-P2P-Search/0.1")
            .build()
            .context("failed to build P2P HTTP client")?;
        let mut sources: Vec<Arc<dyn P2pSource>> = Vec::new();
        let mut market_sources: Vec<Arc<dyn CryptoMarketSource>> = Vec::new();
        let payment_method_aliases = records
            .iter()
            .filter_map(|record| {
                let aliases = &record.config.as_ref()?.p2p.as_ref()?.payment_method_aliases;
                (!aliases.is_empty()).then(|| (record.slug.clone(), aliases.clone()))
            })
            .collect::<HashMap<_, _>>();
        for record in &records {
            if let Some(source) = BestChangeSource::from_record(client.clone(), record) {
                let source = Arc::new(source);
                sources.push(source.clone());
                route_providers.push(source);
            }
            if let Some(source) = PapaChangeSource::from_record(client.clone(), record) {
                sources.push(Arc::new(source));
            }
            if let Some(source) = SkyLabsSource::from_record(client.clone(), record) {
                sources.push(Arc::new(source));
            } else if let Some(source) = DeclarativeP2pSource::from_record(client.clone(), record) {
                sources.push(Arc::new(source));
            }
            if let Some(source) = DeclarativeMarketSource::from_record(client.clone(), record) {
                market_sources.push(Arc::new(source));
            }
            if let Some(source) = WorkflowP2pSource::from_record(
                record,
                config.playwright_chromium_executable.clone(),
                config.p2p_workflow_debug_screenshot.clone(),
            ) {
                sources.push(Arc::new(source));
            }
        }
        let default_assets = if config.p2p_search_assets.is_empty() {
            networks.assets()
        } else {
            config.p2p_search_assets.clone()
        };
        Ok(Self {
            enabled: config.p2p_search_enabled,
            timeout,
            cache_ttl: Duration::from_millis(config.p2p_search_cache_ttl_ms.min(60_000)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            sources: sources.into(),
            payment_method_aliases: Arc::new(payment_method_aliases),
            market_sources: market_sources.into(),
            default_assets: default_assets.into(),
            fiat_intermediaries: config.route_source_fiats.clone().into(),
            networks,
            route_providers: route_providers.into(),
            fiat_route_providers: fiat_route_providers.into(),
            quote_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROVIDER_QUOTES)),
            provider_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            fiat_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            quote_refreshes: Arc::new(Mutex::new(HashSet::new())),
            provider_capabilities_cache: Arc::new(RwLock::new(None)),
            service_latencies: ServiceLatencyTracker::default(),
            fmatch: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_sources(sources: Vec<Arc<dyn P2pSource>>, timeout: Duration) -> Self {
        Self {
            enabled: true,
            timeout,
            cache_ttl: Duration::ZERO,
            cache: Arc::new(RwLock::new(HashMap::new())),
            sources: sources.into(),
            payment_method_aliases: Arc::new(HashMap::new()),
            market_sources: Vec::new().into(),
            default_assets: vec![
                "USDT".into(),
                "USDC".into(),
                "BTC".into(),
                "ETH".into(),
                "BNB".into(),
                "SOL".into(),
                "TRX".into(),
                "TON".into(),
                "DOGE".into(),
                "LTC".into(),
                "DAI".into(),
                "FDUSD".into(),
                "XRP".into(),
                "ADA".into(),
                "DOT".into(),
                "LINK".into(),
                "AVAX".into(),
                "MATIC".into(),
                "BCH".into(),
                "NEAR".into(),
                "APT".into(),
                "ATOM".into(),
                "UNI".into(),
                "SUI".into(),
            ]
            .into(),
            fiat_intermediaries: vec!["AMD".into(), "RUB".into(), "USD".into()].into(),
            networks: NetworkCatalog::test_default(),
            route_providers: Vec::new().into(),
            fiat_route_providers: Vec::new().into(),
            quote_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROVIDER_QUOTES)),
            provider_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            fiat_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            quote_refreshes: Arc::new(Mutex::new(HashSet::new())),
            provider_capabilities_cache: Arc::new(RwLock::new(None)),
            service_latencies: ServiceLatencyTracker::default(),
            fmatch: None,
        }
    }

    #[cfg(test)]
    fn with_cache_ttl(mut self, cache_ttl: Duration) -> Self {
        self.cache_ttl = cache_ttl;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_route_providers(
        mut self,
        providers: Vec<Arc<dyn PublicRouteProvider>>,
    ) -> Self {
        self.route_providers = providers.into();
        self.provider_quote_cache = Arc::new(RwLock::new(HashMap::new()));
        self.quote_refreshes = Arc::new(Mutex::new(HashSet::new()));
        self.provider_capabilities_cache = Arc::new(RwLock::new(None));
        self
    }

    #[cfg(test)]
    pub(crate) fn with_fiat_route_providers(
        mut self,
        providers: Vec<Arc<dyn PublicFiatRouteProvider>>,
    ) -> Self {
        self.fiat_route_providers = providers.into();
        self.fiat_quote_cache = Arc::new(RwLock::new(HashMap::new()));
        self.quote_refreshes = Arc::new(Mutex::new(HashSet::new()));
        self
    }

    pub fn searchable_sources(&self) -> HashSet<String> {
        if !self.enabled {
            return HashSet::new();
        }

        self.sources
            .iter()
            .map(|source| source.name().to_string())
            .chain(
                self.market_sources
                    .iter()
                    .map(|source| source.name().to_string()),
            )
            .collect()
    }

    pub(crate) fn has_fmatch_backend(&self) -> bool {
        self.fmatch.is_some()
    }

    pub fn route_provider_names(&self) -> HashSet<String> {
        self.route_providers
            .iter()
            .map(|provider| provider.name().to_string())
            .chain(
                self.fiat_route_providers
                    .iter()
                    .map(|provider| provider.name().to_string()),
            )
            .collect()
    }

    pub async fn search(&self, query: P2pSearchQuery) -> Result<P2pSearchResponse> {
        self.run_search(query, None, None).await
    }

    pub(crate) async fn search_market(
        &self,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
    ) -> Result<P2pSearchResponse> {
        if self.fmatch.is_some() {
            if market.is_none() {
                return self.search_all_fmatch_markets(query, None).await;
            }
            return self.search_fmatch_market(query, market, None).await;
        }
        self.run_search(query, None, market).await
    }

    pub(crate) async fn stream_search_market(
        &self,
        query: P2pSearchQuery,
        updates: mpsc::Sender<P2pSearchResponse>,
        market: Option<P2pOfferMarket>,
    ) -> Result<P2pSearchResponse> {
        if self.fmatch.is_some() {
            if market.is_none() {
                return self.search_all_fmatch_markets(query, Some(&updates)).await;
            }
            return self
                .search_fmatch_market(query, market, Some(&updates))
                .await;
        }
        self.run_search(query, Some(updates), market).await
    }

    async fn search_all_fmatch_markets(
        &self,
        query: P2pSearchQuery,
        updates: Option<&mpsc::Sender<P2pSearchResponse>>,
    ) -> Result<P2pSearchResponse> {
        let (p2p, direct) = tokio::join!(
            self.search_fmatch_market(query.clone(), Some(P2pOfferMarket::P2p), None),
            self.search_fmatch_market(query.clone(), Some(P2pOfferMarket::DirectExchange), None,)
        );
        let response = match (p2p, direct) {
            (Ok(p2p), Ok(direct)) => merge_market_responses(query.normalize()?, p2p, direct),
            (Ok(response), Err(error)) | (Err(error), Ok(response)) => {
                tracing::warn!(%error, "one Fmatch market partition failed");
                response
            }
            (Err(error), Err(_)) => return Err(error),
        };
        if let Some(updates) = updates {
            let _ = updates.send(response.clone()).await;
        }
        Ok(response)
    }

    async fn search_fmatch_market(
        &self,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
        updates: Option<&mpsc::Sender<P2pSearchResponse>>,
    ) -> Result<P2pSearchResponse> {
        let started = Instant::now();
        let backend = self
            .fmatch
            .as_ref()
            .expect("Fmatch backend checked by caller")
            .clone();
        let query = query.normalize()?;
        let cache_key = fmatch_cache_key(&query, market)?;
        if let Some(mut response) = self.cached(&cache_key) {
            response.cached = true;
            response
                .sources
                .iter_mut()
                .for_each(|source| source.cached = true);
            if let Some(updates) = updates {
                let _ = updates.send(response.clone()).await;
            }
            return Ok(response);
        }
        let content = fmatch_p2p_content(&query, market);
        let candidate_page_size = query.fetch_limit().min(64);

        let response = match backend
            .ap
            .submit_p2p_request("candidates", &content, candidate_page_size)
            .await
        {
            Ok((_outcome, Some(reply))) => {
                let offers = P2pOffer::from_fmatch_reply(&reply, market);
                let mut response = build_search_response(
                    query.clone(),
                    &offers,
                    Vec::new(),
                    false,
                    "fmatch",
                    false,
                    Some(Utc::now()),
                );
                response.sources = source_statuses_from_offers(&response.offers);
                if usable_fmatch_response(&response) {
                    self.cache_response(cache_key.clone(), response.clone());
                    spawn_fmatch_answer_persist(
                        backend.pool.clone(),
                        cache_key.clone(),
                        response.clone(),
                        "fmatch",
                        backend.answer_ttl,
                    );
                    response
                } else {
                    self.fmatch_cache_or_provider(
                        &backend,
                        &cache_key,
                        query,
                        market,
                        updates,
                        "Fmatch returned no matching offers",
                    )
                    .await?
                }
            }
            Ok((_outcome, None)) => {
                self.fmatch_cache_or_provider(
                    &backend,
                    &cache_key,
                    query,
                    market,
                    updates,
                    "Fmatch returned no answer",
                )
                .await?
            }
            Err(error) => {
                tracing::warn!(%error, "Fmatch P2P search failed; trying cache and providers");
                self.fmatch_cache_or_provider(
                    &backend,
                    &cache_key,
                    query,
                    market,
                    updates,
                    &error.to_string(),
                )
                .await?
            }
        };
        if let Some(updates) = updates {
            let _ = updates.send(response.clone()).await;
        }
        tracing::info!(
            fiat = %response.query.fiat,
            asset = %response.query.asset,
            side = ?response.query.side,
            offers_found = response.offers.len(),
            source = %response.source,
            elapsed_ms = started.elapsed().as_millis(),
            "p2p.fmatch_search.completed"
        );
        Ok(response)
    }

    async fn fmatch_cache_or_provider(
        &self,
        backend: &FmatchP2pBackend,
        cache_key: &str,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
        updates: Option<&mpsc::Sender<P2pSearchResponse>>,
        reason: &str,
    ) -> Result<P2pSearchResponse> {
        if let Some(cached) = self
            .cached_fmatch_answer(backend, cache_key, query.clone(), market, reason)
            .await?
        {
            return Ok(cached);
        }
        tracing::warn!(reason, "Fmatch cache miss; using live P2P providers");
        let response = self.run_search(query, updates.cloned(), market).await?;
        Ok(mark_provider_fallback(response, reason))
    }

    async fn cached_fmatch_answer(
        &self,
        backend: &FmatchP2pBackend,
        cache_key: &str,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
        reason: &str,
    ) -> Result<Option<P2pSearchResponse>> {
        let max_age = backend.stale_window.as_secs().min(i64::MAX as u64) as i64;
        let Some(cached) =
            crate::db::repo::p2p_fmatch::latest_answer(&backend.pool, cache_key, max_age).await?
        else {
            return Ok(None);
        };
        let mut response: P2pSearchResponse =
            serde_json::from_value(cached.response).context("invalid cached Fmatch P2P answer")?;
        if !usable_fmatch_response(&response) {
            return Ok(None);
        }
        if let Some(market) = market {
            for offer in &mut response.offers {
                offer.market = market;
            }
        }
        response.query = query;
        response.cached = true;
        response.source = "database_cache".into();
        response.stale = true;
        response.observed_at = Some(cached.observed_at);
        response.sources = source_statuses_from_offers(&response.offers);
        tracing::warn!(
            reason,
            "serving stale cached P2P offers after Fmatch failure"
        );
        Ok(Some(response))
    }

    async fn run_search(
        &self,
        query: P2pSearchQuery,
        updates: Option<mpsc::Sender<P2pSearchResponse>>,
        market: Option<P2pOfferMarket>,
    ) -> Result<P2pSearchResponse> {
        let response = self
            .run_search_once(query.clone(), updates.clone(), market)
            .await?;
        if !response.offers.is_empty() {
            return Ok(response);
        }

        let mut fallback_query = query;
        if fallback_query.payment_method.is_some() {
            fallback_query.payment_method = None;
            let response = self
                .run_search_once(fallback_query.clone(), updates.clone(), market)
                .await?;
            if !response.offers.is_empty() {
                return Ok(response);
            }
        }
        if fallback_query.min_orders.is_some() || fallback_query.min_completion_rate.is_some() {
            fallback_query.min_orders = None;
            fallback_query.min_completion_rate = None;
            return self.run_search_once(fallback_query, updates, market).await;
        }
        Ok(response)
    }

    async fn run_search_once(
        &self,
        query: P2pSearchQuery,
        updates: Option<mpsc::Sender<P2pSearchResponse>>,
        market: Option<P2pOfferMarket>,
    ) -> Result<P2pSearchResponse> {
        if !self.enabled {
            bail!("P2P search is disabled");
        }
        let query = query.normalize()?;
        let cache_key = format!(
            "{market:?}:{}",
            serde_json::to_string(&query).context("failed to build P2P cache key")?
        );
        if let Some(mut response) = self.cached(&cache_key) {
            response.cached = true;
            if let Some(updates) = updates {
                let _ = updates.send(response.clone()).await;
            }
            return Ok(response);
        }
        let selected_sources = self
            .sources
            .iter()
            .filter(|source| {
                market.is_none_or(|market| source.market() == market)
                    && query.sources.as_deref().is_none_or(|requested| {
                        requested.split(',').any(|name| name == source.name())
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut searches = selected_sources
            .into_iter()
            .map(|source| {
                let query = query.clone();
                let aliases = self.payment_method_aliases.get(source.name()).cloned();
                async move {
                    let started = Instant::now();
                    let timeout = source.timeout(self.timeout);
                    let result = tokio::time::timeout(timeout, source.search(&query)).await;
                    let elapsed = started.elapsed().as_millis();
                    match result {
                        Ok(Ok(mut offers)) => {
                            if let Some(aliases) = &aliases {
                                for offer in &mut offers {
                                    canonicalize_offer_payment_methods(offer, aliases);
                                }
                            }
                            let count = offers.len();
                            (
                                offers,
                                SourceStatus {
                                    source: source.name().to_string(),
                                    ok: true,
                                    cached: false,
                                    latency_ms: elapsed,
                                    offers_found: count,
                                    error: None,
                                },
                            )
                        }
                        Ok(Err(error)) => (
                            Vec::new(),
                            SourceStatus {
                                source: source.name().to_string(),
                                ok: false,
                                cached: false,
                                latency_ms: elapsed,
                                offers_found: 0,
                                error: Some(error.to_string()),
                            },
                        ),
                        Err(_) => (
                            Vec::new(),
                            SourceStatus {
                                source: source.name().to_string(),
                                ok: false,
                                cached: false,
                                latency_ms: elapsed,
                                offers_found: 0,
                                error: Some(format!(
                                    "source timed out after {} ms",
                                    timeout.as_millis()
                                )),
                            },
                        ),
                    }
                }
            })
            .collect::<FuturesUnordered<_>>();

        let mut collected_offers = Vec::new();
        let mut sources = Vec::new();
        while let Some((offers, status)) = searches.next().await {
            collected_offers.extend(offers);
            sources.push(status);
            if let Some(updates) = &updates {
                let response = build_search_response(
                    query.clone(),
                    &collected_offers,
                    sources.clone(),
                    false,
                    "provider",
                    false,
                    Some(Utc::now()),
                );
                let _ = updates.send(response).await;
            }
        }

        let response = build_search_response(
            query.clone(),
            &collected_offers,
            sources,
            false,
            "provider",
            false,
            Some(Utc::now()),
        );
        if response.sources.iter().any(|source| source.ok) {
            self.cache_response(cache_key, response.clone());
            self.publish_p2p_offers(&response.offers);
        }
        Ok(response)
    }

    fn publish_p2p_offers(&self, offers: &[P2pOffer]) {
        let Some(backend) = self.fmatch.as_ref() else {
            return;
        };
        let ap = backend.ap.clone();
        let offers = offers.to_vec();
        tokio::spawn(async move {
            let results = join_all(
                offers
                    .chunks(64)
                    .map(|catalog| ap.publish_p2p_catalog(catalog)),
            )
            .await;
            let failures = results.iter().filter(|result| result.is_err()).count();
            if let Some(error) = results.iter().find_map(|result| result.as_ref().err()) {
                tracing::warn!(failures, %error, "P2P offer catalogs failed to publish to Fmatch");
            }
        });
    }

    fn cached(&self, key: &str) -> Option<P2pSearchResponse> {
        if self.cache_ttl.is_zero() {
            return None;
        }
        let cache = self.cache.read().ok()?;
        let cached = cache.get(key)?;
        (cached.inserted_at.elapsed() <= self.cache_ttl).then(|| cached.response.clone())
    }

    fn cache_response(&self, key: String, response: P2pSearchResponse) {
        if self.cache_ttl.is_zero() {
            return;
        }
        if let Ok(mut cache) = self.cache.write() {
            cache.retain(|_, cached| cached.inserted_at.elapsed() <= self.cache_ttl);
            cache.insert(
                key,
                CachedSearch {
                    inserted_at: Instant::now(),
                    response,
                },
            );
        }
    }

    pub(crate) async fn stream_market_tickers(
        &self,
        requested_sources: Option<&str>,
        updates: mpsc::Sender<(String, Result<Vec<CryptoTicker>>)>,
    ) {
        let mut searches = self
            .market_sources
            .iter()
            .filter(|source| {
                requested_sources
                    .is_none_or(|requested| requested.split(',').any(|name| name == source.name()))
            })
            .cloned()
            .map(|source| async move {
                let name = source.name().to_string();
                let result = tokio::time::timeout(source.timeout(self.timeout), source.tickers())
                    .await
                    .map_err(|_| anyhow::anyhow!("{} spot ticker request timed out", name))
                    .and_then(|result| result);
                (name, result)
            })
            .collect::<FuturesUnordered<_>>();

        while let Some(result) = searches.next().await {
            if updates.send(result).await.is_err() {
                return;
            }
        }
    }
}

fn build_search_response(
    query: P2pSearchQuery,
    collected_offers: &[P2pOffer],
    sources: Vec<SourceStatus>,
    cached: bool,
    source: &str,
    stale: bool,
    observed_at: Option<DateTime<Utc>>,
) -> P2pSearchResponse {
    let mut offers = collected_offers
        .iter()
        .filter(|offer| offer.matches(&query))
        .cloned()
        .collect::<Vec<_>>();
    sort_offers(&mut offers, query.side);
    truncate_offers_preserving_sources(&mut offers, query.limit.unwrap_or(DEFAULT_LIMIT));
    P2pSearchResponse {
        query,
        searched_at: Utc::now(),
        cached,
        offers,
        sources,
        source: source.into(),
        stale,
        observed_at,
    }
}

fn merge_market_responses(
    query: P2pSearchQuery,
    p2p: P2pSearchResponse,
    direct: P2pSearchResponse,
) -> P2pSearchResponse {
    let cached = p2p.cached || direct.cached;
    let stale = p2p.stale || direct.stale;
    let observed_at = p2p.observed_at.max(direct.observed_at);
    let source = if p2p.source == "provider_fallback" || direct.source == "provider_fallback" {
        "provider_fallback"
    } else if p2p.source == "database_cache" || direct.source == "database_cache" {
        "database_cache"
    } else if p2p.source == "fmatch" || direct.source == "fmatch" {
        "fmatch"
    } else {
        "provider"
    };
    let offers = p2p
        .offers
        .into_iter()
        .chain(direct.offers)
        .collect::<Vec<_>>();
    let sources = source_statuses_from_offers(&offers);
    build_search_response(query, &offers, sources, cached, source, stale, observed_at)
}

fn mark_provider_fallback(mut response: P2pSearchResponse, reason: &str) -> P2pSearchResponse {
    response.source = "provider_fallback".into();
    tracing::warn!(reason, "using live providers after Fmatch failure");
    response
}

fn source_statuses_from_offers(offers: &[P2pOffer]) -> Vec<SourceStatus> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for offer in offers {
        *counts.entry(offer.source.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(source, offers_found)| SourceStatus {
            source: source.to_string(),
            ok: true,
            cached: true,
            latency_ms: 0,
            offers_found,
            error: None,
        })
        .collect()
}

fn usable_fmatch_response(response: &P2pSearchResponse) -> bool {
    !response.offers.is_empty()
}

fn fmatch_cache_key(query: &P2pSearchQuery, market: Option<P2pOfferMarket>) -> Result<String> {
    let payload = serde_json::to_vec(&json!({
        "market": match market {
            Some(P2pOfferMarket::P2p) => "p2p",
            Some(P2pOfferMarket::DirectExchange) => "direct_exchange",
            None => "all",
        },
        "query": query,
    }))?;
    let digest = Sha256::digest(payload);
    Ok(format!(
        "p2p-fmatch:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn fmatch_p2p_content(query: &P2pSearchQuery, market: Option<P2pOfferMarket>) -> String {
    let market = match market {
        Some(P2pOfferMarket::P2p) => "p2p",
        Some(P2pOfferMarket::DirectExchange) => "direct_exchange",
        None => "all",
    };
    format!(
        "p2p route candidates; market={market}; fiat={}; asset={}; side={:?}; amount={:?}; payment_method={:?}; merchant_only={:?}; min_orders={:?}; min_completion_rate={:?}; limit={:?}; sources={:?}",
        query.fiat,
        query.asset,
        query.side,
        query.amount,
        query.payment_method,
        query.merchant_only,
        query.min_orders,
        query.min_completion_rate,
        query.limit,
        query.sources,
    )
}

async fn persist_fmatch_answer(
    pool: &DbPool,
    cache_key: &str,
    response: &P2pSearchResponse,
    source: &str,
    ttl: Duration,
) {
    let Some(observed_at) = response.observed_at else {
        return;
    };
    let Ok(value) = serde_json::to_value(response) else {
        tracing::warn!(cache_key, "failed to serialize Fmatch P2P answer");
        return;
    };
    let expires_at = observed_at
        + chrono::Duration::from_std(ttl).unwrap_or_else(|_| chrono::Duration::seconds(5));
    if let Err(error) = crate::db::repo::p2p_fmatch::save_answer(
        pool,
        cache_key,
        &value,
        source,
        observed_at,
        expires_at,
    )
    .await
    {
        tracing::warn!(%error, cache_key, "failed to persist Fmatch P2P answer");
    }
}

fn spawn_fmatch_answer_persist(
    pool: DbPool,
    cache_key: String,
    response: P2pSearchResponse,
    source: &'static str,
    ttl: Duration,
) {
    tokio::spawn(async move {
        persist_fmatch_answer(&pool, &cache_key, &response, source, ttl).await;
    });
}

fn truncate_offers_preserving_sources(offers: &mut Vec<P2pOffer>, limit: usize) {
    if offers.len() <= limit {
        return;
    }

    let reserved_indices = {
        let mut represented_sources = HashSet::new();
        offers
            .iter()
            .enumerate()
            .filter_map(|(index, offer)| {
                represented_sources
                    .insert(offer.source.as_str())
                    .then_some(index)
            })
            .take(limit)
            .collect::<HashSet<_>>()
    };
    let mut remaining = limit.saturating_sub(reserved_indices.len());
    let mut index = 0;
    offers.retain(|_| {
        let reserved = reserved_indices.contains(&index);
        index += 1;
        reserved
            || if remaining > 0 {
                remaining -= 1;
                true
            } else {
                false
            }
    });
}

fn sort_offers(offers: &mut [P2pOffer], side: P2pSide) {
    offers.sort_by(|left, right| {
        let price_order = left
            .price_number()
            .partial_cmp(&right.price_number())
            .unwrap_or(Ordering::Equal);
        let price_order = match side {
            P2pSide::BuyCrypto => price_order,
            P2pSide::SellCrypto => price_order.reverse(),
        };
        price_order
            .then_with(|| {
                right
                    .advertiser
                    .is_merchant
                    .cmp(&left.advertiser.is_merchant)
            })
            .then_with(|| {
                right
                    .advertiser
                    .completion_rate_30d
                    .partial_cmp(&left.advertiser.completion_rate_30d)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                right
                    .advertiser
                    .completed_orders_30d
                    .cmp(&left.advertiser.completed_orders_30d)
            })
    });
}

#[cfg(test)]
mod tests;

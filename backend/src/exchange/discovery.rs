use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::activitypub::model::AcquirerCandidate;
use crate::activitypub::Service as ActivityPubService;
use crate::core::redis::{self, RedisPool};
use crate::db::DbPool;
use crate::exchange::model::{ExchangeOrder, ExchangeSolver, NewExchangeSolver};
use crate::exchange::repo;
use crate::exchange::status::{OrderStatus, SolverStatus};

const CANDIDATE_CACHE_TTL_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverDiscoverySource {
    Fmatch,
    RedisCache,
    LocalRegistry,
}

impl SolverDiscoverySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fmatch => "fmatch",
            Self::RedisCache => "redis_cache",
            Self::LocalRegistry => "local_registry",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverCandidate {
    pub solver: ExchangeSolver,
    pub rank: u64,
    pub source: SolverDiscoverySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fmatch_candidate: Option<AcquirerCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverDiscoveryResult {
    pub order_id: Uuid,
    pub source: SolverDiscoverySource,
    pub candidates: Vec<SolverCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedCandidate {
    name: String,
    short_id: String,
    rank: u64,
    price: Option<f64>,
    quality: Option<f64>,
    raw: Value,
}

impl From<AcquirerCandidate> for CachedCandidate {
    fn from(candidate: AcquirerCandidate) -> Self {
        Self {
            name: candidate.name,
            short_id: candidate.short_id,
            rank: candidate.rank,
            price: candidate.price,
            quality: candidate.quality,
            raw: candidate.raw,
        }
    }
}

impl From<CachedCandidate> for AcquirerCandidate {
    fn from(candidate: CachedCandidate) -> Self {
        Self {
            name: candidate.name,
            short_id: candidate.short_id,
            rank: candidate.rank,
            price: candidate.price,
            quality: candidate.quality,
            raw: candidate.raw,
        }
    }
}

pub async fn discover_solvers_for_order(
    pool: &DbPool,
    ap: &ActivityPubService,
    redis_pool: Option<&RedisPool>,
    order: &ExchangeOrder,
) -> Result<SolverDiscoveryResult> {
    ensure_discovering(pool, order).await?;

    let cache_key = order_cache_key(order);
    if let Ok(candidates) = discover_via_fmatch(ap, order).await {
        cache_candidates(redis_pool, &cache_key, &candidates).await;
        let mapped =
            map_fmatch_candidates(pool, order, candidates, SolverDiscoverySource::Fmatch).await?;
        transition_to_quoting(pool, order).await?;
        return Ok(SolverDiscoveryResult {
            order_id: order.id,
            source: SolverDiscoverySource::Fmatch,
            candidates: mapped,
        });
    }

    if let Some(candidates) = cached_candidates(redis_pool, &cache_key).await {
        let mapped =
            map_fmatch_candidates(pool, order, candidates, SolverDiscoverySource::RedisCache)
                .await?;
        transition_to_quoting(pool, order).await?;
        return Ok(SolverDiscoveryResult {
            order_id: order.id,
            source: SolverDiscoverySource::RedisCache,
            candidates: mapped,
        });
    }

    let candidates = local_registry_candidates(pool, order).await?;
    if candidates.is_empty() {
        fail_discovery(pool, order, "no exchange solvers discovered").await?;
        anyhow::bail!("no exchange solvers discovered");
    }
    transition_to_quoting(pool, order).await?;
    Ok(SolverDiscoveryResult {
        order_id: order.id,
        source: SolverDiscoverySource::LocalRegistry,
        candidates,
    })
}

pub fn fmatch_exchange_content(order: &ExchangeOrder) -> String {
    format!(
        "exchange order {}; send {} {} from {}/{} via {} to {}/{} via {}; source_country={}; source_currency={}; target_country={}; target_currency={}; source_method={}; target_method={}; source_amount_minor={}",
        order.id,
        order.source_amount_minor,
        order.source_currency,
        order.source_country,
        order.source_currency,
        order.source_method_type,
        order.target_country,
        order.target_currency,
        order.target_method_type,
        order.source_country,
        order.source_currency,
        order.target_country,
        order.target_currency,
        order.source_method_type,
        order.target_method_type,
        order.source_amount_minor,
    )
}

fn order_cache_key(order: &ExchangeOrder) -> String {
    redis::cache_key(
        &format!("{}-{}", order.source_country, order.source_currency),
        &format!("{}-{}", order.target_country, order.target_currency),
        Some(&order.source_amount_minor.to_string()),
    )
}

async fn ensure_discovering(pool: &DbPool, order: &ExchangeOrder) -> Result<()> {
    match order.status {
        OrderStatus::Created => {
            let _ = repo::transition_order_status(
                pool,
                &order.id,
                OrderStatus::Created,
                OrderStatus::Discovering,
            )
            .await?;
        }
        OrderStatus::Discovering | OrderStatus::Quoting | OrderStatus::Quoted => {}
        other => anyhow::bail!(
            "exchange order cannot discover solvers from {}",
            other.as_str()
        ),
    }
    Ok(())
}

async fn transition_to_quoting(pool: &DbPool, order: &ExchangeOrder) -> Result<()> {
    let _ = repo::transition_order_status(
        pool,
        &order.id,
        OrderStatus::Discovering,
        OrderStatus::Quoting,
    )
    .await?;
    Ok(())
}

async fn fail_discovery(pool: &DbPool, order: &ExchangeOrder, reason: &str) -> Result<()> {
    let changed = repo::transition_order_status(
        pool,
        &order.id,
        OrderStatus::Discovering,
        OrderStatus::Failed,
    )
    .await?;
    if !changed {
        tracing::warn!(order_id = %order.id, reason, "failed to mark exchange discovery as failed");
    }
    Ok(())
}

async fn discover_via_fmatch(
    ap: &ActivityPubService,
    order: &ExchangeOrder,
) -> Result<Vec<AcquirerCandidate>> {
    let (_outcome, body) = ap
        .submit_request("candidates", &fmatch_exchange_content(order))
        .await?;
    Ok(body
        .as_ref()
        .map(AcquirerCandidate::from_reply)
        .unwrap_or_default())
}

async fn cache_candidates(
    redis_pool: Option<&RedisPool>,
    key: &str,
    candidates: &[AcquirerCandidate],
) {
    let Some(pool) = redis_pool else {
        return;
    };
    let cached = candidates
        .iter()
        .cloned()
        .map(CachedCandidate::from)
        .collect::<Vec<_>>();
    if let Err(err) = redis::set_json(pool, key, &cached, CANDIDATE_CACHE_TTL_SECS).await {
        tracing::warn!(error = %err, key, "failed to cache fmatch exchange candidates");
    }
}

async fn cached_candidates(
    redis_pool: Option<&RedisPool>,
    key: &str,
) -> Option<Vec<AcquirerCandidate>> {
    let pool = redis_pool?;
    match redis::get_json::<Vec<CachedCandidate>>(pool, key).await {
        Ok(Some(candidates)) => Some(candidates.into_iter().map(Into::into).collect()),
        Ok(None) => None,
        Err(err) => {
            tracing::warn!(error = %err, key, "failed to read cached exchange candidates");
            None
        }
    }
}

async fn map_fmatch_candidates(
    pool: &DbPool,
    order: &ExchangeOrder,
    candidates: Vec<AcquirerCandidate>,
    source: SolverDiscoverySource,
) -> Result<Vec<SolverCandidate>> {
    let mut out = Vec::new();
    for candidate in candidates {
        let solver = repo::upsert_solver(pool, &solver_from_candidate(order, &candidate)).await?;
        out.push(SolverCandidate {
            solver,
            rank: candidate.rank,
            source,
            fmatch_candidate: Some(candidate),
        });
    }
    out.sort_by_key(|candidate| candidate.rank);
    Ok(out)
}

async fn local_registry_candidates(
    pool: &DbPool,
    order: &ExchangeOrder,
) -> Result<Vec<SolverCandidate>> {
    let solvers = repo::solvers_for_order(pool, order).await?;
    Ok(solvers
        .into_iter()
        .enumerate()
        .map(|(idx, solver)| SolverCandidate {
            solver,
            rank: (idx + 1) as u64,
            source: SolverDiscoverySource::LocalRegistry,
            fmatch_candidate: None,
        })
        .collect())
}

fn solver_from_candidate(
    order: &ExchangeOrder,
    candidate: &AcquirerCandidate,
) -> NewExchangeSolver {
    let slug = solver_slug(candidate);
    NewExchangeSolver {
        slug,
        actor_id: candidate
            .raw
            .get("actor")
            .and_then(Value::as_str)
            .map(str::to_string),
        handle: non_empty(&candidate.short_id),
        display_name: candidate.name.clone(),
        status: SolverStatus::Discovered,
        countries: json!([&order.source_country, &order.target_country]),
        currencies: json!([&order.source_currency, &order.target_currency]),
        rails: json!([&order.source_method_type, &order.target_method_type]),
        min_amount_minor: None,
        max_amount_minor: None,
        fee_model: json!({
            "source": "fmatch",
            "price": candidate.price,
            "quality": candidate.quality,
            "rank": candidate.rank,
        }),
        risk_score: risk_from_quality(candidate.quality),
    }
}

fn solver_slug(candidate: &AcquirerCandidate) -> String {
    if let Some(short_id) = non_empty(&candidate.short_id) {
        return slugify(&short_id);
    }
    slugify(&candidate.name)
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        format!("solver-{}", Uuid::new_v4())
    } else {
        slug
    }
}

fn risk_from_quality(quality: Option<f64>) -> i32 {
    let quality = quality.unwrap_or(0.5).clamp(0.0, 1.0);
    ((1.0 - quality) * 100.0).round() as i32
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::exchange::model::ExchangeOrder;
    use crate::exchange::status::FundingInstructionStatus;

    fn order() -> ExchangeOrder {
        ExchangeOrder {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            idempotency_key: "idem".into(),
            source_country: "AM".into(),
            source_currency: "AMD".into(),
            source_amount_minor: 100_000,
            source_method_type: "card".into(),
            source_method_ref: None,
            target_country: "RU".into(),
            target_currency: "RUB".into(),
            target_amount_min_minor: Some(18_000),
            target_method_type: "bank".into(),
            target_method_ref: None,
            funding_instruction_id: None,
            funding_status: FundingInstructionStatus::NotStarted,
            status: OrderStatus::Created,
            deadline_at: None,
            selected_quote_id: None,
            failure_code: None,
            failure_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn fmatch_payload_contains_exchange_intent_fields() {
        let content = fmatch_exchange_content(&order());
        assert!(content.contains("source_country=AM"));
        assert!(content.contains("source_currency=AMD"));
        assert!(content.contains("target_country=RU"));
        assert!(content.contains("target_currency=RUB"));
        assert!(content.contains("source_amount_minor=100000"));
    }

    #[test]
    fn candidate_maps_to_discovered_solver() {
        let candidate = AcquirerCandidate {
            name: "Fast Solver".into(),
            short_id: "fast-low-limit".into(),
            rank: 1,
            price: Some(0.01),
            quality: Some(0.9),
            raw: json!({"actor": "https://solver.example/actor"}),
        };
        let solver = solver_from_candidate(&order(), &candidate);
        assert_eq!(solver.slug, "fast-low-limit");
        assert_eq!(
            solver.actor_id.as_deref(),
            Some("https://solver.example/actor")
        );
        assert_eq!(solver.status, SolverStatus::Discovered);
        assert_eq!(solver.risk_score, 10);
    }

    #[test]
    fn slugify_handles_display_names() {
        assert_eq!(slugify("Slow Better Rate LLC"), "slow-better-rate-llc");
        assert_eq!(slugify("  --FAST__solver-- "), "fast-solver");
    }
}

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use deadpool_redis::redis::AsyncCommands;
use tokio::sync::mpsc;

use crate::core::redis::RedisPool;
use crate::db::DbPool;

const REDIS_INDEX_KEY: &str = "pay3flow:provider-latency:index";
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ServiceOperation {
    P2pSearch,
    MarketTicker,
    RouteQuote,
    FiatQuote,
}

impl ServiceOperation {
    fn label(self) -> &'static str {
        match self {
            Self::P2pSearch => "p2p_search",
            Self::MarketTicker => "market_ticker",
            Self::RouteQuote => "route_quote",
            Self::FiatQuote => "fiat_quote",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "p2p_search" => Some(Self::P2pSearch),
            "market_ticker" => Some(Self::MarketTicker),
            "route_quote" => Some(Self::RouteQuote),
            "fiat_quote" => Some(Self::FiatQuote),
            _ => None,
        }
    }

    fn failure_penalty(self) -> Duration {
        match self {
            Self::P2pSearch | Self::MarketTicker => Duration::from_secs(4),
            Self::RouteQuote | Self::FiatQuote => Duration::from_secs(12),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MetricKey {
    operation: ServiceOperation,
    provider: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Estimate {
    duration_micros_total: u64,
    samples: u64,
    successes: u64,
    failures: u64,
    last_latency_micros: u64,
}

impl Estimate {
    fn average_micros(self) -> u64 {
        self.duration_micros_total
            .checked_div(self.samples)
            .unwrap_or_default()
    }

    fn sample(latency_micros: u64, success: bool) -> Self {
        Self {
            duration_micros_total: latency_micros,
            samples: 1,
            successes: u64::from(success),
            failures: u64::from(!success),
            last_latency_micros: latency_micros,
        }
    }

    fn merge(&mut self, other: Self) {
        self.duration_micros_total = self
            .duration_micros_total
            .saturating_add(other.duration_micros_total);
        self.samples = self.samples.saturating_add(other.samples);
        self.successes = self.successes.saturating_add(other.successes);
        self.failures = self.failures.saturating_add(other.failures);
        self.last_latency_micros = other.last_latency_micros;
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PendingWrite {
    postgres: Option<Estimate>,
    redis: Option<Estimate>,
}

impl PendingWrite {
    fn sample(sample: Estimate, redis_enabled: bool) -> Self {
        Self {
            postgres: Some(sample),
            redis: redis_enabled.then_some(sample),
        }
    }

    fn merge(&mut self, other: Self) {
        merge_optional(&mut self.postgres, other.postgres);
        merge_optional(&mut self.redis, other.redis);
    }

    fn is_pending(self) -> bool {
        self.postgres.is_some() || self.redis.is_some()
    }
}

fn merge_optional(target: &mut Option<Estimate>, source: Option<Estimate>) {
    if let Some(source) = source {
        target.get_or_insert_default().merge(source);
    }
}

#[derive(Clone)]
struct Persistence {
    pending: Arc<Mutex<HashMap<MetricKey, PendingWrite>>>,
    wake: mpsc::Sender<()>,
    redis_enabled: bool,
}

impl Persistence {
    /// This method never awaits. One channel token can represent any number of
    /// coalesced provider samples, so database slowness cannot back-pressure a
    /// user request and the queue cannot grow per request.
    fn enqueue(&self, key: MetricKey, sample: Estimate) {
        if let Ok(mut pending) = self.pending.lock() {
            pending
                .entry(key)
                .or_default()
                .merge(PendingWrite::sample(sample, self.redis_enabled));
        }
        if self.wake.try_send(()).is_err() {
            // A full channel means the worker is already scheduled. The sample
            // remains in `pending` and will be included when it drains.
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ServiceLatencyTracker {
    estimates: Arc<RwLock<HashMap<MetricKey, Estimate>>>,
    persistence: Option<Persistence>,
}

impl ServiceLatencyTracker {
    pub(crate) async fn persistent(pool: DbPool, redis: Option<RedisPool>) -> Self {
        let mut estimates = load_postgres(&pool).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "p2p.service_latency.postgres_load_failed");
            HashMap::new()
        });
        if let Some(redis) = redis.as_ref() {
            match load_redis(redis).await {
                Ok(cached) => merge_loaded(&mut estimates, cached),
                Err(error) => tracing::warn!(%error, "p2p.service_latency.redis_load_failed"),
            }
        }

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (wake, receiver) = mpsc::channel(1);
        tokio::spawn(persistence_worker(
            pool,
            redis.clone(),
            pending.clone(),
            wake.clone(),
            receiver,
        ));
        tracing::info!(
            metrics = estimates.len(),
            redis_enabled = redis.is_some(),
            "p2p.service_latency.loaded"
        );
        Self {
            estimates: Arc::new(RwLock::new(estimates)),
            persistence: Some(Persistence {
                pending,
                wake,
                redis_enabled: redis.is_some(),
            }),
        }
    }

    pub(crate) fn observe(
        &self,
        operation: ServiceOperation,
        provider: &str,
        elapsed: Duration,
        success: bool,
    ) {
        let effective = if success {
            elapsed
        } else {
            elapsed.max(operation.failure_penalty())
        };
        let sample = Estimate::sample(duration_micros(effective), success);
        let key = MetricKey {
            operation,
            provider: provider.to_string(),
        };
        let estimate = self
            .estimates
            .write()
            .ok()
            .map(|mut estimates| {
                let estimate = estimates.entry(key.clone()).or_default();
                estimate.merge(sample);
                *estimate
            })
            .unwrap_or(sample);
        if let Some(persistence) = &self.persistence {
            persistence.enqueue(key, sample);
        }
        tracing::debug!(
            provider,
            operation = operation.label(),
            success,
            latency_ms = elapsed.as_millis(),
            scheduling_latency_ms = effective.as_millis(),
            average_latency_ms = estimate.average_micros() / 1_000,
            samples = estimate.samples,
            "p2p.service_latency.observed"
        );
    }

    pub(crate) fn compare(
        &self,
        operation: ServiceOperation,
        left: &str,
        right: &str,
        cold_start: impl Fn(&str) -> u8,
    ) -> Ordering {
        let estimates = self.estimates.read().ok();
        let find = |provider: &str| {
            estimates.as_ref().and_then(|values| {
                values
                    .get(&MetricKey {
                        operation,
                        provider: provider.to_string(),
                    })
                    .copied()
            })
        };
        match (find(left), find(right)) {
            (Some(left_estimate), Some(right_estimate)) => left_estimate
                .average_micros()
                .cmp(&right_estimate.average_micros())
                .then_with(|| cold_start(left).cmp(&cold_start(right)))
                .then_with(|| left.cmp(right)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => cold_start(left)
                .cmp(&cold_start(right))
                .then_with(|| left.cmp(right)),
        }
    }

    pub(crate) fn render_metrics(&self) -> String {
        let Some(estimates) = self.estimates.read().ok() else {
            return String::new();
        };
        let mut rows = estimates.iter().collect::<Vec<_>>();
        rows.sort_by(|(left, _), (right, _)| {
            left.operation
                .label()
                .cmp(right.operation.label())
                .then_with(|| left.provider.cmp(&right.provider))
        });
        let mut output = String::from(
            "# HELP pay3flow_provider_response_latency_seconds Average provider response latency used by the scheduler; failures include a timeout penalty.\n\
# TYPE pay3flow_provider_response_latency_seconds gauge\n\
# HELP pay3flow_provider_response_samples_total Provider network responses observed by the scheduler.\n\
# TYPE pay3flow_provider_response_samples_total counter\n\
# HELP pay3flow_provider_response_failures_total Failed provider network responses observed by the scheduler.\n\
# TYPE pay3flow_provider_response_failures_total counter\n",
        );
        for (key, estimate) in rows {
            let provider = prometheus_label(&key.provider);
            let operation = key.operation.label();
            let average = estimate.average_micros() as f64 / 1_000_000.0;
            let _ = writeln!(output, "pay3flow_provider_response_latency_seconds{{provider=\"{provider}\",operation=\"{operation}\"}} {average}");
            let _ = writeln!(output, "pay3flow_provider_response_samples_total{{provider=\"{provider}\",operation=\"{operation}\"}} {}", estimate.samples);
            let _ = writeln!(output, "pay3flow_provider_response_failures_total{{provider=\"{provider}\",operation=\"{operation}\"}} {}", estimate.failures);
        }
        output
    }
}

fn duration_micros(duration: Duration) -> u64 {
    duration.as_micros().clamp(1, u64::MAX as u128) as u64
}

fn prometheus_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn merge_loaded(target: &mut HashMap<MetricKey, Estimate>, source: HashMap<MetricKey, Estimate>) {
    for (key, estimate) in source {
        let current = target.entry(key).or_default();
        if estimate.samples >= current.samples {
            *current = estimate;
        }
    }
}

async fn load_postgres(pool: &DbPool) -> Result<HashMap<MetricKey, Estimate>> {
    let client = pool.get().await.context("get PostgreSQL connection")?;
    let rows = client.query(
        "SELECT provider, operation, duration_micros_total, samples, successes, failures, last_latency_micros FROM provider_latency_metrics",
        &[],
    ).await.context("load provider latency metrics")?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let operation = ServiceOperation::parse(row.get::<_, String>(1).as_str())?;
            Some((
                MetricKey {
                    provider: row.get(0),
                    operation,
                },
                Estimate {
                    duration_micros_total: non_negative(row.get(2)),
                    samples: non_negative(row.get(3)),
                    successes: non_negative(row.get(4)),
                    failures: non_negative(row.get(5)),
                    last_latency_micros: non_negative(row.get(6)),
                },
            ))
        })
        .collect())
}

async fn load_redis(pool: &RedisPool) -> Result<HashMap<MetricKey, Estimate>> {
    let mut connection = pool.get().await.context("get Redis connection")?;
    let keys: Vec<String> = connection
        .smembers(REDIS_INDEX_KEY)
        .await
        .context("load Redis provider latency index")?;
    let mut estimates = HashMap::new();
    for key in keys {
        let values: HashMap<String, String> = connection
            .hgetall(&key)
            .await
            .with_context(|| format!("load Redis provider latency hash {key}"))?;
        let Some(operation) = values
            .get("operation")
            .and_then(|value| ServiceOperation::parse(value))
        else {
            continue;
        };
        let Some(provider) = values.get("provider").filter(|value| !value.is_empty()) else {
            continue;
        };
        estimates.insert(
            MetricKey {
                operation,
                provider: provider.clone(),
            },
            Estimate {
                duration_micros_total: redis_number(&values, "duration_micros_total"),
                samples: redis_number(&values, "samples"),
                successes: redis_number(&values, "successes"),
                failures: redis_number(&values, "failures"),
                last_latency_micros: redis_number(&values, "last_latency_micros"),
            },
        );
    }
    Ok(estimates)
}

fn redis_number(values: &HashMap<String, String>, field: &str) -> u64 {
    values
        .get(field)
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn non_negative(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

async fn persistence_worker(
    pool: DbPool,
    redis: Option<RedisPool>,
    pending: Arc<Mutex<HashMap<MetricKey, PendingWrite>>>,
    wake: mpsc::Sender<()>,
    mut receiver: mpsc::Receiver<()>,
) {
    while receiver.recv().await.is_some() {
        let mut batch = pending
            .lock()
            .ok()
            .map(|mut values| std::mem::take(&mut *values))
            .unwrap_or_default();
        if batch.is_empty() {
            continue;
        }

        let postgres = batch
            .iter()
            .filter_map(|(key, write)| write.postgres.map(|value| (key.clone(), value)))
            .collect();
        match persist_postgres(&pool, &postgres).await {
            Ok(()) => batch.values_mut().for_each(|write| write.postgres = None),
            Err(error) => tracing::warn!(%error, "p2p.service_latency.postgres_write_failed"),
        }
        if let Some(redis) = redis.as_ref() {
            let redis_batch = batch
                .iter()
                .filter_map(|(key, write)| write.redis.map(|value| (key.clone(), value)))
                .collect();
            match persist_redis(redis, &redis_batch).await {
                Ok(()) => batch.values_mut().for_each(|write| write.redis = None),
                Err(error) => tracing::warn!(%error, "p2p.service_latency.redis_write_failed"),
            }
        }

        batch.retain(|_, write| write.is_pending());
        if !batch.is_empty() {
            if let Ok(mut queued) = pending.lock() {
                for (key, write) in batch {
                    queued.entry(key).or_default().merge(write);
                }
            }
            tokio::time::sleep(RETRY_DELAY).await;
            let _ = wake.try_send(());
        }
    }
}

async fn persist_postgres(pool: &DbPool, deltas: &HashMap<MetricKey, Estimate>) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }
    let mut client = pool.get().await.context("get PostgreSQL connection")?;
    let transaction = client
        .transaction()
        .await
        .context("begin latency transaction")?;
    for (key, delta) in deltas {
        transaction.execute(r#"
INSERT INTO provider_latency_metrics
    (provider, operation, duration_micros_total, samples, successes, failures, last_latency_micros)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (provider, operation) DO UPDATE SET
    duration_micros_total = provider_latency_metrics.duration_micros_total + EXCLUDED.duration_micros_total,
    samples = provider_latency_metrics.samples + EXCLUDED.samples,
    successes = provider_latency_metrics.successes + EXCLUDED.successes,
    failures = provider_latency_metrics.failures + EXCLUDED.failures,
    last_latency_micros = EXCLUDED.last_latency_micros,
    updated_at = now()
"#, &[&key.provider, &key.operation.label(), &to_i64(delta.duration_micros_total),
        &to_i64(delta.samples), &to_i64(delta.successes), &to_i64(delta.failures),
        &to_i64(delta.last_latency_micros)]).await.context("upsert provider latency")?;
    }
    transaction
        .commit()
        .await
        .context("commit latency transaction")?;
    Ok(())
}

async fn persist_redis(pool: &RedisPool, deltas: &HashMap<MetricKey, Estimate>) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }
    const SCRIPT: &str = r#"
local total = redis.call('HINCRBY', KEYS[1], 'duration_micros_total', ARGV[3])
local samples = redis.call('HINCRBY', KEYS[1], 'samples', ARGV[4])
redis.call('HINCRBY', KEYS[1], 'successes', ARGV[5])
redis.call('HINCRBY', KEYS[1], 'failures', ARGV[6])
redis.call('HSET', KEYS[1], 'provider', ARGV[1], 'operation', ARGV[2], 'last_latency_micros', ARGV[7], 'average_latency_micros', math.floor(total / samples))
redis.call('SADD', KEYS[2], KEYS[1])
return samples
"#;
    let mut connection = pool.get().await.context("get Redis connection")?;
    for (key, delta) in deltas {
        let redis_key = format!(
            "pay3flow:provider-latency:{}:{}",
            key.operation.label(),
            key.provider
        );
        deadpool_redis::redis::cmd("EVAL")
            .arg(SCRIPT)
            .arg(2)
            .arg(redis_key)
            .arg(REDIS_INDEX_KEY)
            .arg(&key.provider)
            .arg(key.operation.label())
            .arg(delta.duration_micros_total)
            .arg(delta.samples)
            .arg(delta.successes)
            .arg(delta.failures)
            .arg(delta.last_latency_micros)
            .query_async::<_, i64>(&mut connection)
            .await
            .context("update Redis provider latency")?;
    }
    Ok(())
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learned_latency_overrides_cold_start_order() {
        let tracker = ServiceLatencyTracker::default();
        tracker.observe(
            ServiceOperation::RouteQuote,
            "slow",
            Duration::from_millis(900),
            true,
        );
        tracker.observe(
            ServiceOperation::RouteQuote,
            "fast",
            Duration::from_millis(80),
            true,
        );
        assert_eq!(
            tracker.compare(ServiceOperation::RouteQuote, "fast", "slow", |_| 0),
            Ordering::Less
        );
    }

    #[test]
    fn fast_failures_do_not_receive_priority() {
        let tracker = ServiceLatencyTracker::default();
        tracker.observe(
            ServiceOperation::FiatQuote,
            "failed",
            Duration::from_millis(1),
            false,
        );
        tracker.observe(
            ServiceOperation::FiatQuote,
            "healthy",
            Duration::from_millis(500),
            true,
        );
        assert_eq!(
            tracker.compare(ServiceOperation::FiatQuote, "healthy", "failed", |_| 0),
            Ordering::Less
        );
    }

    #[test]
    fn arithmetic_average_uses_every_request() {
        let tracker = ServiceLatencyTracker::default();
        tracker.observe(
            ServiceOperation::P2pSearch,
            "provider",
            Duration::from_millis(800),
            true,
        );
        tracker.observe(
            ServiceOperation::P2pSearch,
            "provider",
            Duration::from_millis(400),
            true,
        );
        let estimates = tracker.estimates.read().unwrap();
        let estimate = estimates
            .get(&MetricKey {
                operation: ServiceOperation::P2pSearch,
                provider: "provider".into(),
            })
            .unwrap();
        assert_eq!(estimate.average_micros(), 600_000);
        assert_eq!(estimate.samples, 2);
    }

    #[test]
    fn coalescing_preserves_every_sample_for_both_stores() {
        let mut write = PendingWrite::default();
        write.merge(PendingWrite::sample(Estimate::sample(100, true), true));
        write.merge(PendingWrite::sample(Estimate::sample(300, false), true));
        assert_eq!(write.postgres.unwrap().samples, 2);
        assert_eq!(write.postgres, write.redis);
    }
}

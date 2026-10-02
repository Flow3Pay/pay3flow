use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use deadpool_redis::redis::AsyncCommands;
use tokio::time::{self, Duration};

use crate::core::redis::RedisPool;
use crate::db::DbPool;
use crate::p2p::models::SourceStatus;

const LATENCY_SAMPLES_CAP: usize = 10;
const LATENCY_REDIS_UPDATE_INTERVAL_SECS: u64 = 30;
const LATENCY_DB_UPDATE_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Default)]
pub struct ProviderLatencyTracker {
    recent: Arc<parking_lot::RwLock<HashMap<String, Vec<u64>>>>,
}

impl ProviderLatencyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_background_refresh(
        self,
        pool: Option<DbPool>,
        redis: Option<RedisPool>,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let tracker = self.clone();
        let handle = tokio::spawn(async move {
            let mut db_interval =
                time::interval(Duration::from_secs(LATENCY_DB_UPDATE_INTERVAL_SECS));
            let mut redis_interval =
                time::interval(Duration::from_secs(LATENCY_REDIS_UPDATE_INTERVAL_SECS));

            loop {
                tokio::select! {
                    _ = db_interval.tick() => {
                        if let Some(pool) = &pool {
                            let _ = tracker.save_all_to_postgres(pool).await;
                        }
                    }
                    _ = redis_interval.tick() => {
                        if let Some(redis) = &redis {
                            let _ = tracker.save_all_to_redis(redis).await;
                        }
                    }
                }
            }
        });
        (self, handle)
    }

    pub fn is_empty(&self) -> bool {
        self.recent.read().is_empty()
    }

    pub fn avg_latency_ms(&self, provider: &str) -> Option<f64> {
        let recent = self.recent.read();
        let values = recent.get(provider)?;
        if values.is_empty() {
            return None;
        }
        Some(values.iter().map(|v| *v as f64).sum::<f64>() / values.len() as f64)
    }

    #[cfg(test)]
    pub fn priority_order<'a>(
        &'a self,
        providers: &'a [Arc<dyn crate::p2p::service::P2pSource>],
    ) -> Vec<&'a Arc<dyn crate::p2p::service::P2pSource>> {
        let mut ordered: Vec<&Arc<dyn crate::p2p::service::P2pSource>> =
            providers.iter().collect();
        ordered.sort_by(|a, b| {
            let avg_a = self.avg_latency_ms(a.name()).unwrap_or(f64::MAX);
            let avg_b = self.avg_latency_ms(b.name()).unwrap_or(f64::MAX);
            // Faster providers first (ascending by average).
            avg_a.partial_cmp(&avg_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        ordered
    }

    pub fn record(&self, status: &SourceStatus) {
        // Only real (non-cached) successful measurements count toward the average.
        if status.ok && !status.cached {
            let mut recent = self.recent.write();
            let values = recent.entry(status.source.clone()).or_default();
            values.push(status.latency_ms as u64);
            // FIFO window: drop the oldest sample, never reorder. Reordering would
            // silently keep the fastest samples instead of the most recent ones.
            if values.len() > LATENCY_SAMPLES_CAP {
                values.remove(0);
            }
        }
    }

    pub async fn load_from_postgres(&self, pool: &DbPool) -> Result<(), anyhow::Error> {
        let client = pool.get().await?;
        let rows = client
            .query(
                "SELECT provider_slug, avg_latency_ms, last_10_latency_ms, samples_count FROM provider_latency ORDER BY provider_slug",
                &[],
            )
            .await?;
        let mut recent = self.recent.write();
        for row in rows {
            let slug: String = row.get(0);
            let _avg: Option<f64> = row.get(1);
            // JSONB column decodes into serde_json::Value, not String.
            let window: serde_json::Value = row.get(2);
            let arr: Vec<u64> = serde_json::from_value(window).unwrap_or_default();
            let _samples: i64 = row.get(3);
            // Fill the ring buffer from the persisted window to avoid a cold start.
            if !arr.is_empty() {
                recent.insert(
                    slug,
                    arr.into_iter().take(LATENCY_SAMPLES_CAP).collect(),
                );
            }
        }
        tracing::info!(
            "loaded {} provider latency records from postgres",
            recent.len()
        );
        Ok(())
    }

    pub async fn save_all_to_postgres(&self, pool: &DbPool) -> Result<(), anyhow::Error> {
        let snapshot: HashMap<String, Vec<u64>> = self.recent.read().clone();
        if snapshot.is_empty() {
            return Ok(());
        }
        let client = pool.get().await?;
        for (slug, values) in snapshot {
            if values.is_empty() {
                continue;
            }
            let avg = values.iter().map(|v| *v as f64).sum::<f64>() / values.len() as f64;
            let json = serde_json::to_value(&values).context("serialize latency array")?;
            let result = client
                .execute(
                    r#"
INSERT INTO provider_latency
    (provider_slug, avg_latency_ms, last_10_latency_ms, samples_count, updated_at)
VALUES ($1, $2, $3, $4, now())
ON CONFLICT (provider_slug) DO UPDATE SET
    avg_latency_ms = EXCLUDED.avg_latency_ms,
    last_10_latency_ms = EXCLUDED.last_10_latency_ms,
    samples_count = EXCLUDED.samples_count,
    updated_at = now()
"#,
                    &[
                        &slug.as_str(),
                        &avg,
                        &json,
                        &(values.len() as i64),
                    ],
                )
                .await;
            if let Err(err) = result {
                tracing::warn!(provider = %slug, error = %err, "failed to persist provider latency");
            }
        }
        Ok(())
    }

    pub async fn save_all_to_redis(&self, redis: &RedisPool) -> Result<(), anyhow::Error> {
        let snapshot: HashMap<String, Vec<u64>> = self.recent.read().clone();
        if snapshot.is_empty() {
            return Ok(());
        }
        let mut conn = redis.get().await?;
        let count = snapshot.len();
        for (slug, values) in snapshot {
            if values.is_empty() {
                continue;
            }
            let key = format!("provider:latency:{}", slug);
            let avg = values.iter().map(|v| *v as f64).sum::<f64>() / values.len() as f64;
            // Persist the raw window so Redis alone can rebuild the average.
            let payload = serde_json::to_string(&serde_json::json!({
                "avg_latency_ms": avg,
                "samples": values,
            }))
            .context("serialize redis payload")?;
            let _: () = conn
                .set_ex(
                    &key,
                    payload,
                    (LATENCY_REDIS_UPDATE_INTERVAL_SECS * 3) as usize,
                )
                .await
                .context("redis latency set_ex")?;
        }
        tracing::debug!("persisted {} provider latencies to redis", count);
        Ok(())
    }

    pub async fn load_from_redis(&self, redis: &RedisPool) -> Result<(), anyhow::Error> {
        let keys: Vec<String> = redis
            .get()
            .await?
            .keys("provider:latency:*")
            .await
            .context("redis latency keys")?;
        if keys.is_empty() {
            return Ok(());
        }
        let mut recent = self.recent.write();
        let mut loaded = 0usize;
        for key in keys {
            let slug = key.trim_start_matches("provider:latency:").to_string();
            let payload: Option<String> = redis
                .get()
                .await?
                .get(&key)
                .await
                .context("redis latency get")?;
            let Some(payload) = payload else {
                continue;
            };
            let samples: Vec<u64> = serde_json::from_str::<serde_json::Value>(&payload)
                .ok()
                .and_then(|v| v.get("samples").cloned())
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            if samples.is_empty() {
                continue;
            }
            recent.insert(
                slug,
                samples
                    .into_iter()
                    .take(LATENCY_SAMPLES_CAP)
                    .collect(),
            );
            loaded += 1;
        }
        tracing::info!("loaded {} provider latency records from redis", loaded);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_are_computed_only_from_ok_non_cached_measurements() {
        let tracker = ProviderLatencyTracker::new();
        let ok = SourceStatus {
            source: "binance".into(),
            ok: true,
            cached: false,
            latency_ms: 100,
            offers_found: 0,
            error: None,
            avg_latency_ms: None,
        };
        let failed = SourceStatus {
            source: "bad".into(),
            ok: false,
            cached: false,
            latency_ms: 1,
            offers_found: 0,
            error: Some("boom".into()),
            avg_latency_ms: None,
        };
        let cached = SourceStatus {
            source: "cache".into(),
            ok: true,
            cached: true,
            latency_ms: 9999,
            offers_found: 0,
            error: None,
            avg_latency_ms: None,
        };
        tracker.record(&ok);
        tracker.record(&ok);
        tracker.record(&failed);
        tracker.record(&cached);
        assert_eq!(tracker.avg_latency_ms("binance"), Some(100.0));
        assert_eq!(tracker.avg_latency_ms("bad"), None);
        assert_eq!(tracker.avg_latency_ms("cache"), None);
    }

    #[test]
    fn keeps_only_last_10_measurements() {
        let tracker = ProviderLatencyTracker::new();
        for i in 0..15u128 {
            tracker.record(&SourceStatus {
                source: "fast".into(),
                ok: true,
                cached: false,
                latency_ms: i,
                offers_found: 0,
                error: None,
                avg_latency_ms: None,
            });
        }
        // last 10 values are 5..=14, average 9.5
        assert_eq!(tracker.avg_latency_ms("fast"), Some(9.5));
    }

    #[test]
    fn keeps_the_most_recent_samples_not_the_fastest() {
        let tracker = ProviderLatencyTracker::new();
        // Descending first so an implementation that sorts the window (keeping the
        // 10 smallest values) would produce a clearly different average.
        for value in [100u128, 90, 80, 70, 60, 50, 40, 30, 20, 10, 900, 800] {
            tracker.record(&SourceStatus {
                source: "bybit".into(),
                ok: true,
                cached: false,
                latency_ms: value,
                offers_found: 0,
                error: None,
                avg_latency_ms: None,
            });
        }
        // Last 10 = 80,70,60,50,40,30,20,10,900,800 -> sum 2060 -> avg 206.
        // (A window that sorts values would instead keep the 10 smallest: avg 125.)
        assert_eq!(tracker.avg_latency_ms("bybit"), Some(206.0));
    }

    struct MockP2pSource {
        name: String,
    }

    #[async_trait::async_trait]
    impl crate::p2p::service::P2pSource for MockP2pSource {
        fn name(&self) -> &str {
            &self.name
        }

        async fn search(
            &self,
            _query: &crate::p2p::models::P2pSearchQuery,
        ) -> Result<Vec<crate::p2p::models::P2pOffer>, anyhow::Error> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn priority_order_puts_faster_providers_first() {
        use crate::p2p::service::P2pSource;
        let tracker = ProviderLatencyTracker::new();
        tracker.record(&SourceStatus {
            source: "slow".into(),
            ok: true,
            cached: false,
            latency_ms: 1000,
            offers_found: 0,
            error: None,
            avg_latency_ms: None,
        });
        tracker.record(&SourceStatus {
            source: "fast".into(),
            ok: true,
            cached: false,
            latency_ms: 100,
            offers_found: 0,
            error: None,
            avg_latency_ms: None,
        });
        let providers = vec![
            Arc::new(MockP2pSource { name: "slow".into() }) as Arc<dyn P2pSource>,
            Arc::new(MockP2pSource { name: "fast".into() }) as Arc<dyn P2pSource>,
        ];
        let ordered = tracker.priority_order(&providers);
        tracing::debug!("avg slow={:?} fast={:?}", tracker.avg_latency_ms("slow"), tracker.avg_latency_ms("fast"));
        assert_eq!(ordered[0].name(), "fast");
        assert_eq!(ordered[1].name(), "slow");
    }

    #[test]
    fn priority_order_ignores_missing_measurements() {
        use crate::p2p::service::P2pSource;
        let tracker = ProviderLatencyTracker::new();
        let providers = vec![
            Arc::new(MockP2pSource { name: "a".into() }) as Arc<dyn P2pSource>,
            Arc::new(MockP2pSource { name: "b".into() }) as Arc<dyn P2pSource>,
        ];
        let ordered = tracker.priority_order(&providers);
        // No measurements -> keep original order.
        assert_eq!(ordered[0].name(), "a");
        assert_eq!(ordered[1].name(), "b");
    }
}

//! Public offer snapshots refreshed independently of route requests.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use parking_lot::{Mutex, RwLock};
use tokio::sync::{Notify, Semaphore};

use crate::config::Config;
use crate::core::redis::{self, RedisPool};
use crate::provider_adapter::P2pAdapterMarket;
use crate::providers::ProviderAdapterRecord;

use super::routes::fiat_quote_key;
use super::service::{build_search_response, merge_market_responses};
use super::{P2pOfferMarket, P2pSearchQuery, P2pSearchResponse, P2pSearchService, P2pSide};
use crate::route_engine::Amount;

const MAX_BACKGROUND_POLLS: usize = 8;
const MAX_DYNAMIC_TARGETS: usize = 64;
const REDIS_TIMEOUT: Duration = Duration::from_millis(150);
const COLD_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct OfferKey {
    fiat: String,
    asset: String,
    side: P2pSide,
    market: P2pOfferMarket,
}

impl OfferKey {
    fn from_query(query: &P2pSearchQuery, market: P2pOfferMarket) -> Self {
        Self {
            fiat: query.fiat.clone(),
            asset: query.asset.clone(),
            side: query.side,
            market,
        }
    }

    fn redis_key(&self) -> String {
        format!(
            "p2p:offers:v1:{:?}:{}:{}:{:?}",
            self.market, self.fiat, self.asset, self.side
        )
    }

    fn polling_interval(&self, requests: u32) -> Duration {
        match self.market {
            P2pOfferMarket::DirectExchange => Duration::from_secs(5 * 60),
            P2pOfferMarket::P2p if requests >= 10 => Duration::from_secs(15),
            P2pOfferMarket::P2p if requests > 0 => Duration::from_secs(30),
            P2pOfferMarket::P2p => Duration::from_secs(60),
        }
    }

    fn max_age(&self) -> Duration {
        match self.market {
            P2pOfferMarket::P2p => Duration::from_secs(5 * 60),
            P2pOfferMarket::DirectExchange => Duration::from_secs(15 * 60),
        }
    }

    fn baseline_query(&self) -> P2pSearchQuery {
        P2pSearchQuery {
            fiat: self.fiat.clone(),
            asset: self.asset.clone(),
            side: self.side,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(100),
            sources: None,
        }
    }
}

pub(crate) struct BackgroundOfferStore {
    snapshots: RwLock<HashMap<OfferKey, P2pSearchResponse>>,
    planned: HashSet<OfferKey>,
    active: Mutex<HashMap<OfferKey, u64>>,
    next_generation: AtomicU64,
    requests: Mutex<HashMap<OfferKey, u32>>,
    last_demand: Mutex<HashMap<OfferKey, Instant>>,
    notify: Notify,
    planned_semaphore: Semaphore,
    dynamic_semaphore: Semaphore,
    redis: Option<RedisPool>,
}

impl BackgroundOfferStore {
    fn new(redis: Option<RedisPool>, planned: HashSet<OfferKey>) -> Self {
        Self {
            snapshots: RwLock::new(HashMap::new()),
            planned,
            active: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
            requests: Mutex::new(HashMap::new()),
            last_demand: Mutex::new(HashMap::new()),
            notify: Notify::new(),
            planned_semaphore: Semaphore::new(MAX_BACKGROUND_POLLS - 2),
            dynamic_semaphore: Semaphore::new(2),
            redis,
        }
    }

    pub(crate) fn targets(config: &Config, records: &[ProviderAdapterRecord]) -> Vec<OfferKey> {
        let mut fiats = config.route_source_fiats.clone();
        let mut market_assets: HashMap<P2pOfferMarket, Vec<String>> = HashMap::new();
        for record in records {
            if let Some(adapter) = record
                .config
                .as_ref()
                .and_then(|config| config.p2p.as_ref())
            {
                fiats.extend(adapter.supported_fiats.iter().cloned());
                let market = match adapter.market {
                    P2pAdapterMarket::P2p => P2pOfferMarket::P2p,
                    P2pAdapterMarket::DirectExchange => P2pOfferMarket::DirectExchange,
                };
                market_assets
                    .entry(market)
                    .or_default()
                    .extend(adapter.supported_assets.iter().cloned());
            }
            if let Some(workflow) = &record.workflow {
                market_assets
                    .entry(P2pOfferMarket::DirectExchange)
                    .or_default()
                    .extend(workflow.supported_assets.iter().cloned());
            }
        }
        fiats.sort_by_key(|fiat| {
            let priority = match fiat.as_str() {
                "AMD" => 0,
                "RUB" => 1,
                _ => 2,
            };
            (priority, fiat.clone())
        });
        fiats.dedup();
        let mut keys = Vec::new();
        for market in [P2pOfferMarket::P2p, P2pOfferMarket::DirectExchange] {
            let assets = market_assets.entry(market).or_default();
            assets.extend(config.p2p_search_assets.iter().cloned());
            if assets.is_empty() {
                assets.push("USDT".into());
            }
            assets.sort_by_key(|asset| {
                let priority = match asset.as_str() {
                    "USDT" => 0,
                    "USDC" => 1,
                    "BTC" => 2,
                    _ => 3,
                };
                (priority, asset.clone())
            });
            assets.dedup();
            for asset in assets {
                for fiat in &fiats {
                    for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
                        keys.push(OfferKey {
                            fiat: fiat.clone(),
                            asset: asset.clone(),
                            side,
                            market,
                        });
                    }
                }
            }
        }
        keys
    }

    fn record_request(&self, key: &OfferKey) {
        let mut requests = self.requests.lock();
        let count = requests.entry(key.clone()).or_default();
        *count = count.saturating_add(1);
        self.last_demand.lock().insert(key.clone(), Instant::now());
    }

    fn dynamic_expired(&self, key: &OfferKey) -> bool {
        !self.planned.contains(key)
            && self
                .last_demand
                .lock()
                .get(key)
                .is_none_or(|last| last.elapsed() > Duration::from_secs(10 * 60))
    }

    fn take_requests(&self, key: &OfferKey) -> u32 {
        self.requests.lock().remove(key).unwrap_or_default()
    }

    fn response(&self, key: &OfferKey) -> Option<P2pSearchResponse> {
        let response = self.snapshots.read().get(key)?.clone();
        let age = Utc::now()
            .signed_duration_since(response.searched_at)
            .to_std()
            .unwrap_or_default();
        (age <= key.max_age()).then_some(response)
    }

    async fn load(&self, key: &OfferKey) -> Option<P2pSearchResponse> {
        if let Some(response) = self.response(key) {
            return Some(response);
        }
        let redis = self.redis.as_ref()?;
        let response = tokio::time::timeout(
            REDIS_TIMEOUT,
            redis::get_json::<P2pSearchResponse>(redis, &key.redis_key()),
        )
        .await
        .ok()?
        .ok()??;
        self.snapshots.write().insert(key.clone(), response.clone());
        self.response(key)
    }

    fn publish(&self, key: OfferKey, response: P2pSearchResponse) {
        self.snapshots.write().insert(key.clone(), response.clone());
        self.notify.notify_waiters();
        if let Some(redis) = self.redis.clone() {
            tokio::spawn(async move {
                let result = tokio::time::timeout(
                    REDIS_TIMEOUT,
                    redis::set_json(&redis, &key.redis_key(), &response, key.max_age().as_secs()),
                )
                .await;
                if let Ok(Err(error)) = result {
                    tracing::warn!(%error, "background offer snapshot save failed");
                }
            });
        }
    }
}

impl P2pSearchService {
    pub(crate) fn start_background_offer_refresh(
        &mut self,
        targets: Vec<OfferKey>,
        redis: Option<RedisPool>,
    ) {
        let mut fiats = targets
            .iter()
            .map(|key| key.fiat.clone())
            .collect::<Vec<_>>();
        fiats.sort();
        fiats.dedup();
        let planned = targets.iter().cloned().collect();
        let store = Arc::new(BackgroundOfferStore::new(redis, planned));
        self.background_offers = Some(store);
        for target in targets {
            self.register_background_target(target);
        }
        self.start_background_fiat_quotes(fiats);
        self.start_background_spot_tickers();
        self.start_background_route_quotes();
    }

    fn start_background_route_quotes(&self) {
        for provider in self.route_providers.iter().cloned() {
            let cache = self.provider_quote_snapshots.clone();
            let semaphore = self.quote_semaphore.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    let assets = match tokio::time::timeout(
                        Duration::from_secs(30),
                        provider.supported_assets(),
                    )
                    .await
                    {
                        Ok(assets) => assets,
                        Err(_) => Vec::new(),
                    };
                    let mut assets = assets;
                    assets.sort_by_key(|asset| {
                        let priority = match asset.symbol.as_str() {
                            "USDT" => 0,
                            "USDC" => 1,
                            "BTC" => 2,
                            "ETH" => 3,
                            _ => 4,
                        };
                        (priority, asset.to_string())
                    });
                    assets.dedup();
                    assets.truncate(12);
                    let jobs = assets
                        .iter()
                        .flat_map(|from| {
                            assets
                                .iter()
                                .filter(move |to| *to != from)
                                .map(move |to| (from.clone(), to.clone()))
                        })
                        .collect::<Vec<_>>();
                    stream::iter(jobs)
                        .for_each_concurrent(8, |(from, to)| {
                            let provider = provider.clone();
                            let cache = cache.clone();
                            let semaphore = semaphore.clone();
                            async move {
                                let amount_value = match from.symbol.as_str() {
                                    "USDT" | "USDC" | "DAI" | "FDUSD" => 100.0,
                                    "BTC" => 0.002,
                                    "ETH" => 0.05,
                                    _ => 1.0,
                                };
                                let Ok(amount) = Amount::from_f64(amount_value, from.clone())
                                else {
                                    return;
                                };
                                let Ok(permit) = semaphore.acquire().await else {
                                    return;
                                };
                                let result = tokio::time::timeout(
                                    Duration::from_secs(12),
                                    provider.quotes(from.clone(), to.clone(), amount),
                                )
                                .await;
                                drop(permit);
                                if let Ok(Ok(quotes)) = result {
                                    if !quotes.is_empty() {
                                        let key = format!("{}|{}|{}", provider.name(), from, to);
                                        if let Ok(mut cache) = cache.write() {
                                            cache.insert(key, (std::time::Instant::now(), quotes));
                                        }
                                    }
                                }
                            }
                        })
                        .await;
                }
            });
        }
    }

    fn start_background_spot_tickers(&self) {
        for source in self.market_sources.iter().cloned() {
            let cache = self.market_tickers_cache.clone();
            let redis = self.route_cache_redis.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let name = source.name().to_string();
                let key = format!("p2p:spot:v1:{name}");
                if let Some(redis) = &redis {
                    if let Ok(Ok(Some(tickers))) = tokio::time::timeout(
                        REDIS_TIMEOUT,
                        redis::get_json::<Vec<super::spot::CryptoTicker>>(redis, &key),
                    )
                    .await
                    {
                        if let Ok(mut cache) = cache.write() {
                            cache.insert(name.clone(), tickers);
                        }
                    }
                }
                loop {
                    interval.tick().await;
                    if let Ok(Ok(tickers)) =
                        tokio::time::timeout(Duration::from_secs(30), source.tickers()).await
                    {
                        if let Ok(mut cache) = cache.write() {
                            cache.insert(name.clone(), tickers.clone());
                        }
                        if let Some(redis) = &redis {
                            let _ = tokio::time::timeout(
                                REDIS_TIMEOUT,
                                redis::set_json(redis, &key, &tickers, 15 * 60),
                            )
                            .await;
                        }
                    }
                }
            });
        }
    }

    fn start_background_fiat_quotes(&self, fiats: Vec<String>) {
        for provider in self.fiat_route_providers.iter().cloned() {
            for source in &fiats {
                for target in &fiats {
                    if source == target || !provider.supports_pair(source, target) {
                        continue;
                    }
                    let service = self.clone();
                    let provider = provider.clone();
                    let source = source.clone();
                    let target = target.clone();
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
                        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        let baseline_amount = 100_000.0;
                        let key =
                            fiat_quote_key(provider.name(), &source, &target, baseline_amount);
                        loop {
                            interval.tick().await;
                            let _ = service
                                .fetch_fiat_quote(
                                    key.clone(),
                                    provider.clone(),
                                    source.clone(),
                                    target.clone(),
                                    baseline_amount,
                                )
                                .await;
                        }
                    });
                }
            }
        }
    }

    fn register_background_target(&self, key: OfferKey) {
        let Some(store) = self.background_offers.as_ref() else {
            return;
        };
        let generation = {
            let mut active = store.active.lock();
            if active.contains_key(&key) {
                return;
            }
            if !store.planned.contains(&key)
                && active
                    .keys()
                    .filter(|key| !store.planned.contains(*key))
                    .count()
                    >= MAX_DYNAMIC_TARGETS
            {
                let oldest = {
                    let demand = store.last_demand.lock();
                    active
                        .keys()
                        .filter(|key| !store.planned.contains(*key))
                        .min_by_key(|key| demand.get(*key).copied())
                        .cloned()
                };
                if let Some(oldest) = oldest {
                    active.remove(&oldest);
                    store.snapshots.write().remove(&oldest);
                    store.last_demand.lock().remove(&oldest);
                    store.requests.lock().remove(&oldest);
                }
            }
            let generation = store.next_generation.fetch_add(1, Ordering::Relaxed);
            active.insert(key.clone(), generation);
            generation
        };
        let store = store.clone();
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                if store.active.lock().get(&key) != Some(&generation) {
                    break;
                }
                let semaphore = if store.planned.contains(&key) {
                    &store.planned_semaphore
                } else {
                    &store.dynamic_semaphore
                };
                let Ok(permit) = semaphore.acquire().await else {
                    break;
                };
                let query = key.baseline_query();
                let result = tokio::time::timeout(Duration::from_secs(60), async {
                    if service.has_fmatch_backend() {
                        service
                            .search_fmatch_market(query, Some(key.market), None)
                            .await
                    } else {
                        service.run_search_once(query, None, Some(key.market)).await
                    }
                })
                .await;
                drop(permit);
                match result {
                    Ok(Ok(response)) => store.publish(key.clone(), response),
                    Ok(Err(error)) => tracing::warn!(%error, ?key, "background offer poll failed"),
                    Err(_) => tracing::warn!(?key, "background offer poll timed out"),
                }
                tokio::time::sleep(key.polling_interval(store.take_requests(&key))).await;
                if store.dynamic_expired(&key) {
                    let mut active = store.active.lock();
                    if active.get(&key) == Some(&generation) {
                        active.remove(&key);
                        store.snapshots.write().remove(&key);
                        store.last_demand.lock().remove(&key);
                    }
                    break;
                }
            }
        });
    }

    pub(crate) async fn search_background_offers(
        &self,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
    ) -> anyhow::Result<P2pSearchResponse> {
        let query = query.normalize()?;
        let store = self
            .background_offers
            .as_ref()
            .expect("background store checked by caller");
        let markets = market.map_or_else(
            || vec![P2pOfferMarket::P2p, P2pOfferMarket::DirectExchange],
            |market| vec![market],
        );
        let mut snapshots = Vec::new();
        for market in markets {
            let key = OfferKey::from_query(&query, market);
            store.record_request(&key);
            self.register_background_target(key.clone());
            let notified = store.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let mut snapshot = store.load(&key).await;
            if snapshot.is_none() {
                if tokio::time::timeout(COLD_WAIT, notified).await.is_ok() {
                    snapshot = store.load(&key).await;
                }
            }
            if let Some(snapshot) = snapshot {
                let age = Utc::now()
                    .signed_duration_since(snapshot.searched_at)
                    .to_std()
                    .unwrap_or_default();
                let requested_sources = query.sources.as_deref();
                let offers = snapshot
                    .offers
                    .iter()
                    .filter(|offer| {
                        requested_sources
                            .is_none_or(|names| names.split(',').any(|name| name == offer.source))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let statuses = snapshot
                    .sources
                    .into_iter()
                    .filter(|status| {
                        requested_sources
                            .is_none_or(|names| names.split(',').any(|name| name == status.source))
                    })
                    .map(|mut status| {
                        status.cached = true;
                        status
                    })
                    .collect();
                snapshots.push(build_search_response(
                    query.clone(),
                    &offers,
                    statuses,
                    true,
                    &snapshot.source,
                    snapshot.stale || age > key.polling_interval(0) * 2,
                    snapshot.observed_at,
                ));
            }
        }
        Ok(match snapshots.len() {
            0 => build_search_response(
                query,
                &[],
                Vec::new(),
                true,
                "background_pending",
                true,
                None,
            ),
            1 => snapshots.remove(0),
            _ => merge_market_responses(query, snapshots.remove(0), snapshots.remove(0)),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Result;
    use async_trait::async_trait;

    use super::*;
    use crate::p2p::{P2pOffer, P2pSource};

    struct CountSource(Arc<AtomicUsize>);

    struct OrderedSource {
        name: &'static str,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl P2pSource for OrderedSource {
        fn name(&self) -> &str {
            self.name
        }

        async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            self.calls.lock().push(self.name);
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl P2pSource for CountSource {
        fn name(&self) -> &str {
            "counted"
        }

        async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn a_user_search_reads_the_background_snapshot_without_polling_the_source() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service = P2pSearchService::with_sources(
            vec![Arc::new(CountSource(calls.clone()))],
            Duration::from_secs(1),
        );
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: None,
        };
        let key = OfferKey::from_query(&query, P2pOfferMarket::P2p);
        service.start_background_offer_refresh(vec![key.clone()], None);
        tokio::time::timeout(Duration::from_secs(1), async {
            while service
                .background_offers
                .as_ref()
                .unwrap()
                .response(&key)
                .is_none()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let before = calls.load(Ordering::Relaxed);
        let mut filtered = query;
        filtered.amount = Some(100_000.0);
        service
            .search_market(filtered, Some(P2pOfferMarket::P2p))
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), before);
        assert_eq!(before, 1);
    }

    #[tokio::test]
    async fn cached_reputation_prioritizes_background_provider_calls() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(OrderedSource {
                    name: "low",
                    calls: calls.clone(),
                }),
                Arc::new(OrderedSource {
                    name: "high",
                    calls: calls.clone(),
                }),
            ],
            Duration::from_secs(1),
        );
        service
            .reputation_scores
            .write()
            .extend([("low".into(), 20), ("high".into(), 90)]);
        let query = OfferKey {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            market: P2pOfferMarket::P2p,
        }
        .baseline_query();
        service
            .run_search_once(query, None, Some(P2pOfferMarket::P2p))
            .await
            .unwrap();
        assert_eq!(*calls.lock(), vec!["high", "low"]);
    }

    #[test]
    fn p2p_polling_cadence_tracks_demand() {
        let key = OfferKey {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            market: P2pOfferMarket::P2p,
        };
        assert_eq!(key.polling_interval(0), Duration::from_secs(60));
        assert_eq!(key.polling_interval(1), Duration::from_secs(30));
        assert_eq!(key.polling_interval(10), Duration::from_secs(15));
        assert_eq!(
            OfferKey {
                market: P2pOfferMarket::DirectExchange,
                ..key
            }
            .polling_interval(10),
            Duration::from_secs(300)
        );
    }
}

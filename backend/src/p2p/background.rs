//! Public offer snapshots refreshed independently of route requests.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use parking_lot::{Mutex, RwLock};
use tokio::sync::{Notify, Semaphore};

use crate::config::Config;
use crate::core::redis::{self, RedisPool};
use crate::networks::NetworkCatalog;
use crate::provider_adapter::P2pAdapterMarket;
use crate::providers::ProviderAdapterRecord;

use super::routes::fiat_quote_key;
use super::service::{build_search_response, merge_market_responses};
use super::{P2pOfferMarket, P2pSearchQuery, P2pSearchResponse, P2pSearchService, P2pSide};
use crate::route_engine::{Amount, Asset};

const MAX_BACKGROUND_POLLS: usize = 3;
const MAX_REGULAR_BACKGROUND_POLLS: usize = 2;
const MAX_ROUTE_QUOTE_POLLS_PER_PROVIDER: usize = 4;
const MAX_SPOT_POLLS: usize = 4;
const MAX_DYNAMIC_TARGETS: usize = 64;
const MAX_MEMORY_SNAPSHOTS: usize = 128;
const REDIS_TIMEOUT: Duration = Duration::from_millis(150);
const COLD_WAIT: Duration = Duration::from_secs(65);

struct QuotePoll {
    from: Asset,
    to: Asset,
    key: String,
}

fn quote_asset_priority(symbol: &str) -> u8 {
    match symbol {
        "USDT" => 0,
        "USDC" => 1,
        "BTC" => 2,
        "ETH" => 3,
        "BNB" => 4,
        "ADA" => 5,
        _ => 6,
    }
}

fn quote_polls(provider: &str, mut assets: Vec<Asset>) -> Vec<QuotePoll> {
    assets.sort_by_key(Asset::to_string);
    assets.dedup();
    let mut polls = assets
        .iter()
        .flat_map(|from| {
            assets
                .iter()
                .filter(move |to| *to != from)
                .map(move |to| QuotePoll {
                    from: from.clone(),
                    to: to.clone(),
                    key: format!("{provider}|{from}|{to}"),
                })
        })
        .collect::<Vec<_>>();
    polls.sort_by_key(|poll| {
        (
            quote_asset_priority(&poll.from.symbol),
            quote_asset_priority(&poll.to.symbol),
            poll.key.clone(),
        )
    });
    polls
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct OfferKey {
    fiat: String,
    asset: String,
    side: P2pSide,
    market: P2pOfferMarket,
}

impl OfferKey {
    fn priority(&self) -> (u8, u8, u8, u8, &str, &str) {
        let fiat = match self.fiat.as_str() {
            "AMD" => 0,
            "RUB" => 1,
            _ => 2,
        };
        let asset = quote_asset_priority(&self.asset);
        let market = match self.market {
            P2pOfferMarket::P2p => 0,
            P2pOfferMarket::DirectExchange => 1,
        };
        (
            fiat,
            asset,
            market,
            self.side as u8,
            &self.fiat,
            &self.asset,
        )
    }

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
    queue: Mutex<PollQueue>,
    requests: Mutex<HashMap<OfferKey, u32>>,
    last_demand: Mutex<HashMap<OfferKey, Instant>>,
    notify: Notify,
    work_notify: Notify,
    redis: Option<RedisPool>,
}

struct PollQueue {
    registered: HashSet<OfferKey>,
    due: HashMap<OfferKey, Instant>,
}

impl BackgroundOfferStore {
    fn new(redis: Option<RedisPool>, planned: HashSet<OfferKey>) -> Self {
        let now = Instant::now();
        let due = planned.iter().cloned().map(|key| (key, now)).collect();
        Self {
            snapshots: RwLock::new(HashMap::new()),
            queue: Mutex::new(PollQueue {
                registered: planned.clone(),
                due,
            }),
            planned,
            requests: Mutex::new(HashMap::new()),
            last_demand: Mutex::new(HashMap::new()),
            notify: Notify::new(),
            work_notify: Notify::new(),
            redis,
        }
    }

    pub(crate) fn targets(
        config: &Config,
        records: &[ProviderAdapterRecord],
        networks: &NetworkCatalog,
        catalog_fiats: &[String],
    ) -> Vec<OfferKey> {
        let mut fiats = config.route_source_fiats.clone();
        fiats.extend_from_slice(catalog_fiats);
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
            let mut assets = match market {
                P2pOfferMarket::P2p => vec!["USDT".into()],
                P2pOfferMarket::DirectExchange => {
                    let mut assets = market_assets.remove(&market).unwrap_or_default();
                    assets.extend(config.p2p_search_assets.iter().cloned());
                    assets.extend(networks.assets());
                    assets
                }
            };
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
                    if market == P2pOfferMarket::P2p
                        && !matches!(fiat.as_str(), "AMD" | "RUB" | "USD" | "EUR" | "BYN")
                    {
                        continue;
                    }
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
        keys.sort_by_key(|key| {
            let asset = match key.asset.as_str() {
                "USDT" => 0,
                "USDC" => 1,
                "BTC" => 2,
                _ => 3,
            };
            let fiat = match key.fiat.as_str() {
                "AMD" => 0,
                "RUB" => 1,
                _ => 2,
            };
            let market = match key.market {
                P2pOfferMarket::P2p => 0,
                P2pOfferMarket::DirectExchange => 1,
            };
            (fiat, asset, market, key.side as u8)
        });
        keys
    }

    fn record_request(&self, key: &OfferKey) -> u32 {
        let recent = self
            .last_demand
            .lock()
            .insert(key.clone(), Instant::now())
            .is_some_and(|last| last.elapsed() <= Duration::from_secs(60));
        let count = {
            let mut requests = self.requests.lock();
            let count = requests.entry(key.clone()).or_default();
            *count = if recent { count.saturating_add(1) } else { 1 };
            *count
        };
        let fresh = self.snapshots.read().get(key).is_some_and(|response| {
            Utc::now()
                .signed_duration_since(response.searched_at)
                .to_std()
                .unwrap_or_default()
                <= key.max_age()
        });
        if !fresh {
            if let Some(due) = self.queue.lock().due.get_mut(key) {
                *due = Instant::now();
            }
        }
        self.work_notify.notify_waiters();
        count
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

    fn register_target(&self, key: OfferKey) {
        let mut demand = self.last_demand.lock();
        let mut queue = self.queue.lock();
        if queue.registered.contains(&key) {
            return;
        }
        if queue.registered.len().saturating_sub(self.planned.len()) >= MAX_DYNAMIC_TARGETS {
            let oldest = queue
                .due
                .keys()
                .filter(|candidate| !self.planned.contains(*candidate))
                .min_by_key(|candidate| demand.get(*candidate).copied())
                .cloned();
            if let Some(oldest) = oldest {
                queue.registered.remove(&oldest);
                queue.due.remove(&oldest);
                self.snapshots.write().remove(&oldest);
                self.requests.lock().remove(&oldest);
                demand.remove(&oldest);
            }
        }
        queue.registered.insert(key.clone());
        queue.due.insert(key, Instant::now());
        drop(queue);
        drop(demand);
        self.work_notify.notify_waiters();
    }

    fn take_due(&self, urgent_only: bool) -> (Option<OfferKey>, Option<Instant>) {
        let demand = self.last_demand.lock();
        let mut queue = self.queue.lock();
        let now = Instant::now();
        let requested = |key: &OfferKey| {
            demand
                .get(key)
                .is_some_and(|time| time.elapsed() < Duration::from_secs(60))
        };
        let key = queue
            .due
            .iter()
            .filter(|(key, due)| **due <= now && (!urgent_only || requested(key)))
            .min_by_key(|(key, _)| (!requested(key), key.priority()))
            .map(|(key, _)| key.clone());
        if let Some(key) = &key {
            queue.due.remove(key);
        }
        let next_due = queue
            .due
            .iter()
            .filter(|(key, _)| !urgent_only || requested(key))
            .map(|(_, due)| *due)
            .min();
        (key, next_due)
    }

    fn finish_poll(&self, key: OfferKey) {
        let requests = self.take_requests(&key);
        let expired = self.dynamic_expired(&key);
        let mut queue = self.queue.lock();
        if !queue.registered.contains(&key) {
            return;
        }
        if expired {
            queue.registered.remove(&key);
        } else {
            queue
                .due
                .insert(key.clone(), Instant::now() + key.polling_interval(requests));
            self.work_notify.notify_waiters();
        }
        drop(queue);
        if expired {
            self.snapshots.write().remove(&key);
            self.last_demand.lock().remove(&key);
        }
    }

    fn remember(&self, key: OfferKey, response: P2pSearchResponse) {
        let demand = self.last_demand.lock();
        let mut snapshots = self.snapshots.write();
        if snapshots.len() >= MAX_MEMORY_SNAPSHOTS && !snapshots.contains_key(&key) {
            if let Some(oldest) = snapshots
                .keys()
                .min_by_key(|candidate| demand.get(*candidate).copied())
                .cloned()
            {
                snapshots.remove(&oldest);
            }
        }
        snapshots.insert(key, response);
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
        self.remember(key.clone(), response.clone());
        self.response(key)
    }

    fn publish(&self, key: OfferKey, response: P2pSearchResponse) {
        self.remember(key.clone(), response.clone());
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
        self.background_offers = Some(store.clone());
        for index in 0..MAX_BACKGROUND_POLLS {
            let service = self.clone();
            let store = store.clone();
            let urgent_only = index >= MAX_REGULAR_BACKGROUND_POLLS;
            tokio::spawn(async move {
                loop {
                    let notified = store.work_notify.notified();
                    tokio::pin!(notified);
                    notified.as_mut().enable();
                    let (key, next_due) = store.take_due(urgent_only);
                    let Some(key) = key else {
                        if let Some(due) = next_due {
                            tokio::select! {
                                _ = notified => {},
                                _ = tokio::time::sleep_until(due.into()) => {},
                            }
                        } else {
                            notified.await;
                        }
                        continue;
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
                    let still_registered = store.queue.lock().registered.contains(&key);
                    match result {
                        Ok(Ok(response)) if still_registered => {
                            store.publish(key.clone(), response)
                        }
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) => {
                            tracing::warn!(%error, ?key, "background offer poll failed")
                        }
                        Err(_) => tracing::warn!(?key, "background offer poll timed out"),
                    }
                    store.finish_poll(key);
                    if !urgent_only {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            });
        }
        self.start_background_fiat_quotes(fiats);
        self.start_background_spot_tickers();
        self.start_background_route_quotes();
    }

    fn start_background_route_quotes(&self) {
        for provider in self.route_providers.iter().cloned() {
            let cache = self.provider_quote_snapshots.clone();
            let requests = self.provider_quote_requests.clone();
            let semaphore = self.quote_semaphore.clone();
            tokio::spawn(async move {
                loop {
                    let assets = match tokio::time::timeout(
                        Duration::from_secs(30),
                        provider.supported_assets(),
                    )
                    .await
                    {
                        Ok(assets) => assets,
                        Err(_) => Vec::new(),
                    };
                    let mut jobs = quote_polls(provider.name(), assets);
                    let mut last_demand = HashMap::new();
                    while !jobs.is_empty() {
                        let demand = requests
                            .lock()
                            .map(|requests| requests.clone())
                            .unwrap_or_default();
                        if demand != last_demand {
                            jobs.sort_by(|left, right| {
                                let left_count = demand.get(&left.key).copied().unwrap_or_default();
                                let right_count =
                                    demand.get(&right.key).copied().unwrap_or_default();
                                right_count.cmp(&left_count).then_with(|| {
                                    (
                                        quote_asset_priority(&left.from.symbol),
                                        quote_asset_priority(&left.to.symbol),
                                        &left.key,
                                    )
                                        .cmp(&(
                                            quote_asset_priority(&right.from.symbol),
                                            quote_asset_priority(&right.to.symbol),
                                            &right.key,
                                        ))
                                })
                            });
                            last_demand = demand;
                        }
                        let batch = jobs
                            .drain(..jobs.len().min(MAX_ROUTE_QUOTE_POLLS_PER_PROVIDER))
                            .collect::<Vec<_>>();
                        stream::iter(batch)
                            .for_each_concurrent(MAX_ROUTE_QUOTE_POLLS_PER_PROVIDER, |job| {
                                let provider = provider.clone();
                                let cache = cache.clone();
                                let semaphore = semaphore.clone();
                                async move {
                                    let amount_value = match job.from.symbol.as_str() {
                                        "USDT" | "USDC" | "DAI" | "FDUSD" => 100.0,
                                        "BTC" => 0.002,
                                        "ETH" => 0.05,
                                        _ => 1.0,
                                    };
                                    let Ok(amount) =
                                        Amount::from_f64(amount_value, job.from.clone())
                                    else {
                                        return;
                                    };
                                    let Ok(permit) = semaphore.acquire().await else {
                                        return;
                                    };
                                    let result = tokio::time::timeout(
                                        Duration::from_secs(12),
                                        provider.quotes(job.from.clone(), job.to.clone(), amount),
                                    )
                                    .await;
                                    drop(permit);
                                    if let Ok(Ok(quotes)) = result {
                                        if !quotes.is_empty() {
                                            if let Ok(mut cache) = cache.write() {
                                                cache.insert(
                                                    job.key,
                                                    (std::time::Instant::now(), quotes),
                                                );
                                            }
                                        }
                                    }
                                }
                            })
                            .await;
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                    tokio::time::sleep(Duration::from_secs(5 * 60)).await;
                }
            });
        }
    }

    fn start_background_spot_tickers(&self) {
        let slots = Arc::new(Semaphore::new(MAX_SPOT_POLLS));
        for source in self.market_sources.iter().cloned() {
            let cache = self.market_tickers_cache.clone();
            let redis = self.route_cache_redis.clone();
            let slots = slots.clone();
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
                    let Ok(permit) = slots.acquire().await else {
                        break;
                    };
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
                    drop(permit);
                }
            });
        }
    }

    fn start_background_fiat_quotes(&self, fiats: Vec<String>) {
        let mut jobs = Vec::new();
        for provider in self.fiat_route_providers.iter().cloned() {
            for source in &fiats {
                for target in &fiats {
                    if source == target || !provider.supports_pair(source, target) {
                        continue;
                    }
                    jobs.push((provider.clone(), source.clone(), target.clone()));
                }
            }
        }
        if jobs.is_empty() {
            return;
        }
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                stream::iter(jobs.iter().cloned())
                    .for_each_concurrent(2, |(provider, source, target)| {
                        let service = service.clone();
                        async move {
                            let baseline_amount = 100_000.0;
                            let key =
                                fiat_quote_key(provider.name(), &source, &target, baseline_amount);
                            let _ = service
                                .fetch_fiat_quote(key, provider, source, target, baseline_amount)
                                .await;
                        }
                    })
                    .await;
                tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            }
        });
    }

    fn register_background_target(&self, key: OfferKey) {
        let Some(store) = self.background_offers.as_ref() else {
            return;
        };
        store.register_target(key);
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
        let cold_deadline = tokio::time::Instant::now() + COLD_WAIT;
        let mut snapshots = Vec::new();
        for market in markets {
            let key = OfferKey::from_query(&query, market);
            store.record_request(&key);
            self.register_background_target(key.clone());
            let snapshot = loop {
                let notified = store.notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if let Some(snapshot) = store.load(&key).await {
                    break Some(snapshot);
                }
                if tokio::time::timeout_at(cold_deadline, notified)
                    .await
                    .is_err()
                {
                    break None;
                }
            };
            if let Some(snapshot) = snapshot {
                snapshots.push(shape_snapshot(query.clone(), &key, snapshot));
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

    pub(crate) async fn cached_p2p_offers(
        &self,
        query: &P2pSearchQuery,
    ) -> Option<P2pSearchResponse> {
        let query = query.clone().normalize().ok()?;
        let store = self.background_offers.as_ref()?;
        let key = OfferKey::from_query(&query, P2pOfferMarket::P2p);
        let requests = store.record_request(&key);
        if requests >= 3 {
            self.register_background_target(key.clone());
        }
        store
            .load(&key)
            .await
            .map(|snapshot| shape_snapshot(query, &key, snapshot))
    }
}

fn shape_snapshot(
    query: P2pSearchQuery,
    key: &OfferKey,
    snapshot: P2pSearchResponse,
) -> P2pSearchResponse {
    let age = Utc::now()
        .signed_duration_since(snapshot.searched_at)
        .to_std()
        .unwrap_or_default();
    let requested_sources = query.sources.as_deref();
    let offers = snapshot
        .offers
        .into_iter()
        .filter(|offer| {
            requested_sources.is_none_or(|names| names.split(',').any(|name| name == offer.source))
        })
        .collect::<Vec<_>>();
    let statuses = snapshot
        .sources
        .into_iter()
        .filter(|status| {
            requested_sources.is_none_or(|names| names.split(',').any(|name| name == status.source))
        })
        .map(|mut status| {
            status.cached = true;
            status
        })
        .collect();
    build_search_response(
        query,
        &offers,
        statuses,
        true,
        &snapshot.source,
        snapshot.stale || age > key.polling_interval(0) * 2,
        snapshot.observed_at,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Result;
    use async_trait::async_trait;

    use super::*;
    use crate::p2p::{P2pOffer, P2pSource};

    struct CountSource(Arc<AtomicUsize>);

    struct SlowSource(Arc<AtomicUsize>);

    struct TrackingSource {
        calls: Arc<Mutex<Vec<String>>>,
        active: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
    }

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

    #[async_trait]
    impl P2pSource for SlowSource {
        fn name(&self) -> &str {
            "slow"
        }

        fn market(&self) -> P2pOfferMarket {
            P2pOfferMarket::DirectExchange
        }

        async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl P2pSource for TrackingSource {
        fn name(&self) -> &str {
            "tracking"
        }

        fn market(&self) -> P2pOfferMarket {
            P2pOfferMarket::DirectExchange
        }

        async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            self.calls.lock().push(query.asset.clone());
            tokio::time::sleep(Duration::from_millis(80)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn background_queue_bounds_searches_and_prioritizes_requested_pairs() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut service = P2pSearchService::with_sources(
            vec![Arc::new(TrackingSource {
                calls: calls.clone(),
                active: active.clone(),
                peak: peak.clone(),
            })],
            Duration::from_secs(1),
        );
        let targets = (0..24)
            .map(|index| OfferKey {
                fiat: "AMD".into(),
                asset: format!("T{index:02}"),
                side: P2pSide::SellCrypto,
                market: P2pOfferMarket::DirectExchange,
            })
            .collect::<Vec<_>>();
        let requested = targets.last().unwrap().clone();
        service.start_background_offer_refresh(targets, None);

        tokio::time::timeout(Duration::from_secs(2), async {
            while calls.lock().len() < MAX_REGULAR_BACKGROUND_POLLS {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        service
            .background_offers
            .as_ref()
            .unwrap()
            .record_request(&requested);
        tokio::time::timeout(Duration::from_secs(3), async {
            while calls.lock().len() < MAX_BACKGROUND_POLLS + 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        assert_eq!(calls.lock()[MAX_REGULAR_BACKGROUND_POLLS], requested.asset);
        assert!(peak.load(Ordering::SeqCst) <= MAX_BACKGROUND_POLLS);
        assert_eq!(
            service
                .background_offers
                .as_ref()
                .unwrap()
                .queue
                .lock()
                .registered
                .len(),
            24
        );
    }

    #[test]
    fn offer_snapshots_have_a_memory_limit_and_keep_requested_directions() {
        let store = BackgroundOfferStore::new(None, HashSet::new());
        let key = |index| OfferKey {
            fiat: "AMD".into(),
            asset: format!("T{index:03}"),
            side: P2pSide::SellCrypto,
            market: P2pOfferMarket::DirectExchange,
        };
        for index in 0..MAX_MEMORY_SNAPSHOTS {
            let target = key(index);
            store.remember(
                target.clone(),
                build_search_response(
                    target.baseline_query(),
                    &[],
                    Vec::new(),
                    true,
                    "provider",
                    false,
                    None,
                ),
            );
        }
        store.record_request(&key(0));
        let newest = key(MAX_MEMORY_SNAPSHOTS);
        store.remember(
            newest.clone(),
            build_search_response(
                newest.baseline_query(),
                &[],
                Vec::new(),
                true,
                "provider",
                false,
                None,
            ),
        );
        let snapshots = store.snapshots.read();
        assert_eq!(snapshots.len(), MAX_MEMORY_SNAPSHOTS);
        assert!(snapshots.contains_key(&key(0)));
        assert!(snapshots.contains_key(&newest));
    }

    #[tokio::test]
    async fn a_cold_exchanger_search_waits_for_its_background_snapshot() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service = P2pSearchService::with_sources(
            vec![Arc::new(SlowSource(calls.clone()))],
            Duration::from_secs(60),
        );
        let query = OfferKey {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            market: P2pOfferMarket::DirectExchange,
        };
        service.start_background_offer_refresh(vec![query.clone()], None);
        let response = tokio::time::timeout(
            Duration::from_secs(3),
            service.search_market(query.baseline_query(), Some(P2pOfferMarket::DirectExchange)),
        )
        .await
        .expect("cold search must finish when its background poll does")
        .unwrap();
        assert_eq!(response.source, "provider");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn an_unpopular_p2p_pair_is_searched_on_demand() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut service = P2pSearchService::with_sources(
            vec![Arc::new(CountSource(calls.clone()))],
            Duration::from_secs(1),
        );
        service.start_background_offer_refresh(Vec::new(), None);
        let response = service
            .search_market(
                OfferKey {
                    fiat: "AMD".into(),
                    asset: "BNB".into(),
                    side: P2pSide::SellCrypto,
                    market: P2pOfferMarket::P2p,
                }
                .baseline_query(),
                Some(P2pOfferMarket::P2p),
            )
            .await
            .unwrap();
        assert_eq!(response.source, "provider");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
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

    #[test]
    fn quote_polling_keeps_uncommon_assets_after_the_first_twelve() {
        let mut assets = (0..14)
            .map(|index| Asset::new(format!("T{index}"), None::<&str>).unwrap())
            .collect::<Vec<_>>();
        assets.push(Asset::new("ADA", Some("cardano")).unwrap());
        assets.push(Asset::new("BNB", Some("bnb-smart-chain")).unwrap());
        assets.push(Asset::new("USDT", Some("tron")).unwrap());
        let polls = quote_polls("bestchange", assets.clone());

        assert_eq!(polls.len(), assets.len() * (assets.len() - 1));
        assert!(polls.iter().any(|poll| {
            poll.from == Asset::new("ADA", Some("cardano")).unwrap()
                && poll.to == Asset::new("USDT", Some("tron")).unwrap()
        }));
        assert!(polls.iter().any(|poll| {
            poll.from == Asset::new("BNB", Some("bnb-smart-chain")).unwrap()
                && poll.to == Asset::new("USDT", Some("tron")).unwrap()
        }));
    }

    #[test]
    fn background_targets_cover_catalog_fiats_and_network_assets() {
        let config = Config::load().unwrap();
        let targets = BackgroundOfferStore::targets(
            &config,
            &[],
            &NetworkCatalog::test_default(),
            &["BYN".into()],
        );
        assert!(targets.iter().any(|key| {
            key.fiat == "BYN"
                && key.asset == "BTC"
                && key.side == P2pSide::SellCrypto
                && key.market == P2pOfferMarket::DirectExchange
        }));
        assert!(targets.iter().any(|key| {
            key.fiat == "BYN" && key.asset == "USDT" && key.market == P2pOfferMarket::P2p
        }));
    }
}

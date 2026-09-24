use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::db::DbPool;
use crate::networks::NetworkCatalog;
use crate::p2p::declarative::{DeclarativeMarketSource, DeclarativeP2pSource};
use crate::p2p::spot::{CryptoMarketSource, CryptoTicker};
use crate::p2p::workflow::WorkflowP2pSource;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum P2pSide {
    #[serde(rename = "buy", alias = "buy_crypto")]
    BuyCrypto,
    #[serde(rename = "sell", alias = "sell_crypto")]
    SellCrypto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pSearchQuery {
    pub fiat: String,
    pub asset: String,
    pub side: P2pSide,
    /// Fiat amount, for example `100000` AMD. Omit to search every limit range.
    pub amount: Option<f64>,
    pub payment_method: Option<String>,
    pub merchant_only: Option<bool>,
    pub min_orders: Option<u64>,
    /// Fraction from 0 to 1. `0.95` means a 95% completion rate.
    pub min_completion_rate: Option<f64>,
    pub limit: Option<usize>,
    /// Optional comma-separated list of P2P sources to query.
    pub sources: Option<String>,
}

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
        !(2..=32).contains(&source.len())
            || !source.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        bail!("sources must contain only 2-32 character alphanumeric names");
    }
    sources.sort_unstable();
    sources.dedup();
    Ok(Some(sources.join(",")))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Advertiser {
    pub id: Option<String>,
    pub nickname: String,
    pub user_type: Option<String>,
    pub is_merchant: bool,
    pub is_verified: bool,
    pub completed_orders_30d: Option<u64>,
    pub completion_rate_30d: Option<f64>,
    pub positive_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct P2pOffer {
    pub source: String,
    pub ad_id: String,
    pub side: P2pSide,
    pub fiat: String,
    pub asset: String,
    /// Fiat units paid or received for one unit of `asset`.
    pub price: String,
    pub available_asset: String,
    pub min_fiat: String,
    pub max_fiat: String,
    pub payment_methods: Vec<String>,
    pub pay_time_limit_minutes: Option<u32>,
    pub advertiser: Advertiser,
    pub advertiser_profile_url: Option<String>,
    pub source_url: String,
    /// True only when the venue URL addresses this exact advertisement.
    /// Public market URLs must not be presented as exact offer links.
    pub source_url_is_exact: bool,
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
            || self
                .payment_methods
                .iter()
                .all(|method| method.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return PaymentMethodMatch::Unknown;
        }

        PaymentMethodMatch::No
    }

    fn matches(&self, query: &P2pSearchQuery) -> bool {
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

fn canonical_payment_method(value: &str) -> String {
    let normalized = value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "tbank" | "tinkoffbank" => "tinkoff".into(),
        "sber" => "sberbank".into(),
        other => other.into(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceStatus {
    pub source: String,
    pub ok: bool,
    pub latency_ms: u128,
    pub offers_found: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2pSearchResponse {
    pub query: P2pSearchQuery,
    pub searched_at: DateTime<Utc>,
    pub cached: bool,
    pub offers: Vec<P2pOffer>,
    pub sources: Vec<SourceStatus>,
}

#[async_trait]
pub(crate) trait P2pSource: Send + Sync {
    fn name(&self) -> &str;
    fn timeout(&self, default: Duration) -> Duration {
        default
    }
    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>>;
}

#[derive(Clone)]
pub struct P2pSearchService {
    enabled: bool,
    timeout: Duration,
    cache_ttl: Duration,
    cache: Arc<RwLock<HashMap<String, CachedSearch>>>,
    sources: Arc<[Arc<dyn P2pSource>]>,
    market_sources: Arc<[Arc<dyn CryptoMarketSource>]>,
    pub(crate) default_assets: Arc<[String]>,
    pub(crate) networks: NetworkCatalog,
}

#[derive(Clone)]
struct CachedSearch {
    inserted_at: Instant,
    response: P2pSearchResponse,
}

impl P2pSearchService {
    pub fn from_config(config: &Config, networks: NetworkCatalog) -> Result<Self> {
        Self::from_provider_records(config, networks, Vec::new())
    }

    pub async fn from_database(
        config: &Config,
        networks: NetworkCatalog,
        pool: &DbPool,
    ) -> Result<Self> {
        let records = crate::providers::adapters(pool).await?;
        Self::from_provider_records(config, networks, records)
    }

    fn from_provider_records(
        config: &Config,
        networks: NetworkCatalog,
        records: Vec<crate::providers::ProviderAdapterRecord>,
    ) -> Result<Self> {
        if records.iter().any(|record| record.workflow.is_some()) {
            playwright_rs::server::driver::get_driver_executable()
                .map_err(|error| anyhow::anyhow!("Playwright driver is unavailable: {error}"))?;
            if let Ok(executable) = std::env::var("PLAYWRIGHT_CHROMIUM_EXECUTABLE") {
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
        for record in &records {
            if let Some(source) = DeclarativeP2pSource::from_record(client.clone(), record) {
                sources.push(Arc::new(source));
            }
            if let Some(source) = DeclarativeMarketSource::from_record(client.clone(), record) {
                market_sources.push(Arc::new(source));
            }
            if let Some(source) = WorkflowP2pSource::from_record(record) {
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
            market_sources: market_sources.into(),
            default_assets: default_assets.into(),
            networks,
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
            networks: NetworkCatalog::test_default(),
        }
    }

    #[cfg(test)]
    fn with_cache_ttl(mut self, cache_ttl: Duration) -> Self {
        self.cache_ttl = cache_ttl;
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

    pub async fn search(&self, query: P2pSearchQuery) -> Result<P2pSearchResponse> {
        self.run_search(query, None).await
    }

    pub(crate) async fn stream_search(
        &self,
        query: P2pSearchQuery,
        updates: mpsc::Sender<P2pSearchResponse>,
    ) -> Result<P2pSearchResponse> {
        self.run_search(query, Some(updates)).await
    }

    async fn run_search(
        &self,
        query: P2pSearchQuery,
        updates: Option<mpsc::Sender<P2pSearchResponse>>,
    ) -> Result<P2pSearchResponse> {
        if !self.enabled {
            bail!("P2P search is disabled");
        }
        let query = query.normalize()?;
        let cache_key = serde_json::to_string(&query).context("failed to build P2P cache key")?;
        if let Some(mut response) = self.cached(&cache_key) {
            response.cached = true;
            if let Some(updates) = updates {
                let _ = updates.send(response.clone()).await;
            }
            return Ok(response);
        }
        let selected_sources =
            self.sources
                .iter()
                .filter(|source| {
                    query.sources.as_deref().is_none_or(|requested| {
                        requested.split(',').any(|name| name == source.name())
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
        let mut searches = selected_sources
            .into_iter()
            .map(|source| {
                let query = query.clone();
                async move {
                    let started = Instant::now();
                    let timeout = source.timeout(self.timeout);
                    let result = tokio::time::timeout(timeout, source.search(&query)).await;
                    let elapsed = started.elapsed().as_millis();
                    match result {
                        Ok(Ok(offers)) => {
                            let count = offers.len();
                            (
                                offers,
                                SourceStatus {
                                    source: source.name().to_string(),
                                    ok: true,
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
                let response =
                    build_search_response(query.clone(), &collected_offers, sources.clone(), false);
                let _ = updates.send(response).await;
            }
        }

        let response = build_search_response(query, &collected_offers, sources, false);
        if response.sources.iter().any(|source| source.ok) {
            self.cache_response(cache_key, response.clone());
        }
        Ok(response)
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

    pub(crate) async fn search_market_tickers(
        &self,
        requested_sources: Option<&str>,
    ) -> Vec<(String, Result<Vec<CryptoTicker>>)> {
        let selected_sources = self
            .market_sources
            .iter()
            .filter(|source| {
                requested_sources
                    .is_none_or(|requested| requested.split(',').any(|name| name == source.name()))
            })
            .collect::<Vec<_>>();

        join_all(selected_sources.into_iter().map(|source| async move {
            let name = source.name().to_string();
            let result = tokio::time::timeout(source.timeout(self.timeout), source.tickers())
                .await
                .map_err(|_| anyhow::anyhow!("{} spot ticker request timed out", name))
                .and_then(|result| result);
            (name, result)
        }))
        .await
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
    }
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
mod tests {
    use super::*;

    struct StubSource {
        name: &'static str,
        offers: Vec<P2pOffer>,
        delay: Duration,
    }

    #[async_trait]
    impl P2pSource for StubSource {
        fn name(&self) -> &str {
            self.name
        }

        async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            tokio::time::sleep(self.delay).await;
            Ok(self.offers.clone())
        }
    }

    fn offer(source: &str, price: &str, min: &str, max: &str, orders: u64) -> P2pOffer {
        P2pOffer {
            source: source.into(),
            ad_id: format!("{source}-{price}"),
            side: P2pSide::BuyCrypto,
            fiat: "AMD".into(),
            asset: "USDT".into(),
            price: price.into(),
            available_asset: "1000".into(),
            min_fiat: min.into(),
            max_fiat: max.into(),
            payment_methods: vec!["IDBank".into()],
            pay_time_limit_minutes: Some(15),
            advertiser: Advertiser {
                id: None,
                nickname: source.into(),
                user_type: Some("merchant".into()),
                is_merchant: true,
                is_verified: true,
                completed_orders_30d: Some(orders),
                completion_rate_30d: Some(0.99),
                positive_rate: Some(1.0),
            },
            advertiser_profile_url: None,
            source_url: "https://example.test".into(),
            source_url_is_exact: false,
        }
    }

    #[test]
    fn reputation_filters_keep_offers_when_metrics_are_not_published() {
        let query = P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "USDC".into(),
            side: P2pSide::SellCrypto,
            amount: Some(100.0),
            payment_method: Some("Sberbank".into()),
            merchant_only: None,
            min_orders: Some(20),
            min_completion_rate: Some(0.9),
            limit: Some(10),
            sources: Some("whitebird".into()),
        };
        let mut direct_exchange = offer("whitebird", "84.7", "1", "100000", 100);
        direct_exchange.side = P2pSide::SellCrypto;
        direct_exchange.fiat = "RUB".into();
        direct_exchange.asset = "USDC".into();
        direct_exchange.payment_methods.clear();
        direct_exchange.advertiser.completed_orders_30d = None;
        direct_exchange.advertiser.completion_rate_30d = None;

        assert!(direct_exchange.matches(&query));

        direct_exchange.advertiser.completed_orders_30d = Some(19);
        assert!(!direct_exchange.matches(&query));

        direct_exchange.advertiser.completed_orders_30d = None;
        direct_exchange.advertiser.completion_rate_30d = Some(0.89);
        assert!(!direct_exchange.matches(&query));
    }

    #[test]
    fn global_limit_keeps_the_best_offer_from_each_source() {
        let query = P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "USDC".into(),
            side: P2pSide::SellCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(60),
            sources: Some("bybit,whitebird".into()),
        };
        let mut offers = (0..60)
            .map(|index| {
                let mut offer = offer(
                    "bybit",
                    &format!("{}", 90.0 - f64::from(index) / 100.0),
                    "1",
                    "100000",
                    100,
                );
                offer.side = P2pSide::SellCrypto;
                offer.fiat = "RUB".into();
                offer.asset = "USDC".into();
                offer
            })
            .collect::<Vec<_>>();
        let mut whitebird = offer("whitebird", "84.7", "1", "100000", 0);
        whitebird.side = P2pSide::SellCrypto;
        whitebird.fiat = "RUB".into();
        whitebird.asset = "USDC".into();
        whitebird.payment_methods.clear();
        whitebird.advertiser.completed_orders_30d = None;
        whitebird.advertiser.completion_rate_30d = None;
        offers.push(whitebird);

        let response = build_search_response(query, &offers, Vec::new(), false);

        assert_eq!(response.offers.len(), 60);
        assert!(response
            .offers
            .iter()
            .any(|offer| offer.source == "whitebird"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fans_out_filters_and_sorts() {
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(StubSource {
                    name: "one",
                    offers: vec![offer("one", "362", "10000", "200000", 100)],
                    delay: Duration::from_millis(30),
                }),
                Arc::new(StubSource {
                    name: "two",
                    offers: vec![
                        offer("two", "360", "10000", "200000", 200),
                        offer("too-small", "350", "100", "1000", 200),
                    ],
                    delay: Duration::from_millis(30),
                }),
            ],
            Duration::from_secs(1),
        );
        let started = Instant::now();
        let response = service
            .search(P2pSearchQuery {
                fiat: "amd".into(),
                asset: "usdt".into(),
                side: P2pSide::BuyCrypto,
                amount: Some(50_000.0),
                payment_method: None,
                merchant_only: Some(true),
                min_orders: Some(50),
                min_completion_rate: Some(0.95),
                limit: Some(10),
                sources: None,
            })
            .await
            .unwrap();

        assert!(started.elapsed() < Duration::from_millis(55));
        assert_eq!(response.offers.len(), 2);
        assert_eq!(response.offers[0].source, "two");
        assert!(response.sources.iter().all(|source| source.ok));
    }

    #[tokio::test]
    async fn times_out_one_source_without_losing_other_results() {
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(StubSource {
                    name: "fast",
                    offers: vec![offer("fast", "360", "1", "100000", 20)],
                    delay: Duration::ZERO,
                }),
                Arc::new(StubSource {
                    name: "slow",
                    offers: Vec::new(),
                    delay: Duration::from_millis(100),
                }),
            ],
            Duration::from_millis(10),
        );
        let response = service
            .search(P2pSearchQuery {
                fiat: "AMD".into(),
                asset: "USDT".into(),
                side: P2pSide::BuyCrypto,
                amount: None,
                payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                limit: None,
                sources: None,
            })
            .await
            .unwrap();

        assert_eq!(response.offers.len(), 1);
        assert_eq!(
            response.sources.iter().filter(|source| source.ok).count(),
            1
        );
    }

    #[tokio::test]
    async fn queries_only_requested_sources() {
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(StubSource {
                    name: "one",
                    offers: vec![offer("one", "362", "1", "100000", 20)],
                    delay: Duration::ZERO,
                }),
                Arc::new(StubSource {
                    name: "two",
                    offers: vec![offer("two", "360", "1", "100000", 20)],
                    delay: Duration::ZERO,
                }),
            ],
            Duration::from_secs(1),
        );

        let response = service
            .search(P2pSearchQuery {
                fiat: "AMD".into(),
                asset: "USDT".into(),
                side: P2pSide::BuyCrypto,
                amount: None,
                payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                limit: None,
                sources: Some("TWO".into()),
            })
            .await
            .unwrap();

        assert_eq!(response.sources.len(), 1);
        assert_eq!(response.sources[0].source, "two");
        assert_eq!(response.offers.len(), 1);
        assert_eq!(response.offers[0].source, "two");
    }

    #[tokio::test]
    async fn unavailable_requested_source_returns_an_empty_result() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(StubSource {
                name: "one",
                offers: vec![offer("one", "362", "1", "100000", 20)],
                delay: Duration::ZERO,
            })],
            Duration::from_secs(1),
        );

        let response = service
            .search(P2pSearchQuery {
                fiat: "AMD".into(),
                asset: "USDT".into(),
                side: P2pSide::BuyCrypto,
                amount: None,
                payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                limit: None,
                sources: Some("whitebird".into()),
            })
            .await
            .expect("an unavailable catalog source must not fail the whole route search");

        assert!(response.offers.is_empty());
        assert!(response.sources.is_empty());
    }

    #[test]
    fn reports_only_configured_search_sources() {
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(StubSource {
                    name: "one",
                    offers: Vec::new(),
                    delay: Duration::ZERO,
                }),
                Arc::new(StubSource {
                    name: "two",
                    offers: Vec::new(),
                    delay: Duration::ZERO,
                }),
            ],
            Duration::from_secs(1),
        );

        assert_eq!(
            service.searchable_sources(),
            HashSet::from(["one".to_string(), "two".to_string()])
        );
        assert!(!service.searchable_sources().contains("whitebird"));
    }

    #[tokio::test]
    async fn reuses_short_lived_search_cache() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(StubSource {
                name: "cached",
                offers: vec![offer("cached", "360", "1", "100000", 20)],
                delay: Duration::ZERO,
            })],
            Duration::from_secs(1),
        )
        .with_cache_ttl(Duration::from_secs(1));
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: None,
            sources: None,
        };

        let first = service.search(query.clone()).await.unwrap();
        let second = service.search(query).await.unwrap();
        assert!(!first.cached);
        assert!(second.cached);
        assert_eq!(first.offers, second.offers);
    }

    #[test]
    fn payment_filter_keeps_opaque_ids_but_rejects_known_mismatch() {
        let mut numeric = offer("bybit", "360", "1", "100000", 20);
        numeric.payment_methods = vec!["18".into(), "40".into()];
        assert_eq!(
            numeric.payment_method_match("Sberbank"),
            PaymentMethodMatch::Unknown
        );

        let known = offer("bitget", "360", "1", "100000", 20);
        assert_eq!(
            known.payment_method_match("Ameriabank"),
            PaymentMethodMatch::No
        );
        assert_eq!(
            known.payment_method_match("ID Bank"),
            PaymentMethodMatch::Exact
        );
    }
}

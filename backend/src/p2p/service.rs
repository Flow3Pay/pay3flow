use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::{join_all, BoxFuture};
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Semaphore};

use crate::activitypub::Service as ActivityPubService;
use crate::compiled_provider_code::bestchange::BestChangeSource;
use crate::compiled_provider_code::papa_change::PapaChangeSource;
use crate::compiled_provider_code::skylabs::SkyLabsSource;
use crate::config::Config;
use crate::core::redis::{get_json, RedisPool};
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
use crate::service_reputation::{vote_quality_score, ServiceStats};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;
const MAX_BACKGROUND_SEARCHES: usize = 5;
const BACKGROUND_SEARCH_INTERVAL: Duration = Duration::from_millis(2_400);
const BACKGROUND_POPULAR_PAIR_CADENCE: usize = 7;
const MAX_TRACKED_POPULAR_PAIRS: usize = 512;
const MAX_REFRESHED_POPULAR_PAIRS: usize = 3;
// Keep enough pair snapshots for a paced full catalog pass (25 observations
// per minute) and a small delay before a pair is visited again.
const PROVIDER_SNAPSHOT_TTL: Duration = Duration::from_secs(35 * 60);
const PROVIDER_SNAPSHOT_STALE_AFTER: Duration = Duration::from_secs(30 * 60);
const MAX_PROVIDER_SNAPSHOTS: usize = 1_024;

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

fn canonical_payment_method(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn provider_snapshot_key(query: &P2pSearchQuery, market: Option<P2pOfferMarket>) -> String {
    format!("{market:?}:{}:{}:{:?}", query.fiat, query.asset, query.side)
}

fn canonicalize_offer_payment_methods(
    offer: &mut P2pOffer,
    aliases: &BTreeMap<String, Vec<String>>,
) {
    for method in &mut offer.payment_methods {
        let normalized = canonical_payment_method(method);
        let canonical = aliases
            .iter()
            .find(|(canonical, _)| canonical_payment_method(canonical) == normalized)
            .map(|(canonical, _)| canonical)
            .or_else(|| {
                aliases.iter().find_map(|(canonical, variants)| {
                    variants
                        .iter()
                        .any(|alias| canonical_payment_method(alias) == normalized)
                        .then_some(canonical)
                })
            });
        if let Some(canonical) = canonical {
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
    provider_snapshots: Arc<RwLock<HashMap<String, CachedSearch>>>,
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
    pub(crate) provider_coefficient_cache: Arc<RwLock<HashMap<String, CachedProviderCoefficient>>>,
    pub(crate) fiat_quote_cache: Arc<RwLock<HashMap<String, CachedFiatQuote>>>,
    pub(crate) quote_refreshes: Arc<Mutex<HashSet<String>>>,
    background_pipeline_semaphore: Arc<Semaphore>,
    background_last_started: Arc<Mutex<Option<Instant>>>,
    background_provider_semaphore: Arc<Semaphore>,
    pub(crate) redis: Option<RedisPool>,
    pair_popularity: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) provider_capabilities_cache: Arc<RwLock<Option<ProviderCapabilitiesSnapshot>>>,
    pub(crate) service_latencies: ServiceLatencyTracker,
    fmatch: Option<FmatchP2pBackend>,
}

#[derive(Clone)]
pub(crate) struct CachedProviderQuote {
    pub(crate) inserted_at: Instant,
    pub(crate) quote: PublicRouteQuote,
}

#[derive(Clone, Copy)]
pub(crate) struct CachedProviderCoefficient {
    pub(crate) inserted_at: Instant,
    pub(crate) output_per_input: Option<f64>,
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
const MAX_BACKGROUND_PROVIDER_REQUESTS: usize = 5;

#[derive(Clone)]
struct CachedSearch {
    inserted_at: Instant,
    response: P2pSearchResponse,
}

impl P2pSearchService {
    /// Continuously warm provider observations at a bounded rate. The cursor
    /// advances lazily, so a large currency catalog does not become a queued
    /// collection of tasks in memory.
    pub fn start_background_warmup(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            service.run_background_warmup().await;
        });
    }

    async fn run_background_warmup(&self) {
        let fiats = self.fiat_intermediaries.to_vec();
        let capabilities = Self::provider_capabilities(&self.route_providers).await;
        let mut route_provider_assets = capabilities
            .iter()
            .map(|capability| {
                let mut assets = capability.assets.iter().cloned().collect::<Vec<_>>();
                assets.sort_by_key(ToString::to_string);
                (capability.provider.clone(), assets)
            })
            .collect::<Vec<_>>();
        route_provider_assets.sort_by_key(|(provider, _)| provider.name().to_string());
        let mut fiat_pairs = Vec::new();
        for provider in self.fiat_route_providers.iter() {
            for source in &fiats {
                for target in fiats
                    .iter()
                    .filter(|target| !source.eq_ignore_ascii_case(target))
                {
                    if provider.supports_pair(source, target) {
                        fiat_pairs.push((provider.clone(), source.clone(), target.clone()));
                    }
                }
            }
        }
        let mut assets = self.networks.assets();
        assets.extend(self.default_assets.iter().cloned());
        assets.sort();
        assets.dedup();
        if !self.enabled
            || (fiats.is_empty() && fiat_pairs.is_empty() && route_provider_assets.is_empty())
            || (assets.is_empty() && fiat_pairs.is_empty() && route_provider_assets.is_empty())
        {
            tracing::info!("background provider warmup is disabled or has no catalog");
            return;
        }
        tracing::info!(
            fiat_count = fiats.len(),
            fiat_exchange_pairs = fiat_pairs.len(),
            public_route_provider_count = route_provider_assets.len(),
            asset_count = assets.len(),
            p2p_pair_observations = fiats.len().saturating_mul(assets.len()).saturating_mul(2),
            max_active = MAX_BACKGROUND_SEARCHES,
            interval_ms = BACKGROUND_SEARCH_INTERVAL.as_millis(),
            "background provider warmup started"
        );

        let mut active: FuturesUnordered<BoxFuture<'static, ()>> = FuturesUnordered::new();
        let mut fiat_index = 0;
        let mut asset_index = 0;
        let mut side = P2pSide::BuyCrypto;
        let mut poll_ticks = 0;
        let mut fiat_pair_index: usize = 0;
        let mut route_provider_index = 0;
        let mut route_from_index = 0;
        let mut route_to_index = 1;
        let mut popular_pair_cursor = 0;
        let mut interval = tokio::time::interval(BACKGROUND_SEARCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Do not queue work while the five background slots are
                    // occupied. The current catalog cursor is retried next tick.
                    if active.len() < MAX_BACKGROUND_SEARCHES {
                        let refresh_p2p = !fiats.is_empty()
                            && !assets.is_empty()
                            && poll_ticks % BACKGROUND_POPULAR_PAIR_CADENCE
                                == BACKGROUND_POPULAR_PAIR_CADENCE - 1;
                        let popular = if refresh_p2p {
                            self.next_popular_pair(&mut popular_pair_cursor)
                        } else {
                            None
                        };
                        let used_popular_pair = popular.is_some();
                        poll_ticks = poll_ticks.saturating_add(1);
                        let due_fiat_pair = (!refresh_p2p).then(|| {
                            (0..fiat_pairs.len()).find_map(|offset| {
                                let index = fiat_pair_index.wrapping_add(offset) % fiat_pairs.len();
                                let (provider, source, target) = &fiat_pairs[index];
                                self.background_fiat_quote_is_due(
                                    provider.name(),
                                    source,
                                    target,
                                )
                                .then(|| {
                                    fiat_pair_index = index.wrapping_add(1);
                                    fiat_pairs[index].clone()
                                })
                            })
                        }).flatten();
                        if let Some((provider, source, target)) = due_fiat_pair {
                            let Some(permit) = self.try_start_background_pipeline() else {
                                continue;
                            };
                            let service = self.clone();
                            active.push(Box::pin(async move {
                                let _permit = permit;
                                if let Err(error) = service
                                    .refresh_background_fiat_quote(provider, source.clone(), target.clone())
                                    .await
                                {
                                    tracing::debug!(%error, %source, %target, "background fiat exchange coefficient refresh failed");
                                }
                            }));
                            continue;
                        }
                        let due_route_pair = if !refresh_p2p {
                            self.next_due_route_provider_pair(
                                &route_provider_assets,
                                &mut route_provider_index,
                                &mut route_from_index,
                                &mut route_to_index,
                            )
                        } else {
                            None
                        };
                        if let Some((provider, from, to)) = due_route_pair {
                            let Some(permit) = self.try_start_background_pipeline() else {
                                continue;
                            };
                            let service = self.clone();
                            active.push(Box::pin(async move {
                                let _permit = permit;
                                let provider_name = provider.name().to_string();
                                if let Err(error) = service
                                    .refresh_background_provider_coefficient(
                                        provider,
                                        from.clone(),
                                        to.clone(),
                                    )
                                    .await
                                {
                                    service.mark_background_provider_coefficient_failed(
                                        &provider_name,
                                        &from,
                                        &to,
                                    );
                                    tracing::debug!(%error, %from, %to, "background route provider coefficient refresh failed");
                                }
                            }));
                            continue;
                        }
                        if fiats.is_empty() || assets.is_empty() {
                            continue;
                        }
                        let (query, fiat, asset, scheduled_side) = if let Some(query) = popular {
                            let fiat = query.fiat.clone();
                            let asset = query.asset.clone();
                            let side = query.side;
                            (query, fiat, asset, side)
                        } else {
                            let fiat = fiats[fiat_index].clone();
                            let asset = assets[asset_index].clone();
                            let query = P2pSearchQuery {
                                fiat: fiat.clone(),
                                asset: asset.clone(),
                                side,
                                amount: None,
                                payment_method: None,
                                merchant_only: None,
                                min_orders: None,
                                min_completion_rate: None,
                limit: Some(crate::p2p::routes::LEG_SEARCH_LIMIT),
                                sources: None,
                            };
                            (query, fiat, asset, side)
                        };
                        let Some(permit) = self.try_start_background_pipeline() else {
                            continue;
                        };
                        let service = self.clone();
                        active.push(Box::pin(async move {
                            let _permit = permit;
                            if let Err(error) = service.refresh_background_search(query).await {
                                tracing::warn!(%error, %fiat, %asset, ?scheduled_side, "background provider observation failed");
                            }
                        }));

                        if !used_popular_pair {
                            if side == P2pSide::BuyCrypto {
                                side = P2pSide::SellCrypto;
                            } else {
                                side = P2pSide::BuyCrypto;
                                asset_index += 1;
                                if asset_index == assets.len() {
                                    asset_index = 0;
                                    fiat_index = (fiat_index + 1) % fiats.len();
                                }
                            }
                        }
                    }
                }
                _ = active.next(), if !active.is_empty() => {}
            }
        }
    }

    pub(crate) fn try_start_background_pipeline(
        &self,
    ) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let permit = self
            .background_pipeline_semaphore
            .clone()
            .try_acquire_owned()
            .ok()?;
        let now = Instant::now();
        let mut last_started = self.background_last_started.lock().ok()?;
        if last_started.is_some_and(|last| now.duration_since(last) < BACKGROUND_SEARCH_INTERVAL) {
            return None;
        }
        *last_started = Some(now);
        Some(permit)
    }

    fn next_due_route_provider_pair(
        &self,
        catalogs: &[(Arc<dyn PublicRouteProvider>, Vec<Asset>)],
        provider_index: &mut usize,
        from_index: &mut usize,
        to_index: &mut usize,
    ) -> Option<(Arc<dyn PublicRouteProvider>, Asset, Asset)> {
        for _ in 0..catalogs.len() {
            if *provider_index >= catalogs.len() {
                *provider_index = 0;
                *from_index = 0;
                *to_index = 1;
            }
            let (provider, assets) = &catalogs[*provider_index];
            if assets.len() < 2 {
                *provider_index += 1;
                continue;
            }
            if *from_index >= assets.len() {
                *from_index = 0;
                *to_index = 1;
                *provider_index += 1;
                continue;
            }
            if *to_index >= assets.len() {
                *to_index = 0;
                *from_index += 1;
                continue;
            }
            if *from_index == *to_index {
                *to_index += 1;
                continue;
            }
            let from = assets[*from_index].clone();
            let to = assets[*to_index].clone();
            *to_index += 1;
            if self.background_provider_coefficient_is_due(provider.name(), &from, &to) {
                return Some((provider.clone(), from, to));
            }
            return None;
        }
        None
    }

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
        let mut fiat_intermediaries = config.route_source_fiats.clone();
        fiat_intermediaries.extend(records.iter().flat_map(|record| {
            record
                .config
                .as_ref()
                .and_then(|adapters| adapters.p2p.as_ref())
                .into_iter()
                .flat_map(|adapter| adapter.supported_fiats.iter().cloned())
        }));
        fiat_intermediaries.iter_mut().for_each(|fiat| {
            *fiat = fiat.trim().to_ascii_uppercase();
        });
        fiat_intermediaries.sort();
        fiat_intermediaries.dedup();
        Ok(Self {
            enabled: config.p2p_search_enabled,
            timeout,
            cache_ttl: Duration::from_millis(config.p2p_search_cache_ttl_ms.min(60_000)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            provider_snapshots: Arc::new(RwLock::new(HashMap::new())),
            sources: sources.into(),
            payment_method_aliases: Arc::new(payment_method_aliases),
            market_sources: market_sources.into(),
            default_assets: default_assets.into(),
            fiat_intermediaries: fiat_intermediaries.into(),
            networks,
            route_providers: route_providers.into(),
            fiat_route_providers: fiat_route_providers.into(),
            quote_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROVIDER_QUOTES)),
            provider_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            provider_coefficient_cache: Arc::new(RwLock::new(HashMap::new())),
            fiat_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            quote_refreshes: Arc::new(Mutex::new(HashSet::new())),
            background_pipeline_semaphore: Arc::new(Semaphore::new(MAX_BACKGROUND_SEARCHES)),
            background_last_started: Arc::new(Mutex::new(None)),
            background_provider_semaphore: Arc::new(Semaphore::new(
                MAX_BACKGROUND_PROVIDER_REQUESTS,
            )),
            redis: None,
            pair_popularity: Arc::new(Mutex::new(HashMap::new())),
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
            provider_snapshots: Arc::new(RwLock::new(HashMap::new())),
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
            provider_coefficient_cache: Arc::new(RwLock::new(HashMap::new())),
            fiat_quote_cache: Arc::new(RwLock::new(HashMap::new())),
            quote_refreshes: Arc::new(Mutex::new(HashSet::new())),
            background_pipeline_semaphore: Arc::new(Semaphore::new(MAX_BACKGROUND_SEARCHES)),
            background_last_started: Arc::new(Mutex::new(None)),
            background_provider_semaphore: Arc::new(Semaphore::new(
                MAX_BACKGROUND_PROVIDER_REQUESTS,
            )),
            redis: None,
            pair_popularity: Arc::new(Mutex::new(HashMap::new())),
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
        self.provider_coefficient_cache = Arc::new(RwLock::new(HashMap::new()));
        self.quote_refreshes = Arc::new(Mutex::new(HashSet::new()));
        self.provider_capabilities_cache = Arc::new(RwLock::new(None));
        self
    }

    pub fn with_redis(mut self, redis: Option<RedisPool>) -> Self {
        self.redis = redis;
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
        self.record_query_interest(&query);
        self.run_search(query, None, None).await
    }

    pub(crate) async fn search_market(
        &self,
        query: P2pSearchQuery,
        market: Option<P2pOfferMarket>,
    ) -> Result<P2pSearchResponse> {
        self.record_query_interest(&query);
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
        self.record_query_interest(&query);
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

    fn record_query_interest(&self, query: &P2pSearchQuery) {
        let Ok(query) = query.clone().normalize() else {
            return;
        };
        if !self.fiat_intermediaries.contains(&query.fiat)
            || !(self.default_assets.contains(&query.asset)
                || self.networks.assets().contains(&query.asset))
        {
            return;
        }
        let side = match query.side {
            P2pSide::BuyCrypto => "buy",
            P2pSide::SellCrypto => "sell",
        };
        let key = format!("{}|{}|{side}", query.fiat, query.asset);
        let Ok(mut popularity) = self.pair_popularity.lock() else {
            return;
        };
        popularity.retain(|_, (_, seen_at)| seen_at.elapsed() < Duration::from_secs(24 * 60 * 60));
        if popularity.len() >= MAX_TRACKED_POPULAR_PAIRS && !popularity.contains_key(&key) {
            if let Some(coldest) = popularity
                .iter()
                .min_by_key(|(_, (count, seen_at))| (*count, std::cmp::Reverse(seen_at.elapsed())))
                .map(|(key, _)| key.clone())
            {
                popularity.remove(&coldest);
            }
        }
        let entry = popularity.entry(key).or_insert((0, Instant::now()));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = Instant::now();
    }

    fn next_popular_pair(&self, cursor: &mut usize) -> Option<P2pSearchQuery> {
        let mut popularity = self.pair_popularity.lock().ok()?;
        popularity.retain(|_, (_, seen_at)| seen_at.elapsed() < Duration::from_secs(24 * 60 * 60));
        let mut pairs = popularity
            .iter()
            .map(|(key, (count, seen_at))| {
                let age_penalty = 1.0 + seen_at.elapsed().as_secs_f64() / 300.0;
                (key.clone(), *count as f64 / age_penalty)
            })
            .collect::<Vec<_>>();
        pairs.sort_by(|left, right| right.1.total_cmp(&left.1));
        pairs.truncate(MAX_REFRESHED_POPULAR_PAIRS);
        if pairs.is_empty() {
            return None;
        }
        let pair = &pairs[*cursor % pairs.len()].0;
        *cursor = cursor.saturating_add(1);
        let mut parts = pair.split('|');
        let fiat = parts.next()?.to_string();
        let asset = parts.next()?.to_string();
        let side = match parts.next()? {
            "buy" => P2pSide::BuyCrypto,
            "sell" => P2pSide::SellCrypto,
            _ => return None,
        };
        Some(P2pSearchQuery {
            fiat,
            asset,
            side,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(crate::p2p::routes::LEG_SEARCH_LIMIT),
            sources: None,
        })
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
        if let Some(response) = self
            .provider_snapshot(&query, market)
            .filter(|response| !response.stale)
        {
            tracing::info!(
                fiat = %query.fiat,
                asset = %query.asset,
                side = ?query.side,
                offers_found = response.offers.len(),
                "p2p.background_snapshot.served"
            );
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
        if let Some(mut response) = self.provider_snapshot(&query, market) {
            for source in &mut response.sources {
                source.cached = true;
            }
            tracing::info!(
                fiat = %query.fiat,
                asset = %query.asset,
                side = ?query.side,
                offers_found = response.offers.len(),
                "p2p.background_snapshot.served_after_fmatch_miss"
            );
            if let Some(updates) = updates {
                let _ = updates.send(response.clone()).await;
            }
            return Ok(response);
        }
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
        self.run_search_once_with_mode(query, updates, market, false)
            .await
    }

    async fn run_search_once_with_mode(
        &self,
        query: P2pSearchQuery,
        updates: Option<mpsc::Sender<P2pSearchResponse>>,
        market: Option<P2pOfferMarket>,
        background: bool,
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
        if let Some(mut response) = self.provider_snapshot(&query, market) {
            response.cached = true;
            if let Some(updates) = &updates {
                let _ = updates.send(response.clone()).await;
            }
            return Ok(response);
        }
        let mut selected_sources = self
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
        if background {
            let priorities = self.background_provider_priorities().await;
            selected_sources.sort_by(|left, right| {
                let priority = |source: &Arc<dyn P2pSource>| {
                    priorities
                        .get(&source.name().to_ascii_lowercase())
                        .copied()
                        .unwrap_or((i16::MIN, f64::NEG_INFINITY, 0, 0))
                };
                let left = priority(left);
                let right = priority(right);
                right
                    .0
                    .cmp(&left.0)
                    .then_with(|| right.1.total_cmp(&left.1))
                    .then_with(|| right.2.cmp(&left.2))
                    .then_with(|| right.3.cmp(&left.3))
            });
        }
        let background_provider_semaphore = self.background_provider_semaphore.clone();
        let mut searches = selected_sources
            .into_iter()
            .map(|source| {
                let query = query.clone();
                let aliases = self.payment_method_aliases.get(source.name()).cloned();
                let background_provider_semaphore = background_provider_semaphore.clone();
                async move {
                    let _background_permit = if background {
                        match background_provider_semaphore.acquire_owned().await {
                            Ok(permit) => Some(permit),
                            Err(error) => {
                                return (
                                    Vec::new(),
                                    SourceStatus {
                                        source: source.name().to_string(),
                                        ok: false,
                                        cached: false,
                                        latency_ms: 0,
                                        offers_found: 0,
                                        error: Some(error.to_string()),
                                    },
                                );
                            }
                        }
                    } else {
                        None
                    };
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
            if query.amount.is_none()
                && query.payment_method.is_none()
                && query.merchant_only.is_none()
                && query.min_orders.is_none()
                && query.min_completion_rate.is_none()
            {
                self.cache_provider_snapshot(&query, market, response.clone());
            }
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

    async fn refresh_background_search(&self, query: P2pSearchQuery) -> Result<()> {
        let query = query.normalize()?;
        let exact_key = format!(
            "None:{}",
            serde_json::to_string(&query).context("failed to build background cache key")?
        );
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(&exact_key);
        }
        let snapshot_key = provider_snapshot_key(&query, None);
        if let Ok(mut snapshots) = self.provider_snapshots.write() {
            snapshots.remove(&snapshot_key);
        }
        let started = Instant::now();
        let response = self
            .run_search_once_with_mode(query.clone(), None, None, true)
            .await?;
        tracing::info!(
            fiat = %query.fiat,
            asset = %query.asset,
            side = ?query.side,
            offers_found = response.offers.len(),
            sources = response.sources.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "p2p.background_provider_pair.completed"
        );
        Ok(())
    }

    async fn background_provider_priorities(&self) -> HashMap<String, (i16, f64, i64, i64)> {
        let Some(redis) = self.redis.as_ref() else {
            return HashMap::new();
        };
        let cached = tokio::time::timeout(
            Duration::from_millis(50),
            get_json::<HashMap<String, ServiceStats>>(redis, "pay3flow:reputation:services:v2"),
        )
        .await
        .unwrap_or(Ok(None));
        let Ok(Some(stats)) = cached else {
            return HashMap::new();
        };
        stats
            .into_iter()
            .map(|(slug, stats)| {
                (
                    slug,
                    (
                        stats.reputation_score,
                        vote_quality_score(stats.likes_total, stats.dislikes_total)
                            .unwrap_or(f64::NEG_INFINITY),
                        stats.likes_total.saturating_add(stats.dislikes_total),
                        stats.executions_total,
                    ),
                )
            })
            .collect()
    }

    fn provider_snapshot(
        &self,
        query: &P2pSearchQuery,
        market: Option<P2pOfferMarket>,
    ) -> Option<P2pSearchResponse> {
        let snapshots = self.provider_snapshots.read().ok()?;
        let snapshot = snapshots
            .get(&provider_snapshot_key(query, market))
            .or_else(|| snapshots.get(&provider_snapshot_key(query, None)))?;
        (snapshot.inserted_at.elapsed() <= PROVIDER_SNAPSHOT_TTL
            && query.fetch_limit() <= snapshot.response.query.fetch_limit())
        .then(|| {
            let offers = snapshot
                .response
                .offers
                .iter()
                .filter(|offer| {
                    offer.matches(query) && market.is_none_or(|market| offer.market == market)
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut sources: Vec<SourceStatus> = snapshot
                .response
                .sources
                .iter()
                .filter(|source| {
                    query.sources.as_deref().is_none_or(|requested| {
                        requested.split(',').any(|name| name == source.source)
                    })
                })
                .cloned()
                .collect();
            for source in &mut sources {
                source.cached = true;
            }
            build_search_response(
                query.clone(),
                &offers,
                sources,
                true,
                "provider_snapshot",
                snapshot.inserted_at.elapsed() > PROVIDER_SNAPSHOT_STALE_AFTER,
                snapshot.response.observed_at.clone(),
            )
        })
    }

    fn cache_provider_snapshot(
        &self,
        query: &P2pSearchQuery,
        market: Option<P2pOfferMarket>,
        response: P2pSearchResponse,
    ) {
        if let Ok(mut snapshots) = self.provider_snapshots.write() {
            snapshots.retain(|_, cached| cached.inserted_at.elapsed() <= PROVIDER_SNAPSHOT_TTL);
            let key = provider_snapshot_key(query, market);
            if snapshots.len() >= MAX_PROVIDER_SNAPSHOTS && !snapshots.contains_key(&key) {
                if let Some(oldest_key) = snapshots
                    .iter()
                    .min_by_key(|(_, cached)| cached.inserted_at)
                    .map(|(key, _)| key.clone())
                {
                    snapshots.remove(&oldest_key);
                }
            }
            snapshots.insert(
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

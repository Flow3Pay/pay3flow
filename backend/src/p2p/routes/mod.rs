use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::p2p::service::{
    normalize_sources, CachedFiatQuote, CachedProviderQuote, P2pOffer, P2pOfferMarket,
    P2pSearchQuery, P2pSearchResponse, P2pSearchService, P2pSide, PaymentMethodMatch, SourceStatus,
};
use crate::p2p::spot::CryptoTicker;
use crate::route_engine::{
    canonical_network_id, Amount, Asset, PublicRouteProvider, PublicRouteQuote,
};
use crate::service_reputation::{
    CombinedReputation, RouteFeedback, RouteServiceStats, ServiceLink,
};

const DEFAULT_ROUTE_LIMIT: usize = 20;
const MAX_ROUTE_LIMIT: usize = 100;
const LEG_SEARCH_LIMIT: usize = 60;
const DEFAULT_MAX_PRICE_DEVIATION_BPS: u32 = 1_000;
const MAX_PROVIDER_ASSETS: usize = 12;
const MAX_PROVIDER_NETWORK_PAIRS: usize = 32;
const MAX_PROVIDER_OFFERS_PER_LEG: usize = 8;
const PROVIDER_QUOTE_CACHE_TTL: Duration = Duration::from_secs(30);
const MAX_BACKGROUND_PROVIDER_REFRESHES_PER_SEARCH: usize = 8;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeMode {
    #[default]
    All,
    P2p,
    Exchanger,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pRouteSearchQuery {
    pub source_fiat: String,
    pub target_fiat: String,
    pub source_amount: f64,
    pub source_network: Option<String>,
    pub target_network: Option<String>,
    /// Legacy field. Crypto-to-crypto routes no longer use a fiat pivot.
    pub bridge_fiat: Option<String>,
    /// Backward-compatible alias for `intermediary_assets`.
    pub assets: Option<String>,
    /// Optional comma-separated crypto intermediaries for fiat-to-fiat routes.
    /// Defaults to the configured `p2p_search_assets` list when omitted.
    pub intermediary_assets: Option<String>,
    pub source_payment_method: Option<String>,
    pub target_payment_method: Option<String>,
    pub merchant_only: Option<bool>,
    pub min_orders: Option<u64>,
    pub min_completion_rate: Option<f64>,
    /// Default false: same-venue routes do not require an on-chain transfer.
    pub allow_cross_venue: Option<bool>,
    /// Reject prices too far from the median for that leg. Default 1000 (10%).
    pub max_price_deviation_bps: Option<u32>,
    pub limit: Option<usize>,
    /// Optional comma-separated list of P2P sources to query.
    pub sources: Option<String>,
    /// Limit results to P2P offers, exchanger routes, or search both (default).
    #[serde(default)]
    pub exchange_mode: ExchangeMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2pRoute {
    pub route_id: String,
    pub rank: usize,
    pub asset: String,
    /// Canonical network id for the crypto asset selected by the user.
    pub entry_network: Option<String>,
    pub source_network: Option<String>,
    pub target_network: Option<String>,
    pub source_fiat: String,
    pub source_amount: String,
    pub acquired_asset_amount: String,
    pub target_fiat: String,
    pub target_amount: String,
    /// Target-fiat units per one source-fiat unit.
    pub effective_rate: String,
    pub same_venue: bool,
    pub requires_asset_transfer: bool,
    pub transfer_fee_included: bool,
    pub route_kind: String,
    pub bridge_currency: Option<String>,
    pub market_path: Option<CryptoMarketPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_provider_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_quote_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub route_path: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub route_fees: Vec<RouteFee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_expires_at: Option<DateTime<Utc>>,
    /// True when both selected bank names were present in venue responses.
    /// False means at least one venue returned only opaque payment IDs.
    pub payment_methods_verified: bool,
    pub entry_offer: Option<P2pOffer>,
    pub exit_offer: Option<P2pOffer>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<RouteServiceStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation: Option<CombinedReputation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<RouteFeedback>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub service_links: Vec<ServiceLink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteFee {
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CryptoMarketPath {
    pub venue: String,
    pub source_pair: String,
    pub target_pair: String,
    pub source_rate: String,
    pub target_rate: String,
    pub intermediary_amount: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteAssetStatus {
    pub asset: String,
    pub entry_offers: usize,
    pub exit_offers: usize,
    pub routes_built: usize,
    /// False when route construction stopped after collecting a bounded
    /// top-ranked candidate set instead of enumerating every combination.
    pub routes_exhaustive: bool,
    pub can_exchange_to_target: bool,
    pub entry_sources: Vec<SourceStatus>,
    pub exit_sources: Vec<SourceStatus>,
    /// Internal discovery transport. This is deliberately not serialized as
    /// a venue: Fmatch distributes venue offers but is not itself a venue.
    #[serde(skip)]
    entry_discovery_source: Option<String>,
    #[serde(skip)]
    exit_discovery_source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2pRouteSearchResponse {
    pub search_id: Uuid,
    pub routes_found: usize,
    /// Whether `routes_found` is an exact count of every valid combination.
    pub routes_exhaustive: bool,
    pub searched_at: DateTime<Utc>,
    pub source_fiat: String,
    pub target_fiat: String,
    pub source_amount: String,
    pub assets_searched: Vec<String>,
    pub can_exchange_to_target: bool,
    pub routes: Vec<P2pRoute>,
    pub asset_statuses: Vec<RouteAssetStatus>,
    /// Discovery source for the route snapshot: Fmatch, database cache, or
    /// the legacy provider path used by local-only callers.
    pub source: String,
    pub stale: bool,
}

#[derive(Debug, Clone)]
struct NormalizedRouteQuery {
    source_currency: String,
    target_currency: String,
    source_amount: f64,
    source_network: Option<String>,
    target_network: Option<String>,
    assets: Vec<String>,
    assets_explicit: bool,
    source_payment_method: Option<String>,
    target_payment_method: Option<String>,
    merchant_only: bool,
    min_orders: Option<u64>,
    min_completion_rate: Option<f64>,
    allow_cross_venue: bool,
    max_price_deviation_bps: u32,
    limit: usize,
    sources: Option<String>,
    exchange_mode: ExchangeMode,
}

impl NormalizedRouteQuery {
    fn includes_p2p(&self) -> bool {
        matches!(self.exchange_mode, ExchangeMode::All | ExchangeMode::P2p)
    }

    fn includes_exchangers(&self) -> bool {
        matches!(
            self.exchange_mode,
            ExchangeMode::All | ExchangeMode::Exchanger
        )
    }

    fn offer_market(&self) -> Option<P2pOfferMarket> {
        match self.exchange_mode {
            ExchangeMode::All => None,
            ExchangeMode::P2p => Some(P2pOfferMarket::P2p),
            ExchangeMode::Exchanger => Some(P2pOfferMarket::DirectExchange),
        }
    }

    fn accepts_offer(&self, offer: &P2pOffer) -> bool {
        match offer.market {
            P2pOfferMarket::P2p => self.includes_p2p(),
            P2pOfferMarket::DirectExchange => self.includes_exchangers(),
        }
    }
}

#[derive(Clone)]
struct RouteProviderCapability {
    provider: Arc<dyn PublicRouteProvider>,
    assets: HashSet<Asset>,
}

impl RouteProviderCapability {
    fn supports(&self, from: &Asset, to: &Asset) -> bool {
        self.assets.contains(from) && self.assets.contains(to)
    }
}

struct FiatProviderQuoteJob {
    provider: Arc<dyn PublicRouteProvider>,
    from: Asset,
    to: Asset,
    amount: Amount,
    entry_offer: P2pOffer,
    exit_offers: Arc<[P2pOffer]>,
}

fn provider_quote_key(provider: &str, from: &Asset, to: &Asset, amount: &Amount) -> String {
    format!("route|{provider}|{from}|{to}|{}", amount.value)
}

fn fiat_quote_key(provider: &str, source: &str, target: &str, amount: f64) -> String {
    format!("fiat|{provider}|{source}|{target}|{amount:.2}")
}

fn provider_priority(provider: &str) -> u8 {
    match provider {
        "symbiosis" => 0,
        "bestchange" => 1,
        "cow-swap" => 2,
        "near-intents" => 3,
        _ => 4,
    }
}

fn network_priority(network: Option<&str>) -> u8 {
    match network {
        Some("ethereum") => 0,
        Some("tron") => 1,
        Some("bnb-smart-chain") => 2,
        Some("polygon-pos") => 3,
        Some("arbitrum-one") => 4,
        Some("optimism") => 5,
        Some("avalanche-c") => 6,
        Some("solana") => 7,
        _ => 8,
    }
}

async fn quote_all_provider_refs(
    providers: Arc<[RouteProviderCapability]>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Vec<(String, PublicRouteQuote)> {
    let mut searches = FuturesUnordered::new();
    for capability in providers
        .iter()
        .filter(|capability| capability.supports(&from, &to))
    {
        let provider = capability.provider.clone();
        let from = from.clone();
        let to = to.clone();
        let amount = amount.clone();
        searches.push(quote_provider(
            provider,
            from,
            to,
            amount,
            quote_semaphore.clone(),
        ));
    }
    let mut quotes = Vec::new();
    while let Some(quote) = searches.next().await {
        if let Some(quote) = quote {
            quotes.push(quote);
        }
    }
    quotes
}

async fn quote_provider(
    provider: Arc<dyn PublicRouteProvider>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Option<(String, PublicRouteQuote)> {
    let provider_name = provider.name().to_string();
    let from_label = from.to_string();
    let to_label = to.to_string();
    let amount_label = amount.value.clone();
    // The permit intentionally covers the whole external call, including its
    // timeout, so a slow provider cannot keep consuming request slots.
    let _permit = match quote_semaphore.acquire_owned().await {
        Ok(permit) => permit,
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                "provider quote semaphore closed"
            );
            return None;
        }
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        provider.quote(from, to, amount),
    )
    .await;
    match result {
        Ok(Ok(quote)) => Some((provider_name, quote)),
        Ok(Err(error)) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quote failed"
            );
            None
        }
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quote timed out"
            );
            None
        }
    }
}

async fn quote_provider_many(
    provider: Arc<dyn PublicRouteProvider>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Option<(String, Vec<PublicRouteQuote>)> {
    let provider_name = provider.name().to_string();
    let from_label = from.to_string();
    let to_label = to.to_string();
    let amount_label = amount.value.clone();
    let _permit = match quote_semaphore.acquire_owned().await {
        Ok(permit) => permit,
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                "provider quote semaphore closed"
            );
            return None;
        }
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        provider.quotes(from, to, amount),
    )
    .await;
    match result {
        Ok(Ok(quotes)) if !quotes.is_empty() => Some((provider_name, quotes)),
        Ok(Ok(_)) => None,
        Ok(Err(error)) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quotes failed"
            );
            None
        }
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quotes timed out"
            );
            None
        }
    }
}

struct FiatAssetProgress {
    asset: String,
    entry: P2pSearchResponse,
    exit: P2pSearchResponse,
}

impl P2pSearchService {
    pub async fn search_routes(
        &self,
        query: P2pRouteSearchQuery,
    ) -> Result<P2pRouteSearchResponse> {
        self.run_route_search(query, Uuid::new_v4(), None).await
    }

    pub(crate) async fn stream_routes(
        &self,
        query: P2pRouteSearchQuery,
        search_id: Uuid,
        updates: mpsc::Sender<P2pRouteSearchResponse>,
    ) -> Result<P2pRouteSearchResponse> {
        self.run_route_search(query, search_id, Some(updates)).await
    }

    async fn run_route_search(
        &self,
        query: P2pRouteSearchQuery,
        search_id: Uuid,
        updates: Option<mpsc::Sender<P2pRouteSearchResponse>>,
    ) -> Result<P2pRouteSearchResponse> {
        let started = Instant::now();
        let provider_assets = self.provider_assets().await;
        let provider_assets_elapsed = started.elapsed();
        let query = normalize_query(
            query,
            &self.default_assets,
            &self.networks,
            &provider_assets,
        )?;
        let mut routes = HashMap::new();
        let mut asset_statuses = Vec::new();
        let source_is_crypto =
            is_crypto_currency(&query.source_currency, &self.networks, &provider_assets);
        let target_is_crypto =
            is_crypto_currency(&query.target_currency, &self.networks, &provider_assets);
        match (source_is_crypto, target_is_crypto) {
            (false, false) => {
                if updates.is_some() {
                    let (progress_updates, mut progress_snapshots) = mpsc::channel(128);
                    let mut searches = query
                        .assets
                        .iter()
                        .map(|asset| {
                            let entry_query = leg_query(
                                &query.source_currency,
                                asset,
                                P2pSide::BuyCrypto,
                                Some(query.source_amount),
                                query.source_payment_method.clone(),
                                &query,
                            );
                            let exit_query = leg_query(
                                &query.target_currency,
                                asset,
                                P2pSide::SellCrypto,
                                None,
                                query.target_payment_method.clone(),
                                &query,
                            );
                            stream_fiat_asset_search(
                                self,
                                asset.clone(),
                                entry_query,
                                exit_query,
                                query.offer_market(),
                                progress_updates.clone(),
                            )
                        })
                        .collect::<FuturesUnordered<_>>();
                    drop(progress_updates);
                    let mut sources_seen = HashMap::new();

                    while !searches.is_empty() {
                        tokio::select! {
                            progress = progress_snapshots.recv() => {
                                let Some(progress) = progress else { continue };
                                let counts = (progress.entry.sources.len(), progress.exit.sources.len());
                                if sources_seen.get(&progress.asset) == Some(&counts) {
                                    continue;
                                }
                                sources_seen.insert(progress.asset.clone(), counts);
                                apply_fiat_asset_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &progress.asset,
                                    &progress.entry,
                                    &progress.exit,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = searches.next() => {
                                let Some(result) = result else { break };
                                let (asset, entry, exit) = result?;
                                let counts = (entry.sources.len(), exit.sources.len());
                                if sources_seen.get(&asset) != Some(&counts) {
                                    sources_seen.insert(asset.clone(), counts);
                                    apply_fiat_asset_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &entry,
                                        &exit,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                        }
                    }
                } else {
                    let mut searches = query
                        .assets
                        .iter()
                        .map(|asset| async {
                            let entry_query = leg_query(
                                &query.source_currency,
                                asset,
                                P2pSide::BuyCrypto,
                                Some(query.source_amount),
                                query.source_payment_method.clone(),
                                &query,
                            );
                            let exit_query = leg_query(
                                &query.target_currency,
                                asset,
                                P2pSide::SellCrypto,
                                None,
                                query.target_payment_method.clone(),
                                &query,
                            );
                            let market = query.offer_market();
                            let (entry, exit) = tokio::join!(
                                self.search_market(entry_query, market),
                                self.search_market(exit_query, market)
                            );
                            (asset.clone(), entry, exit)
                        })
                        .collect::<FuturesUnordered<_>>();

                    while let Some((asset, entry, exit)) = searches.next().await {
                        apply_fiat_asset_response(
                            &mut routes,
                            &mut asset_statuses,
                            &query,
                            &asset,
                            &entry?,
                            &exit?,
                        );
                    }
                }
            }
            (false, true) => {
                let asset = query.target_currency.clone();
                let leg = leg_query(
                    &query.source_currency,
                    &asset,
                    P2pSide::BuyCrypto,
                    Some(query.source_amount),
                    query.source_payment_method.clone(),
                    &query,
                );
                if updates.is_some() {
                    let (leg_updates, mut leg_snapshots) = mpsc::channel(16);
                    let search = self.stream_search_market(leg, leg_updates, query.offer_market());
                    tokio::pin!(search);
                    let mut sources_seen = 0;
                    loop {
                        tokio::select! {
                            response = leg_snapshots.recv() => {
                                let Some(response) = response else { continue };
                                sources_seen = response.sources.len();
                                apply_fiat_to_crypto_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &asset,
                                    &response,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = &mut search => {
                                let response = result?;
                                if response.sources.len() > sources_seen {
                                    apply_fiat_to_crypto_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &response,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                                break;
                            }
                        }
                    }
                } else {
                    let response = self.search_market(leg, query.offer_market()).await?;
                    apply_fiat_to_crypto_response(
                        &mut routes,
                        &mut asset_statuses,
                        &query,
                        &asset,
                        &response,
                    );
                }
            }
            (true, false) => {
                let asset = query.source_currency.clone();
                let leg = leg_query(
                    &query.target_currency,
                    &asset,
                    P2pSide::SellCrypto,
                    None,
                    query.target_payment_method.clone(),
                    &query,
                );
                if updates.is_some() {
                    let (leg_updates, mut leg_snapshots) = mpsc::channel(16);
                    let search = self.stream_search_market(leg, leg_updates, query.offer_market());
                    tokio::pin!(search);
                    let mut sources_seen = 0;
                    loop {
                        tokio::select! {
                            response = leg_snapshots.recv() => {
                                let Some(response) = response else { continue };
                                sources_seen = response.sources.len();
                                apply_crypto_to_fiat_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &asset,
                                    &response,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = &mut search => {
                                let response = result?;
                                if response.sources.len() > sources_seen {
                                    apply_crypto_to_fiat_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &response,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                                break;
                            }
                        }
                    }
                } else {
                    let response = self.search_market(leg, query.offer_market()).await?;
                    apply_crypto_to_fiat_response(
                        &mut routes,
                        &mut asset_statuses,
                        &query,
                        &asset,
                        &response,
                    );
                }
            }
            (true, true) if query.includes_exchangers() => {
                let source_asset = query.source_currency.clone();
                let target_asset = query.target_currency.clone();
                if updates.is_some() {
                    let (market_updates, mut market_snapshots) = mpsc::channel(8);
                    let search =
                        self.stream_market_tickers(query.sources.as_deref(), market_updates);
                    tokio::pin!(search);
                    // Provider quotes and spot paths are independent. Neither
                    // is allowed to hold back the other's first snapshot.
                    let provider_search = self.search_provider_routes(&query);
                    tokio::pin!(provider_search);
                    let mut providers_finished = false;
                    let mut market_finished = false;
                    loop {
                        if providers_finished && market_finished {
                            break;
                        }
                        tokio::select! {
                            result = market_snapshots.recv() => {
                                let Some(result) = result else { continue };
                                if apply_crypto_market_result(
                                    &mut routes,
                                    &query,
                                    &source_asset,
                                    &target_asset,
                                    result,
                                ) {
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                            () = &mut search, if !market_finished => {
                                while let Ok(result) = market_snapshots.try_recv() {
                                    if apply_crypto_market_result(
                                        &mut routes,
                                        &query,
                                        &source_asset,
                                        &target_asset,
                                        result,
                                    ) {
                                        publish_update(
                                            updates.as_ref(),
                                            response_snapshot(search_id, &query, &routes, &asset_statuses),
                                        ).await;
                                    }
                                }
                                market_finished = true;
                            }
                            provider_routes = &mut provider_search, if !providers_finished => {
                                providers_finished = true;
                                if merge_routes(&mut routes, provider_routes) > 0 {
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                        }
                    }
                } else {
                    let (market_results, provider_routes) = tokio::join!(
                        self.search_market_tickers(query.sources.as_deref()),
                        self.search_provider_routes(&query),
                    );
                    for result in market_results {
                        apply_crypto_market_result(
                            &mut routes,
                            &query,
                            &source_asset,
                            &target_asset,
                            result,
                        );
                    }
                    merge_routes(&mut routes, provider_routes);
                }
            }
            (true, true) => {}
        }
        let provider_search_started = Instant::now();
        let mut provider_routes_exhaustive = true;
        let mut provider_routes = if query.includes_exchangers() {
            match (source_is_crypto, target_is_crypto) {
                (false, true) => self.search_fiat_to_crypto_provider_routes(&query).await,
                (true, false) => self.search_crypto_to_fiat_provider_routes(&query).await,
                (false, false) => {
                    let (routes, exhaustive) = self.search_fiat_provider_routes(&query).await;
                    provider_routes_exhaustive = exhaustive;
                    routes
                }
                (true, true) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        if query.includes_exchangers() && !source_is_crypto && !target_is_crypto {
            provider_routes.extend(self.search_direct_fiat_routes(&query).await);
        }
        if merge_routes(&mut routes, provider_routes) > 0 {
            publish_update(
                updates.as_ref(),
                response_snapshot(search_id, &query, &routes, &asset_statuses),
            )
            .await;
        }
        let mut response = response_snapshot(search_id, &query, &routes, &asset_statuses);
        response.routes_exhaustive &= provider_routes_exhaustive;
        tracing::info!(
            source_currency = %query.source_currency,
            target_currency = %query.target_currency,
            routes_found = response.routes_found,
            routes_exhaustive = response.routes_exhaustive,
            provider_assets_ms = provider_assets_elapsed.as_millis(),
            local_search_ms = provider_search_started.duration_since(started).as_millis(),
            provider_search_ms = provider_search_started.elapsed().as_millis(),
            total_ms = started.elapsed().as_millis(),
            "p2p.route_search.completed"
        );
        Ok(response)
    }

    async fn search_provider_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let source_networks = self.assets_on_network(
            &query.source_currency,
            query.source_network.as_deref(),
            &provider_assets,
        );
        let target_networks = self.assets_on_network(
            &query.target_currency,
            query.target_network.as_deref(),
            &provider_assets,
        );
        let network_pairs = source_networks
            .iter()
            .flat_map(|source| {
                target_networks
                    .iter()
                    .map(move |target| (source.clone(), target.clone()))
            })
            .filter(|(source, target)| {
                !(source.location == target.location
                    && query
                        .source_currency
                        .eq_ignore_ascii_case(&query.target_currency))
            })
            .collect::<Vec<_>>();
        let mut searches = FuturesUnordered::new();
        for capability in capabilities.iter() {
            for (source, target) in network_pairs
                .iter()
                .filter(|(source, target)| capability.supports(source, target))
                .take(MAX_PROVIDER_NETWORK_PAIRS)
            {
                let from = source.clone();
                let to = target.clone();
                let Ok(amount) = Amount::from_f64(query.source_amount, from.clone()) else {
                    continue;
                };
                searches.push(quote_provider_many(
                    capability.provider.clone(),
                    from,
                    to,
                    amount,
                    self.quote_semaphore.clone(),
                ));
            }
        }

        let mut routes = Vec::new();
        while let Some(result) = searches.next().await {
            let Some((provider_name, quotes)) = result else {
                continue;
            };
            for quote in quotes {
                let Ok(input_value) = quote.input.value.parse::<f64>() else {
                    continue;
                };
                let Ok(output_value) = quote.output.value.parse::<f64>() else {
                    continue;
                };
                if !input_value.is_finite() || !output_value.is_finite() || output_value <= 0.0 {
                    continue;
                }
                let source_network = quote.from.location.clone();
                let target_network = quote.to.location.clone();
                let mut warnings = vec![format!(
                "Live dry quote from {provider_name}; execution and wallet compatibility are not verified."
            )];
                if let Some(description) = quote.description.as_deref() {
                    warnings.push(format!("Quoted exchanger: {description}."));
                }
                if source_network != target_network {
                    warnings.push("Cross-network transfer requires the provider's deposit and withdrawal flow; confirm addresses, memos, network fees, and finality before sending.".into());
                }
                routes.push(P2pRoute {
                    route_id: String::new(),
                    rank: 0,
                    asset: quote.to.symbol.clone(),
                    entry_network: source_network.clone(),
                    source_network,
                    target_network,
                    source_fiat: quote.from.symbol.clone(),
                    source_amount: quote.input.value.clone(),
                    acquired_asset_amount: quote.output.value.clone(),
                    target_fiat: quote.to.symbol.clone(),
                    target_amount: quote.output.value,
                    effective_rate: fixed(output_value / input_value, 12),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: !quote.fees.is_empty(),
                    route_kind: "crypto_to_crypto".into(),
                    bridge_currency: None,
                    market_path: None,
                    route_provider: Some(provider_name.clone()),
                    route_provider_url: quote.source_url,
                    provider_quote_id: quote.quote_id.clone(),
                    route_path: quote
                        .path
                        .into_iter()
                        .map(|asset| asset.to_string())
                        .collect(),
                    route_fees: quote
                        .fees
                        .into_iter()
                        .map(|fee| RouteFee {
                            asset: fee.asset.to_string(),
                            amount: fee.value,
                        })
                        .collect(),
                    quote_expires_at: quote.expires_at,
                    payment_methods_verified: true,
                    entry_offer: None,
                    exit_offer: None,
                    warnings,
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
            }
        }
        routes
    }

    async fn provider_assets(&self) -> Vec<Asset> {
        if self.has_fmatch_backend() {
            self.warm_provider_capabilities();
            return self.cached_provider_assets();
        }
        Self::provider_assets_for(&self.route_providers).await
    }

    async fn provider_capabilities(
        providers: &[Arc<dyn PublicRouteProvider>],
    ) -> Arc<[RouteProviderCapability]> {
        let mut capabilities = Vec::with_capacity(providers.len());
        for provider in providers {
            capabilities.push(RouteProviderCapability {
                provider: provider.clone(),
                assets: provider.supported_assets().await.into_iter().collect(),
            });
        }
        capabilities.sort_by_key(|capability| provider_priority(capability.provider.name()));
        capabilities.into()
    }

    pub(crate) fn warm_provider_capabilities(&self) {
        if self
            .provider_capabilities_cache
            .read()
            .is_ok_and(|cache| cache.is_some())
        {
            return;
        }
        let refresh_key = "provider-capabilities".to_string();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let providers = self.route_providers.clone();
        let cache = self.provider_capabilities_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        tokio::spawn(async move {
            let mut loads = providers
                .iter()
                .cloned()
                .map(|provider| async move {
                    let name = provider.name().to_string();
                    let assets = provider.supported_assets().await.into_iter().collect();
                    (name, assets)
                })
                .collect::<FuturesUnordered<_>>();
            let mut snapshot = HashMap::new();
            while let Some((name, assets)) = loads.next().await {
                snapshot.insert(name, assets);
            }
            if let Ok(mut cache) = cache.write() {
                *cache = Some(snapshot);
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }

    fn cached_provider_assets(&self) -> Vec<Asset> {
        let mut assets = self
            .provider_capabilities_cache
            .read()
            .ok()
            .and_then(|cache| cache.clone())
            .into_iter()
            .flat_map(|snapshot| snapshot.into_values().flatten())
            .collect::<Vec<_>>();
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    async fn provider_capabilities_for_query(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Arc<[RouteProviderCapability]> {
        let providers = self.route_providers_for_query(query);
        if !self.has_fmatch_backend() {
            return Self::provider_capabilities(&providers).await;
        }
        self.warm_provider_capabilities();
        let snapshot = self
            .provider_capabilities_cache
            .read()
            .ok()
            .and_then(|cache| cache.clone());
        let Some(snapshot) = snapshot else {
            return Vec::new().into();
        };
        let mut capabilities = providers
            .iter()
            .filter_map(|provider| {
                snapshot
                    .get(provider.name())
                    .cloned()
                    .map(|assets| RouteProviderCapability {
                        provider: provider.clone(),
                        assets,
                    })
            })
            .collect::<Vec<_>>();
        capabilities.sort_by_key(|capability| provider_priority(capability.provider.name()));
        capabilities.into()
    }

    fn cached_provider_quote(&self, key: &str) -> Option<PublicRouteQuote> {
        self.provider_quote_cache
            .read()
            .ok()?
            .get(key)
            .filter(|cached| {
                cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL
                    && cached
                        .quote
                        .expires_at
                        .is_none_or(|expires_at| expires_at > Utc::now())
            })
            .map(|cached| cached.quote.clone())
    }

    fn refresh_provider_quote(&self, key: String, job: &FiatProviderQuoteJob) {
        let refresh_key = key.clone();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let provider = job.provider.clone();
        let from = job.from.clone();
        let to = job.to.clone();
        let amount = job.amount.clone();
        let quote_semaphore = self.quote_semaphore.clone();
        let cache = self.provider_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        let Ok(permit) = quote_semaphore.try_acquire_owned() else {
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };
        tokio::spawn(async move {
            let provider_name = provider.name().to_string();
            let result =
                tokio::time::timeout(Duration::from_secs(12), provider.quote(from, to, amount))
                    .await;
            drop(permit);
            match result {
                Ok(Ok(quote)) => {
                    if let Ok(mut cache) = cache.write() {
                        cache.insert(
                            key,
                            CachedProviderQuote {
                                inserted_at: Instant::now(),
                                quote,
                            },
                        );
                    }
                }
                Ok(Err(error)) => {
                    tracing::debug!(%error, provider = %provider_name, "background provider quote failed");
                }
                Err(_) => {
                    tracing::debug!(provider = %provider_name, "background provider quote timed out");
                }
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }

    fn cached_fiat_quote(&self, key: &str) -> Option<crate::p2p::FiatRouteQuote> {
        self.fiat_quote_cache
            .read()
            .ok()?
            .get(key)
            .filter(|cached| cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL)
            .map(|cached| cached.quote.clone())
    }

    fn refresh_fiat_quote(
        &self,
        key: String,
        provider: Arc<dyn crate::p2p::PublicFiatRouteProvider>,
        source_currency: String,
        target_currency: String,
        source_amount: f64,
    ) {
        let refresh_key = key.clone();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let cache = self.fiat_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        let quote_semaphore = self.quote_semaphore.clone();
        let Ok(permit) = quote_semaphore.try_acquire_owned() else {
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };
        tokio::spawn(async move {
            let quote = tokio::time::timeout(
                Duration::from_secs(12),
                provider.quote(&source_currency, &target_currency, source_amount),
            )
            .await
            .ok()
            .and_then(Result::ok);
            drop(permit);
            if let Some(quote) = quote {
                if let Ok(mut cache) = cache.write() {
                    cache.insert(
                        key,
                        CachedFiatQuote {
                            inserted_at: Instant::now(),
                            quote,
                        },
                    );
                }
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }

    async fn provider_assets_for(providers: &[Arc<dyn PublicRouteProvider>]) -> Vec<Asset> {
        let mut assets = Vec::new();
        for provider in providers {
            assets.extend(provider.supported_assets().await);
        }
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    fn route_providers_for_query(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Arc<[Arc<dyn PublicRouteProvider>]> {
        self.route_providers
            .iter()
            .filter(|provider| source_selected(query, provider.name()))
            .cloned()
            .collect::<Vec<_>>()
            .into()
    }

    async fn intermediary_assets(&self, query: &NormalizedRouteQuery) -> Vec<Asset> {
        let providers = self.route_providers_for_query(query);
        let provider_assets = if self.has_fmatch_backend() {
            self.warm_provider_capabilities();
            self.cached_provider_assets()
        } else {
            Self::provider_assets_for(&providers).await
        };
        let allowed_symbols = query.assets.iter().collect::<HashSet<_>>();
        let mut assets = if query.assets_explicit {
            provider_assets
                .into_iter()
                .filter(|asset| allowed_symbols.contains(&asset.symbol))
                .collect::<Vec<_>>()
        } else {
            provider_assets
        };
        if assets.is_empty() {
            assets = query
                .assets
                .iter()
                .flat_map(|symbol| {
                    self.networks
                        .compatible_networks(symbol)
                        .into_iter()
                        .filter_map(move |network| Asset::new(symbol, Some(&network.id)).ok())
                })
                .collect();
        }
        let target_symbol = query.target_currency.as_str();
        assets.sort_by(|left, right| {
            intermediary_asset_priority(&left.symbol, target_symbol)
                .cmp(&intermediary_asset_priority(&right.symbol, target_symbol))
                .then_with(|| left.symbol.cmp(&right.symbol))
                .then_with(|| left.to_string().cmp(&right.to_string()))
        });
        assets.dedup();
        assets.truncate(MAX_PROVIDER_ASSETS);
        assets
    }

    fn assets_on_network(
        &self,
        symbol: &str,
        network: Option<&str>,
        provider_assets: &[Asset],
    ) -> Vec<Asset> {
        let requested_network = network.map(canonical_network_id);
        let mut assets = provider_assets
            .iter()
            .filter(|asset| asset.symbol.eq_ignore_ascii_case(symbol))
            .filter(|asset| {
                requested_network
                    .as_deref()
                    .is_none_or(|network| asset.location.as_deref() == Some(network))
            })
            .cloned()
            .collect::<Vec<_>>();
        if assets.is_empty() {
            assets = self
                .networks
                .compatible_networks(symbol)
                .into_iter()
                .filter(|candidate| {
                    requested_network
                        .as_deref()
                        .is_none_or(|network| candidate.id == network)
                })
                .filter_map(|candidate| Asset::new(symbol, Some(&candidate.id)).ok())
                .collect();
        }
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    async fn search_fiat_to_crypto_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let target_assets = self.assets_on_network(
            &query.target_currency,
            query.target_network.as_deref(),
            &provider_assets,
        );
        if target_assets.is_empty() {
            return Vec::new();
        }
        let mut intermediary_groups = HashMap::<String, Vec<Asset>>::new();
        for intermediary in self.intermediary_assets(query).await {
            intermediary_groups
                .entry(intermediary.symbol.clone())
                .or_default()
                .push(intermediary);
        }
        let mut entry_searches = FuturesUnordered::new();
        for (symbol, intermediaries) in intermediary_groups {
            let service = self.clone();
            let query = query.clone();
            entry_searches.push(async move {
                let entry = service
                    .search_market(
                        leg_query(
                            &query.source_currency,
                            &symbol,
                            P2pSide::BuyCrypto,
                            Some(query.source_amount),
                            query.source_payment_method.clone(),
                            &query,
                        ),
                        query.offer_market(),
                    )
                    .await;
                (intermediaries, entry)
            });
        }
        let mut searches = FuturesUnordered::new();
        while let Some((intermediaries, entry)) = entry_searches.next().await {
            let Ok(entry) = entry else {
                continue;
            };
            for intermediary in &intermediaries {
                for offer in entry
                    .offers
                    .iter()
                    .filter(|offer| query.accepts_offer(offer))
                    .take(MAX_PROVIDER_OFFERS_PER_LEG)
                {
                    if !offer_matches_network(offer, intermediary.location.as_deref()) {
                        continue;
                    }
                    let Some(price) = positive_number(&offer.price) else {
                        continue;
                    };
                    let acquired = query.source_amount / price;
                    if positive_number(&offer.available_asset)
                        .is_none_or(|available| available < acquired)
                    {
                        continue;
                    }
                    for target in &target_assets {
                        if intermediary == target {
                            continue;
                        }
                        let Ok(amount) = Amount::from_f64(acquired, intermediary.clone()) else {
                            continue;
                        };
                        let capabilities = capabilities.clone();
                        let intermediary = intermediary.clone();
                        let target = target.clone();
                        let offer = offer.clone();
                        let query = query.clone();
                        let quote_semaphore = self.quote_semaphore.clone();
                        searches.push(async move {
                            quote_all_provider_refs(
                                capabilities,
                                intermediary,
                                target,
                                amount,
                                quote_semaphore,
                            )
                            .await
                                .into_iter()
                                .filter_map(|(provider, quote)| {
                                    let output = quote.output.value.parse::<f64>().ok()?;
                                    if !output.is_finite() || output <= 0.0 {
                                        return None;
                                    }
                                    let payment_methods_verified = query
                                        .source_payment_method
                                        .as_deref()
                                        .is_none_or(|method| {
                                            offer.payment_method_match(method)
                                                == PaymentMethodMatch::Exact
                                        });
                                    let mut warnings = vec![
                                        format!("Live dry quote from {provider}; execution and wallet compatibility are not verified."),
                                        "Fiat entry is a live P2P estimate; confirm payment details before sending.".into(),
                                    ];
                                    if !payment_methods_verified {
                                        warnings.push("The selected sender payment method could not be verified by the venue.".into());
                                    }
                                    Some(P2pRoute {
                                        route_id: String::new(),
                                        rank: 0,
                                        asset: quote.to.symbol.clone(),
                                        entry_network: quote.from.location.clone(),
                                        source_network: quote.from.location.clone(),
                                        target_network: quote.to.location.clone(),
                                        source_fiat: query.source_currency.clone(),
                                        source_amount: fixed(query.source_amount, 2),
                                        acquired_asset_amount: quote.output.value.clone(),
                                        target_fiat: query.target_currency.clone(),
                                        target_amount: quote.output.value.clone(),
                                        effective_rate: fixed(output / query.source_amount, 12),
                                        same_venue: false,
                                        requires_asset_transfer: true,
                                        transfer_fee_included: !quote.fees.is_empty(),
                                        route_kind: "fiat_to_crypto".into(),
                                        bridge_currency: None,
                                        market_path: None,
                                        route_provider: Some(provider),
                                        route_provider_url: quote.source_url,
                                        provider_quote_id: quote.quote_id.clone(),
                                        route_path: std::iter::once(query.source_currency.clone())
                                            .chain(quote.path.into_iter().map(|asset| asset.to_string()))
                                            .collect(),
                                        route_fees: quote
                                            .fees
                                            .into_iter()
                                            .map(|fee| RouteFee {
                                                asset: fee.asset.to_string(),
                                                amount: fee.value,
                                            })
                                            .collect(),
                                        quote_expires_at: quote.expires_at,
                                        payment_methods_verified,
                                        entry_offer: Some(offer.clone()),
                                        exit_offer: None,
                                        warnings,
                                        services: Vec::new(),
                                        reputation: None,
                                        feedback: None,
                                        service_links: Vec::new(),
                                    })
                                })
                                .collect::<Vec<_>>()
                        });
                    }
                }
            }
        }
        let mut routes = Vec::new();
        while let Some(batch) = searches.next().await {
            routes.extend(batch);
        }
        routes
    }

    async fn search_crypto_to_fiat_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let source_assets = self.assets_on_network(
            &query.source_currency,
            query.source_network.as_deref(),
            &provider_assets,
        );
        if source_assets.is_empty() {
            return Vec::new();
        }
        let mut searches = FuturesUnordered::new();
        for intermediary in self.intermediary_assets(query).await {
            let Ok(exit) = self
                .search_market(
                    leg_query(
                        &query.target_currency,
                        &intermediary.symbol,
                        P2pSide::SellCrypto,
                        None,
                        query.target_payment_method.clone(),
                        query,
                    ),
                    query.offer_market(),
                )
                .await
            else {
                continue;
            };
            let exit_offers = exit
                .offers
                .into_iter()
                .filter(|offer| query.accepts_offer(offer))
                .filter(|offer| offer_matches_network(offer, intermediary.location.as_deref()))
                .take(MAX_PROVIDER_OFFERS_PER_LEG)
                .collect::<Vec<_>>();
            for source in &source_assets {
                if *source == intermediary {
                    continue;
                }
                let Ok(amount) = Amount::from_f64(query.source_amount, source.clone()) else {
                    continue;
                };
                let capabilities = capabilities.clone();
                let source = source.clone();
                let intermediary = intermediary.clone();
                let exit_offers = exit_offers.clone();
                let source_currency = query.source_currency.clone();
                let target_currency = query.target_currency.clone();
                let source_amount = query.source_amount;
                let target_payment_method = query.target_payment_method.clone();
                let quote_semaphore = self.quote_semaphore.clone();
                searches.push(async move {
                    quote_all_provider_refs(
                        capabilities,
                        source,
                        intermediary,
                        amount,
                        quote_semaphore,
                    )
                    .await
                    .into_iter()
                    .flat_map(|(provider, quote)| {
                        let source_currency = source_currency.clone();
                        let target_currency = target_currency.clone();
                        let target_payment_method = target_payment_method.clone();
                        exit_offers.iter().filter_map(move |offer| {
                            let price = positive_number(&offer.price)?;
                            let output = quote.output.value.parse::<f64>().ok()?;
                            let target_amount = output * price;
                            if !output.is_finite()
                                || output <= 0.0
                                || !covers_target(offer, target_amount)
                                || positive_number(&offer.available_asset)
                                    .is_none_or(|available| available < output)
                            {
                                return None;
                            }
                            let payment_methods_verified = target_payment_method
                                .as_deref()
                                .is_none_or(|method| {
                                    offer.payment_method_match(method)
                                        == PaymentMethodMatch::Exact
                                });
                            let warnings = vec![
                                format!("Live dry quote from {provider}; execution and wallet compatibility are not verified."),
                                "Fiat exit is a live P2P estimate; confirm payment details before sending.".into(),
                            ];
                            let route_quote = quote.clone();
                            Some(P2pRoute {
                                route_id: String::new(),
                                rank: 0,
                                asset: route_quote.from.symbol.clone(),
                                entry_network: route_quote.from.location.clone(),
                                source_network: route_quote.from.location.clone(),
                                target_network: route_quote.to.location.clone(),
                                source_fiat: source_currency.clone(),
                                source_amount: fixed(source_amount, 8),
                                acquired_asset_amount: route_quote.output.value.clone(),
                                target_fiat: target_currency.clone(),
                                target_amount: fixed(target_amount, 2),
                                effective_rate: fixed(target_amount / source_amount, 12),
                                same_venue: false,
                                requires_asset_transfer: true,
                                transfer_fee_included: !route_quote.fees.is_empty(),
                                route_kind: "crypto_to_fiat".into(),
                                bridge_currency: None,
                                market_path: None,
                                route_provider: Some(provider.clone()),
                                route_provider_url: route_quote.source_url.clone(),
                                provider_quote_id: route_quote.quote_id.clone(),
                                route_path: route_quote
                                    .path
                                    .into_iter()
                                    .map(|asset| asset.to_string())
                                    .chain(std::iter::once(target_currency.clone()))
                                    .collect(),
                                route_fees: route_quote
                                    .fees
                                    .into_iter()
                                    .map(|fee| RouteFee {
                                        asset: fee.asset.to_string(),
                                        amount: fee.value,
                                    })
                                    .collect(),
                                quote_expires_at: route_quote.expires_at,
                                payment_methods_verified,
                                entry_offer: None,
                                exit_offer: Some(offer.clone()),
                                warnings,
                                services: Vec::new(),
                                reputation: None,
                                feedback: None,
                                service_links: Vec::new(),
                            })
                        })
                    })
                    .collect::<Vec<_>>()
                });
            }
        }
        let mut routes = Vec::new();
        while let Some(batch) = searches.next().await {
            routes.extend(batch);
        }
        routes
    }

    async fn search_fiat_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
    ) -> (Vec<P2pRoute>, bool) {
        let mut quote_jobs = Vec::new();
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let mut intermediary_symbols = if query.assets_explicit {
            query.assets.clone()
        } else {
            provider_assets
                .iter()
                .map(|asset| asset.symbol.clone())
                .collect::<Vec<_>>()
        };
        intermediary_symbols
            .sort_by_key(|symbol| intermediary_asset_priority(symbol, &query.target_currency));
        intermediary_symbols.dedup();
        intermediary_symbols.truncate(MAX_PROVIDER_ASSETS);
        for asset_symbol in intermediary_symbols {
            let source_networks = self.assets_on_network(
                &asset_symbol,
                query.source_network.as_deref(),
                &provider_assets,
            );
            let target_networks = self.assets_on_network(
                &asset_symbol,
                query.target_network.as_deref(),
                &provider_assets,
            );
            if source_networks.is_empty() || target_networks.is_empty() {
                continue;
            }
            let mut network_pairs = source_networks
                .iter()
                .flat_map(|source| {
                    target_networks
                        .iter()
                        .filter(move |target| source.location != target.location)
                        .map(move |target| (source.clone(), target.clone()))
                })
                .collect::<Vec<_>>();
            network_pairs.sort_by_key(|(from, to)| {
                (
                    network_priority(from.location.as_deref()),
                    network_priority(to.location.as_deref()),
                    from.to_string(),
                    to.to_string(),
                )
            });
            if network_pairs.is_empty() {
                continue;
            }
            let Ok(entry) = self
                .search_market(
                    leg_query(
                        &query.source_currency,
                        &asset_symbol,
                        P2pSide::BuyCrypto,
                        Some(query.source_amount),
                        query.source_payment_method.clone(),
                        query,
                    ),
                    query.offer_market(),
                )
                .await
            else {
                continue;
            };
            let Ok(exit) = self
                .search_market(
                    leg_query(
                        &query.target_currency,
                        &asset_symbol,
                        P2pSide::SellCrypto,
                        None,
                        query.target_payment_method.clone(),
                        query,
                    ),
                    query.offer_market(),
                )
                .await
            else {
                continue;
            };
            let exit_offers = Arc::<[P2pOffer]>::from(
                exit.offers
                    .into_iter()
                    .filter(|offer| query.accepts_offer(offer))
                    .take(MAX_PROVIDER_OFFERS_PER_LEG)
                    .collect::<Vec<_>>(),
            );
            for entry_offer in entry
                .offers
                .into_iter()
                .filter(|offer| query.accepts_offer(offer))
                .take(2)
            {
                let Some(entry_price) = positive_number(&entry_offer.price) else {
                    continue;
                };
                let source_amount = query.source_amount / entry_price;
                if positive_number(&entry_offer.available_asset)
                    .is_none_or(|available| available < source_amount)
                {
                    continue;
                }
                for (from, to) in network_pairs.iter().take(MAX_PROVIDER_NETWORK_PAIRS) {
                    for capability in capabilities
                        .iter()
                        .filter(|capability| capability.supports(from, to))
                    {
                        let from = from.clone();
                        let to = to.clone();
                        let Ok(amount) = Amount::new(fixed(source_amount, 6), from.clone()) else {
                            continue;
                        };
                        let entry_offer = entry_offer.clone();
                        let exit_offers = exit_offers.clone();
                        let provider = capability.provider.clone();
                        quote_jobs.push(FiatProviderQuoteJob {
                            provider,
                            from,
                            to,
                            amount,
                            entry_offer,
                            exit_offers,
                        });
                    }
                }
            }
        }

        let quote_jobs_total = quote_jobs.len();
        let quote_started = Instant::now();
        let mut quote_results = Vec::new();
        let mut refreshes_started = 0;
        for job in quote_jobs.into_iter().take(query.limit) {
            let provider_name = job.provider.name().to_string();
            let key = provider_quote_key(&provider_name, &job.from, &job.to, &job.amount);
            if let Some(quote) = self.cached_provider_quote(&key) {
                quote_results.push((provider_name, quote, job.entry_offer, job.exit_offers, true));
            } else if !self.has_fmatch_backend() {
                if let Some((provider_name, quote)) = quote_provider(
                    job.provider,
                    job.from,
                    job.to,
                    job.amount,
                    self.quote_semaphore.clone(),
                )
                .await
                {
                    quote_results.push((
                        provider_name,
                        quote,
                        job.entry_offer,
                        job.exit_offers,
                        true,
                    ));
                }
            } else {
                if refreshes_started < MAX_BACKGROUND_PROVIDER_REFRESHES_PER_SEARCH {
                    self.refresh_provider_quote(key, &job);
                    refreshes_started += 1;
                }
                let output = Amount::new(job.amount.value.clone(), job.to.clone())
                    .expect("provider quote job contains a validated amount");
                quote_results.push((
                    provider_name.clone(),
                    PublicRouteQuote {
                        provider: provider_name,
                        quote_id: None,
                        description: Some("capability snapshot estimate".into()),
                        source_url: None,
                        from: job.from.clone(),
                        to: job.to.clone(),
                        input: job.amount.clone(),
                        output,
                        fees: Vec::new(),
                        expires_at: None,
                        path: vec![job.from, job.to],
                    },
                    job.entry_offer,
                    job.exit_offers,
                    false,
                ));
            }
        }
        let mut routes = Vec::new();
        let provider_route_limit = query.limit.div_ceil(2).max(capabilities.len());
        for (provider_name, quote, entry, exit_offers, quote_confirmed) in quote_results {
            let Ok(output_asset) = quote.output.value.parse::<f64>() else {
                continue;
            };
            if !output_asset.is_finite() || output_asset <= 0.0 {
                continue;
            }
            for exit in exit_offers.iter() {
                let Some(exit_price) = positive_number(&exit.price) else {
                    continue;
                };
                let target_amount = output_asset * exit_price;
                if !covers_target(exit, target_amount)
                    || positive_number(&exit.available_asset)
                        .is_none_or(|available| available < output_asset)
                {
                    continue;
                }
                let route_quote = quote.clone();
                routes.push(P2pRoute {
                    route_id: String::new(),
                    rank: 0,
                    asset: exit.asset.clone(),
                    entry_network: route_quote.from.location.clone(),
                    source_network: route_quote.from.location.clone(),
                    target_network: route_quote.to.location.clone(),
                    source_fiat: query.source_currency.clone(),
                    source_amount: fixed(query.source_amount, 2),
                    acquired_asset_amount: fixed(output_asset, 8),
                    target_fiat: query.target_currency.clone(),
                    target_amount: fixed(target_amount, 2),
                    effective_rate: fixed(target_amount / query.source_amount, 8),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: !route_quote.fees.is_empty(),
                    route_kind: "fiat_to_fiat".into(),
                    bridge_currency: None,
                    market_path: None,
                    route_provider: Some(provider_name.clone()),
                    route_provider_url: route_quote.source_url.clone(),
                    provider_quote_id: route_quote.quote_id.clone(),
                    route_path: std::iter::once(query.source_currency.clone())
                        .chain(route_quote.path.into_iter().map(|asset| asset.to_string()))
                        .chain(std::iter::once(query.target_currency.clone()))
                        .collect(),
                    route_fees: route_quote
                        .fees
                        .into_iter()
                        .map(|fee| RouteFee {
                            asset: fee.asset.to_string(),
                            amount: fee.value,
                        })
                        .collect(),
                    quote_expires_at: route_quote.expires_at,
                    payment_methods_verified: true,
                    entry_offer: Some(entry.clone()),
                    exit_offer: Some(exit.clone()),
                    warnings: vec![
                        if quote_confirmed {
                            "Recent fiat entry/exit offers plus a cached dry cross-network quote; execution still requires a fresh quote.".into()
                        } else {
                            "Capability-based cross-network estimate; provider fees and live output are refreshed asynchronously and must be confirmed before execution.".into()
                        },
                        "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.".into(),
                    ],
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
            }
            if routes.len() >= provider_route_limit {
                break;
            }
        }
        tracing::info!(
            quote_jobs_total,
            refreshes_started,
            routes_found = routes.len(),
            elapsed_ms = quote_started.elapsed().as_millis(),
            "p2p.provider_route_search.completed"
        );
        (routes, false)
    }

    async fn search_direct_fiat_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let mut quotes = Vec::new();
        for provider in self
            .fiat_route_providers
            .iter()
            .filter(|provider| source_selected(query, provider.name()))
            .cloned()
        {
            if !provider.supports_pair(&query.source_currency, &query.target_currency) {
                continue;
            }
            let source_currency = query.source_currency.clone();
            let target_currency = query.target_currency.clone();
            let source_amount = query.source_amount;
            let key = fiat_quote_key(
                provider.name(),
                &source_currency,
                &target_currency,
                source_amount,
            );
            if let Some(quote) = self.cached_fiat_quote(&key) {
                quotes.push(quote);
            } else if !self.has_fmatch_backend() {
                if let Ok(quote) = provider
                    .quote(&source_currency, &target_currency, source_amount)
                    .await
                {
                    quotes.push(quote);
                }
            } else {
                self.refresh_fiat_quote(
                    key,
                    provider,
                    source_currency,
                    target_currency,
                    source_amount,
                );
            }
        }

        let mut routes = Vec::new();
        for quote in quotes {
            if !quote.source_amount.is_finite()
                || quote.source_amount <= 0.0
                || !quote.target_amount.is_finite()
                || quote.target_amount <= 0.0
                || !quote
                    .source_currency
                    .eq_ignore_ascii_case(&query.source_currency)
                || !quote
                    .target_currency
                    .eq_ignore_ascii_case(&query.target_currency)
            {
                continue;
            }
            let price = quote.source_amount / quote.target_amount;
            let mut payment_methods = [
                query.source_payment_method.clone(),
                query.target_payment_method.clone(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            payment_methods.sort();
            payment_methods.dedup();
            let offer = P2pOffer {
                market: P2pOfferMarket::DirectExchange,
                source: quote.provider.clone(),
                ad_id: format!(
                    "indicative-{}-{}",
                    quote.source_currency.to_ascii_lowercase(),
                    quote.target_currency.to_ascii_lowercase()
                ),
                side: P2pSide::BuyCrypto,
                fiat: quote.source_currency.clone(),
                asset: quote.target_currency.clone(),
                network: None,
                price: fixed(price, 12),
                available_asset: "1000000000".into(),
                min_fiat: "1".into(),
                max_fiat: "1000000000".into(),
                payment_methods,
                pay_time_limit_minutes: None,
                advertiser: crate::p2p::service::Advertiser {
                    id: None,
                    nickname: quote.provider.clone(),
                    user_type: Some("service".into()),
                    is_merchant: true,
                    is_verified: true,
                    completed_orders_30d: None,
                    completion_rate_30d: None,
                    positive_rate: None,
                },
                advertiser_profile_url: None,
                source_url: quote.source_url,
                source_url_is_exact: false,
            };
            routes.push(P2pRoute {
                route_id: String::new(),
                rank: 0,
                asset: quote.target_currency.clone(),
                entry_network: None,
                source_network: None,
                target_network: None,
                source_fiat: quote.source_currency,
                source_amount: fixed(quote.source_amount, 2),
                acquired_asset_amount: fixed(quote.target_amount, 2),
                target_fiat: quote.target_currency,
                target_amount: fixed(quote.target_amount, 2),
                effective_rate: fixed(quote.target_amount / quote.source_amount, 12),
                same_venue: true,
                requires_asset_transfer: false,
                transfer_fee_included: true,
                route_kind: "fiat_to_fiat".into(),
                bridge_currency: None,
                market_path: None,
                route_provider: None,
                route_provider_url: None,
                provider_quote_id: None,
                route_path: Vec::new(),
                route_fees: Vec::new(),
                quote_expires_at: None,
                payment_methods_verified: false,
                entry_offer: Some(offer),
                exit_offer: None,
                warnings: vec![
                    "Indicative direct-transfer quote; confirm the live rate, account eligibility, transfer limits, and recipient details with the provider before sending."
                        .into(),
                ],
                services: Vec::new(),
                reputation: None,
                feedback: None,
                service_links: Vec::new(),
            });
        }
        routes
    }
}

async fn publish_update(
    updates: Option<&mpsc::Sender<P2pRouteSearchResponse>>,
    response: P2pRouteSearchResponse,
) {
    if let Some(updates) = updates {
        let _ = updates.send(response).await;
    }
}

async fn stream_fiat_asset_search(
    service: &P2pSearchService,
    asset: String,
    entry_query: P2pSearchQuery,
    exit_query: P2pSearchQuery,
    market: Option<P2pOfferMarket>,
    progress: mpsc::Sender<FiatAssetProgress>,
) -> Result<(String, P2pSearchResponse, P2pSearchResponse)> {
    let (entry_updates, mut entry_snapshots) = mpsc::channel(16);
    let (exit_updates, mut exit_snapshots) = mpsc::channel(16);
    let entry_search = service.stream_search_market(entry_query, entry_updates, market);
    let exit_search = service.stream_search_market(exit_query, exit_updates, market);
    tokio::pin!(entry_search);
    tokio::pin!(exit_search);

    let mut entry = None;
    let mut exit = None;
    let mut entry_done = false;
    let mut exit_done = false;
    let mut entry_channel_open = true;
    let mut exit_channel_open = true;

    while !entry_done || !exit_done {
        let changed = tokio::select! {
            snapshot = entry_snapshots.recv(), if entry_channel_open => {
                match snapshot {
                    Some(snapshot) => {
                        entry = Some(snapshot);
                        true
                    }
                    None => {
                        entry_channel_open = false;
                        false
                    }
                }
            }
            snapshot = exit_snapshots.recv(), if exit_channel_open => {
                match snapshot {
                    Some(snapshot) => {
                        exit = Some(snapshot);
                        true
                    }
                    None => {
                        exit_channel_open = false;
                        false
                    }
                }
            }
            result = &mut entry_search, if !entry_done => {
                entry = Some(result?);
                entry_done = true;
                entry_channel_open = false;
                true
            }
            result = &mut exit_search, if !exit_done => {
                exit = Some(result?);
                exit_done = true;
                exit_channel_open = false;
                true
            }
        };

        if changed {
            if let (Some(entry), Some(exit)) = (&entry, &exit) {
                if progress
                    .send(FiatAssetProgress {
                        asset: asset.clone(),
                        entry: entry.clone(),
                        exit: exit.clone(),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    let entry = entry.ok_or_else(|| anyhow::anyhow!("entry search returned no response"))?;
    let exit = exit.ok_or_else(|| anyhow::anyhow!("exit search returned no response"))?;
    Ok((asset, entry, exit))
}

fn apply_fiat_asset_response(
    routes: &mut HashMap<String, P2pRoute>,
    asset_statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    entry: &P2pSearchResponse,
    exit: &P2pSearchResponse,
) {
    let entry_offers = reject_price_outliers(
        matching_offers(&entry.offers, query),
        query.max_price_deviation_bps,
    );
    let exit_offers = reject_price_outliers(
        matching_offers(&exit.offers, query),
        query.max_price_deviation_bps,
    );
    let mut discovered = Vec::new();
    let routes_exhaustive =
        compose_fiat_routes(&mut discovered, query, asset, &entry_offers, &exit_offers);
    let routes_built = discovered.len();
    routes.retain(|_, route| route.route_kind != "fiat_to_fiat" || route.asset != asset);
    merge_routes(routes, discovered);
    upsert_asset_status(
        asset_statuses,
        asset.to_string(),
        &entry.sources,
        &exit.sources,
        &entry_offers,
        &exit_offers,
        routes_built,
        routes_exhaustive,
        Some(entry.source.as_str()),
        Some(exit.source.as_str()),
    );
}

fn apply_fiat_to_crypto_response(
    routes: &mut HashMap<String, P2pRoute>,
    asset_statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = reject_price_outliers(
        matching_offers(&response.offers, query),
        query.max_price_deviation_bps,
    );
    let mut discovered = Vec::new();
    compose_fiat_to_crypto_routes(&mut discovered, query, asset, &offers);
    let routes_built = discovered.len();
    routes.clear();
    merge_routes(routes, discovered);
    upsert_asset_status(
        asset_statuses,
        asset.to_string(),
        &response.sources,
        &[],
        &offers,
        &[],
        routes_built,
        true,
        Some(response.source.as_str()),
        None,
    );
}

fn apply_crypto_to_fiat_response(
    routes: &mut HashMap<String, P2pRoute>,
    asset_statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = reject_price_outliers(
        matching_offers(&response.offers, query),
        query.max_price_deviation_bps,
    );
    let mut discovered = Vec::new();
    compose_crypto_to_fiat_routes(&mut discovered, query, asset, &offers);
    let routes_built = discovered.len();
    routes.clear();
    merge_routes(routes, discovered);
    upsert_asset_status(
        asset_statuses,
        asset.to_string(),
        &[],
        &response.sources,
        &[],
        &offers,
        routes_built,
        true,
        None,
        Some(response.source.as_str()),
    );
}

fn apply_crypto_market_result(
    routes: &mut HashMap<String, P2pRoute>,
    query: &NormalizedRouteQuery,
    source_asset: &str,
    target_asset: &str,
    result: (String, Result<Vec<CryptoTicker>>),
) -> bool {
    let (venue, result) = result;
    let Ok(tickers) = result else {
        return false;
    };
    let mut discovered = Vec::new();
    compose_crypto_market_routes(
        &mut discovered,
        query,
        &venue,
        source_asset,
        target_asset,
        &tickers,
    );
    merge_routes(routes, discovered);
    true
}

fn response_snapshot(
    search_id: Uuid,
    query: &NormalizedRouteQuery,
    routes: &HashMap<String, P2pRoute>,
    asset_statuses: &[RouteAssetStatus],
) -> P2pRouteSearchResponse {
    let routes_found = routes.len();
    let mut visible_routes = routes.values().cloned().collect::<Vec<_>>();
    sort_routes(&mut visible_routes);
    truncate_routes_preserving_providers(&mut visible_routes, query.limit);
    for (index, route) in visible_routes.iter_mut().enumerate() {
        route.rank = index + 1;
    }
    let (source, stale) = route_discovery_source(asset_statuses);
    P2pRouteSearchResponse {
        search_id,
        routes_found,
        routes_exhaustive: asset_statuses.iter().all(|status| status.routes_exhaustive),
        searched_at: Utc::now(),
        source_fiat: query.source_currency.clone(),
        target_fiat: query.target_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        assets_searched: query.assets.clone(),
        can_exchange_to_target: routes_found > 0,
        routes: visible_routes,
        asset_statuses: asset_statuses.to_vec(),
        source: source.into(),
        stale,
    }
}

fn route_discovery_source(asset_statuses: &[RouteAssetStatus]) -> (&'static str, bool) {
    let sources = asset_statuses.iter().flat_map(|status| {
        status
            .entry_discovery_source
            .iter()
            .chain(status.exit_discovery_source.iter())
            .map(String::as_str)
    });
    let mut has_fmatch = false;
    let mut has_cache = false;
    let mut has_fallback = false;
    for source in sources {
        has_fmatch |= source == "fmatch";
        has_cache |= source == "database_cache";
        has_fallback |= source == "provider_fallback";
    }
    if has_fallback {
        ("provider_fallback", false)
    } else if has_cache {
        ("database_cache", true)
    } else if has_fmatch {
        ("fmatch", false)
    } else {
        ("provider", false)
    }
}

fn truncate_routes_preserving_providers(routes: &mut Vec<P2pRoute>, limit: usize) {
    if routes.len() <= limit {
        return;
    }

    let reserved_indices = {
        let mut represented_providers = HashSet::new();
        routes
            .iter()
            .enumerate()
            .filter_map(|(index, route)| {
                route_provider_names(route)
                    .iter()
                    .any(|provider| represented_providers.insert(*provider))
                    .then_some(index)
            })
            .take(limit)
            .collect::<HashSet<_>>()
    };
    let mut remaining = limit.saturating_sub(reserved_indices.len());
    let mut index = 0;
    routes.retain(|_| {
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

fn route_provider_names(route: &P2pRoute) -> Vec<&str> {
    let mut providers = Vec::new();
    if let Some(provider) = route.route_provider.as_deref() {
        providers.push(provider);
    }
    if let Some(provider) = route.market_path.as_ref().map(|path| path.venue.as_str()) {
        providers.push(provider);
    }
    if let Some(provider) = route
        .entry_offer
        .as_ref()
        .map(|offer| offer.source.as_str())
    {
        providers.push(provider);
    }
    if let Some(provider) = route.exit_offer.as_ref().map(|offer| offer.source.as_str()) {
        providers.push(provider);
    }
    providers
}

fn sort_routes(routes: &mut [P2pRoute]) {
    routes.sort_by(|left, right| {
        route_target(right)
            .partial_cmp(&route_target(left))
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                right
                    .payment_methods_verified
                    .cmp(&left.payment_methods_verified)
            })
            .then_with(|| right.same_venue.cmp(&left.same_venue))
            .then_with(|| left.route_id.cmp(&right.route_id))
    });
}

fn merge_routes(routes: &mut HashMap<String, P2pRoute>, discovered: Vec<P2pRoute>) -> usize {
    let before = routes.len();
    for mut route in discovered {
        let route_id = route_fingerprint(&route);
        route.route_id.clone_from(&route_id);
        match routes.get(&route_id) {
            Some(current) if route_target(current) >= route_target(&route) => {}
            _ => {
                routes.insert(route_id, route);
            }
        }
    }
    routes.len() - before
}

fn route_fingerprint(route: &P2pRoute) -> String {
    let entry = route
        .entry_offer
        .as_ref()
        .map(|offer| {
            format!(
                "{}:{}:{}",
                offer.source,
                offer.ad_id,
                offer.payment_methods.join(",")
            )
        })
        .unwrap_or_default();
    let exit = route
        .exit_offer
        .as_ref()
        .map(|offer| {
            format!(
                "{}:{}:{}",
                offer.source,
                offer.ad_id,
                offer.payment_methods.join(",")
            )
        })
        .unwrap_or_default();
    let market = route
        .market_path
        .as_ref()
        .map(|path| format!("{}:{}:{}", path.venue, path.source_pair, path.target_pair))
        .unwrap_or_default();
    let mut identity = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        route.route_kind,
        route.asset,
        route.source_fiat,
        route.target_fiat,
        route.source_network.as_deref().unwrap_or_default(),
        route.target_network.as_deref().unwrap_or_default(),
        route.bridge_currency.as_deref().unwrap_or_default(),
        route.route_provider.as_deref().unwrap_or_default(),
        route.route_path.join(","),
        entry,
        exit,
        market,
        route.source_amount,
    );
    if let Some(quote_id) = route.provider_quote_id.as_deref() {
        identity.push('|');
        identity.push_str(quote_id);
    }
    format!("{:x}", Sha256::digest(identity.as_bytes()))
}

fn normalize_query(
    query: P2pRouteSearchQuery,
    default_assets: &[String],
    networks: &crate::networks::NetworkCatalog,
    provider_assets: &[Asset],
) -> Result<NormalizedRouteQuery> {
    if !query.source_amount.is_finite() || query.source_amount <= 0.0 {
        bail!("source_amount must be a positive finite number");
    }
    let source_currency = query.source_fiat.trim().to_ascii_uppercase();
    let target_currency = query.target_fiat.trim().to_ascii_uppercase();
    if query
        .min_completion_rate
        .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
    {
        bail!("min_completion_rate must be between 0 and 1");
    }
    let assets_explicit = query.intermediary_assets.is_some() || query.assets.is_some();
    let assets = query
        .intermediary_assets
        .as_deref()
        .or(query.assets.as_deref())
        .map(|assets| assets.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_else(|| default_assets.to_vec())
        .into_iter()
        .map(|asset| asset.trim().to_ascii_uppercase())
        .filter(|asset| !asset.is_empty())
        .collect::<Vec<_>>();
    if assets.is_empty() || assets.len() > 24 {
        bail!("assets must contain between 1 and 24 comma-separated codes");
    }
    if assets.iter().any(|asset| {
        !(2..=12).contains(&asset.len()) || !asset.bytes().all(|b| b.is_ascii_alphanumeric())
    }) {
        bail!("every asset must be a 2-12 character alphanumeric code");
    }
    let max_price_deviation_bps = query
        .max_price_deviation_bps
        .unwrap_or(DEFAULT_MAX_PRICE_DEVIATION_BPS);
    if max_price_deviation_bps > 5_000 {
        bail!("max_price_deviation_bps must not exceed 5000");
    }
    let source_network = validate_network(
        networks,
        &query.source_network,
        &source_currency,
        provider_assets,
    )?;
    let target_network = validate_network(
        networks,
        &query.target_network,
        &target_currency,
        provider_assets,
    )?;
    if source_currency.eq_ignore_ascii_case(&target_currency) {
        match (&source_network, &target_network) {
            (Some(source_network), Some(target_network)) if source_network == target_network => {
                bail!("source and target asset/network must differ")
            }
            _ => {}
        }
    }

    Ok(NormalizedRouteQuery {
        source_currency,
        target_currency,
        source_amount: query.source_amount,
        source_network,
        target_network,
        assets,
        assets_explicit,
        source_payment_method: trimmed(query.source_payment_method),
        target_payment_method: trimmed(query.target_payment_method),
        merchant_only: query.merchant_only.unwrap_or(false),
        min_orders: query.min_orders,
        min_completion_rate: query.min_completion_rate,
        allow_cross_venue: query.allow_cross_venue.unwrap_or(false),
        max_price_deviation_bps,
        limit: query
            .limit
            .unwrap_or(DEFAULT_ROUTE_LIMIT)
            .clamp(1, MAX_ROUTE_LIMIT),
        sources: normalize_sources(query.sources)?,
        exchange_mode: query.exchange_mode,
    })
}

fn source_selected(query: &NormalizedRouteQuery, provider: &str) -> bool {
    query
        .sources
        .as_deref()
        .is_none_or(|sources| sources.split(',').any(|source| source == provider))
}

fn intermediary_asset_priority(symbol: &str, target_symbol: &str) -> u8 {
    if symbol.eq_ignore_ascii_case(target_symbol) {
        0
    } else {
        match symbol.to_ascii_uppercase().as_str() {
            "USDT" => 1,
            "USDC" => 2,
            "BTC" => 3,
            "ETH" => 4,
            _ => 5,
        }
    }
}

fn validate_network(
    networks: &crate::networks::NetworkCatalog,
    network_id: &Option<String>,
    currency: &str,
    provider_assets: &[Asset],
) -> Result<Option<String>> {
    let Some(network_id) = network_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let canonical_id = canonical_network_id(network_id);
    let network = networks.compatible_network(&canonical_id, currency);
    match network {
        Some(network) => Ok(Some(network.id.clone())),
        None if provider_assets.iter().any(|asset| {
            asset.symbol.eq_ignore_ascii_case(currency)
                && asset.location.as_deref() == Some(canonical_id.as_str())
        }) =>
        {
            Ok(Some(canonical_id))
        }
        None => bail!("network {network_id} is not compatible with {currency}"),
    }
}

fn is_crypto_currency(
    currency: &str,
    networks: &crate::networks::NetworkCatalog,
    provider_assets: &[Asset],
) -> bool {
    networks.is_supported_asset(currency)
        || provider_assets
            .iter()
            .any(|asset| asset.symbol.eq_ignore_ascii_case(currency))
}

fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn leg_query(
    fiat: &str,
    asset: &str,
    side: P2pSide,
    amount: Option<f64>,
    payment_method: Option<String>,
    route: &NormalizedRouteQuery,
) -> P2pSearchQuery {
    P2pSearchQuery {
        fiat: fiat.into(),
        asset: asset.into(),
        side,
        amount,
        payment_method,
        merchant_only: Some(route.merchant_only),
        min_orders: route.min_orders,
        min_completion_rate: route.min_completion_rate,
        limit: Some(LEG_SEARCH_LIMIT),
        sources: route.sources.clone(),
    }
}

fn matching_offers(offers: &[P2pOffer], query: &NormalizedRouteQuery) -> Vec<P2pOffer> {
    offers
        .iter()
        .filter(|offer| query.accepts_offer(offer))
        .cloned()
        .collect()
}

fn reject_price_outliers(offers: Vec<P2pOffer>, max_deviation_bps: u32) -> Vec<P2pOffer> {
    if offers.len() < 3 || max_deviation_bps == 0 {
        return offers;
    }
    let mut prices = offers
        .iter()
        .filter(|offer| offer.market == P2pOfferMarket::P2p)
        .filter_map(|offer| offer.price.parse::<f64>().ok())
        .filter(|price| price.is_finite() && *price > 0.0)
        .collect::<Vec<_>>();
    if prices.len() < 3 {
        return offers;
    }
    prices.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let median = prices[prices.len() / 2];
    let maximum_deviation = median * f64::from(max_deviation_bps) / 10_000.0;
    offers
        .into_iter()
        .filter(|offer| {
            offer.market == P2pOfferMarket::DirectExchange
                || offer
                    .price
                    .parse::<f64>()
                    .is_ok_and(|price| (price - median).abs() <= maximum_deviation)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct FiatRouteCandidate {
    target_amount: f64,
    entry_index: usize,
    exit_index: usize,
}

impl PartialEq for FiatRouteCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.target_amount.total_cmp(&other.target_amount) == Ordering::Equal
            && self.entry_index == other.entry_index
            && self.exit_index == other.exit_index
    }
}

impl Eq for FiatRouteCandidate {}

impl PartialOrd for FiatRouteCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FiatRouteCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.target_amount
            .total_cmp(&other.target_amount)
            .then_with(|| other.entry_index.cmp(&self.entry_index))
            .then_with(|| other.exit_index.cmp(&self.exit_index))
    }
}

fn compose_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
) -> bool {
    let mut entries = entry_offers
        .iter()
        .filter_map(|entry| {
            let entry_price = positive_number(&entry.price)?;
            let acquired_asset = query.source_amount / entry_price;
            positive_number(&entry.available_asset)
                .is_none_or(|available| available >= acquired_asset)
                .then_some((entry, entry_price, acquired_asset))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut exits = exit_offers
        .iter()
        .filter_map(|exit| positive_number(&exit.price).map(|price| (exit, price)))
        .collect::<Vec<_>>();
    exits.sort_by(|left, right| right.1.total_cmp(&left.1));
    if entries.is_empty() || exits.is_empty() {
        return true;
    }

    let candidate_limit = query.limit.saturating_mul(3).max(query.limit);
    routes.reserve(candidate_limit.min(entries.len().saturating_mul(exits.len())));
    let routes_start = routes.len();
    let mut evaluated = HashSet::new();
    let mut source_names = Vec::new();
    let mut seen_sources = HashSet::new();
    for index in 0..entries.len().max(exits.len()) {
        if let Some((entry, _, _)) = entries.get(index) {
            if seen_sources.insert(entry.source.as_str()) {
                source_names.push(entry.source.as_str());
            }
        }
        if let Some((exit, _)) = exits.get(index) {
            if seen_sources.insert(exit.source.as_str()) {
                source_names.push(exit.source.as_str());
            }
        }
    }
    for source in source_names {
        let mut best = None;
        for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
            if entry.source != source {
                continue;
            }
            for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
            if exit.source != source {
                continue;
            }
            for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        if let Some((_, entry_index, exit_index, route)) = best {
            if evaluated.insert((entry_index, exit_index)) {
                routes.push(route);
            }
        }
        if routes.len() - routes_start >= candidate_limit {
            routes[routes_start..].sort_by(|left, right| {
                route_target(right)
                    .partial_cmp(&route_target(left))
                    .unwrap_or(Ordering::Equal)
            });
            return false;
        }
    }

    let mut frontier = BinaryHeap::with_capacity(entries.len());
    for (entry_index, (_, _, acquired_asset)) in entries.iter().enumerate() {
        frontier.push(FiatRouteCandidate {
            target_amount: acquired_asset * exits[0].1,
            entry_index,
            exit_index: 0,
        });
    }

    while let Some(candidate) = frontier.pop() {
        let (entry, _, acquired_asset) = entries[candidate.entry_index];
        let (exit, exit_price) = exits[candidate.exit_index];
        let next_exit_index = candidate.exit_index + 1;
        if next_exit_index < exits.len() {
            frontier.push(FiatRouteCandidate {
                target_amount: acquired_asset * exits[next_exit_index].1,
                entry_index: candidate.entry_index,
                exit_index: next_exit_index,
            });
        }

        if evaluated.insert((candidate.entry_index, candidate.exit_index)) {
            if let Some(route) = compose_fiat_route(
                query,
                asset,
                entry,
                exit,
                acquired_asset,
                acquired_asset * exit_price,
            ) {
                routes.push(route);
                if routes.len() - routes_start >= candidate_limit {
                    routes[routes_start..].sort_by(|left, right| {
                        route_target(right)
                            .partial_cmp(&route_target(left))
                            .unwrap_or(Ordering::Equal)
                    });
                    return frontier.is_empty();
                }
            }
        }
    }
    routes[routes_start..].sort_by(|left, right| {
        route_target(right)
            .partial_cmp(&route_target(left))
            .unwrap_or(Ordering::Equal)
    });
    true
}

fn compose_fiat_route(
    query: &NormalizedRouteQuery,
    asset: &str,
    entry: &P2pOffer,
    exit: &P2pOffer,
    acquired_asset: f64,
    target_amount: f64,
) -> Option<P2pRoute> {
    if !offer_networks_compatible(entry, exit) {
        return None;
    }
    let same_venue = entry.source == exit.source;
    if !same_venue && !query.allow_cross_venue {
        return None;
    }
    if positive_number(&exit.available_asset).is_none_or(|available| available < acquired_asset)
        || !covers_target(exit, target_amount)
    {
        return None;
    }
    let mut warnings = vec![
        "Search estimate only: platform fees, account eligibility and execution are not verified."
            .into(),
    ];
    if !same_venue {
        warnings.push(
            "Cross-venue route requires an asset transfer; network fee and compatible network are not included."
                .into(),
        );
    }
    let entry_method_match = query
        .source_payment_method
        .as_deref()
        .map(|method| entry.payment_method_match(method));
    let exit_method_match = query
        .target_payment_method
        .as_deref()
        .map(|method| exit.payment_method_match(method));
    if entry_method_match == Some(PaymentMethodMatch::Unknown) {
        warnings.push(
            "The selected sender bank could not be verified because the venue returned an opaque payment-method ID."
                .into(),
        );
    }
    if exit_method_match == Some(PaymentMethodMatch::Unknown) {
        warnings.push(
            "The selected recipient bank could not be verified because the venue returned an opaque payment-method ID."
                .into(),
        );
    }
    if !entry.source_url_is_exact || !exit.source_url_is_exact {
        warnings.push(
            "At least one selected venue does not expose a public deep-link for this advertisement. Verify the advertiser ID and terms on the venue before sending money."
                .into(),
        );
    }
    let payment_methods_verified = entry_method_match
        .is_none_or(|matched| matched == PaymentMethodMatch::Exact)
        && exit_method_match.is_none_or(|matched| matched == PaymentMethodMatch::Exact);
    Some(P2pRoute {
        route_id: String::new(),
        rank: 0,
        asset: asset.into(),
        entry_network: entry.network.clone().or_else(|| exit.network.clone()),
        source_network: query.source_network.clone(),
        target_network: query.target_network.clone(),
        source_fiat: query.source_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        acquired_asset_amount: fixed(acquired_asset, 8),
        target_fiat: query.target_currency.clone(),
        target_amount: fixed(target_amount, 2),
        effective_rate: fixed(target_amount / query.source_amount, 8),
        same_venue,
        requires_asset_transfer: !same_venue,
        transfer_fee_included: same_venue,
        route_kind: "fiat_to_fiat".into(),
        bridge_currency: None,
        market_path: None,
        route_provider: None,
        route_provider_url: None,
        provider_quote_id: None,
        route_path: Vec::new(),
        route_fees: Vec::new(),
        quote_expires_at: None,
        payment_methods_verified,
        entry_offer: Some(entry.clone()),
        exit_offer: Some(exit.clone()),
        warnings,
        services: Vec::new(),
        reputation: None,
        feedback: None,
        service_links: Vec::new(),
    })
}

fn compose_fiat_to_crypto_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    offers: &[P2pOffer],
) {
    for offer in offers {
        if !offer_matches_network(offer, query.target_network.as_deref()) {
            continue;
        }
        let Some(price) = positive_number(&offer.price) else {
            continue;
        };
        let target_amount = query.source_amount / price;
        if positive_number(&offer.available_asset).is_none_or(|available| available < target_amount)
        {
            continue;
        }
        let payment_methods_verified = query
            .source_payment_method
            .as_deref()
            .is_none_or(|method| offer.payment_method_match(method) == PaymentMethodMatch::Exact);
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: asset.into(),
            entry_network: query
                .target_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_network: query.source_network.clone(),
            target_network: query
                .target_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_fiat: query.source_currency.clone(),
            source_amount: fixed(query.source_amount, 8),
            acquired_asset_amount: fixed(target_amount, 8),
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 8),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_crypto".into(),
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified,
            entry_offer: Some(offer.clone()),
            exit_offer: None,
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

fn compose_crypto_to_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    offers: &[P2pOffer],
) {
    for offer in offers {
        if !offer_matches_network(offer, query.source_network.as_deref()) {
            continue;
        }
        let Some(price) = positive_number(&offer.price) else {
            continue;
        };
        if positive_number(&offer.available_asset)
            .is_none_or(|available| available < query.source_amount)
        {
            continue;
        }
        let target_amount = query.source_amount * price;
        if !covers_target(offer, target_amount) {
            continue;
        }
        let payment_methods_verified = query
            .target_payment_method
            .as_deref()
            .is_none_or(|method| offer.payment_method_match(method) == PaymentMethodMatch::Exact);
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: asset.into(),
            entry_network: query
                .source_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_network: query
                .source_network
                .clone()
                .or_else(|| offer.network.clone()),
            target_network: query.target_network.clone(),
            source_fiat: query.source_currency.clone(),
            source_amount: fixed(query.source_amount, 8),
            acquired_asset_amount: fixed(query.source_amount, 8),
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 2),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat".into(),
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified,
            entry_offer: None,
            exit_offer: Some(offer.clone()),
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

fn offer_matches_network(offer: &P2pOffer, requested: Option<&str>) -> bool {
    offer
        .network
        .as_deref()
        .zip(requested)
        .is_none_or(|(actual, requested)| {
            canonical_network_id(actual) == canonical_network_id(requested)
        })
}

fn offer_networks_compatible(entry: &P2pOffer, exit: &P2pOffer) -> bool {
    entry
        .network
        .as_deref()
        .zip(exit.network.as_deref())
        .is_none_or(|(entry, exit)| canonical_network_id(entry) == canonical_network_id(exit))
}

fn compose_crypto_market_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    venue: &str,
    source_asset: &str,
    target_asset: &str,
    tickers: &[CryptoTicker],
) {
    const SPOT_FEE_RATE: f64 = 0.001;
    let intermediaries = std::iter::once(None).chain(
        query
            .assets
            .iter()
            .map(String::as_str)
            .filter(|asset| !asset.eq_ignore_ascii_case(source_asset))
            .filter(|asset| !asset.eq_ignore_ascii_case(target_asset))
            .map(Some),
    );

    for intermediary in intermediaries {
        let first = match intermediary {
            Some(asset) => conversion_quote(tickers, source_asset, asset),
            None => conversion_quote(tickers, source_asset, target_asset),
        };
        let second = intermediary.and_then(|asset| conversion_quote(tickers, asset, target_asset));
        let Some(first) = first else {
            continue;
        };
        if intermediary.is_some() && second.is_none() {
            continue;
        }

        let intermediary_amount = query.source_amount * first.rate * (1.0 - SPOT_FEE_RATE);
        let second_rate = second.as_ref().map(|quote| quote.rate).unwrap_or(1.0);
        let target_pair = second
            .as_ref()
            .map(|quote| quote.pair.clone())
            .unwrap_or_else(|| first.pair.clone());
        let target_amount = match second {
            Some(second) => intermediary_amount * second.rate * (1.0 - SPOT_FEE_RATE),
            None => intermediary_amount,
        };
        if !target_amount.is_finite() || target_amount <= 0.0 {
            continue;
        }

        let bridge_currency = intermediary.map(str::to_owned);
        let path = CryptoMarketPath {
            venue: venue.into(),
            source_pair: first.pair,
            target_pair,
            source_rate: fixed(first.rate, 12),
            target_rate: fixed(second_rate, 12),
            intermediary_amount: fixed(intermediary_amount, 12),
        };
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: target_asset.into(),
            entry_network: query.source_network.clone(),
            source_network: query.source_network.clone(),
            target_network: query.target_network.clone(),
            source_fiat: source_asset.into(),
            source_amount: fixed(query.source_amount, 12),
            acquired_asset_amount: fixed(target_amount, 12),
            target_fiat: target_asset.into(),
            target_amount: fixed(target_amount, 12),
            effective_rate: fixed(target_amount / query.source_amount, 12),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto".into(),
            bridge_currency,
            market_path: Some(path),
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified: true,
            entry_offer: None,
            exit_offer: None,
            warnings: vec![
                "Spot-market estimate only: trading fees, slippage and execution are not guaranteed.".into(),
                "Deposit and withdrawal network availability and fees are not verified by the selected venue.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

struct ConversionQuote {
    pair: String,
    rate: f64,
}

fn conversion_quote(tickers: &[CryptoTicker], from: &str, to: &str) -> Option<ConversionQuote> {
    let direct = format!("{}{}", from.to_ascii_uppercase(), to.to_ascii_uppercase());
    if let Some(ticker) = tickers
        .iter()
        .find(|ticker| ticker.symbol.eq_ignore_ascii_case(&direct))
    {
        return Some(ConversionQuote {
            pair: ticker.symbol.clone(),
            rate: ticker.bid,
        });
    }

    let inverse = format!("{}{}", to.to_ascii_uppercase(), from.to_ascii_uppercase());
    tickers
        .iter()
        .find(|ticker| ticker.symbol.eq_ignore_ascii_case(&inverse))
        .map(|ticker| ConversionQuote {
            pair: ticker.symbol.clone(),
            rate: 1.0 / ticker.ask,
        })
}

fn upsert_asset_status(
    statuses: &mut Vec<RouteAssetStatus>,
    asset: String,
    entry_sources: &[SourceStatus],
    exit_sources: &[SourceStatus],
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
    routes_built: usize,
    routes_exhaustive: bool,
    entry_discovery_source: Option<&str>,
    exit_discovery_source: Option<&str>,
) {
    let status = RouteAssetStatus {
        asset,
        entry_offers: entry_offers.len(),
        exit_offers: exit_offers.len(),
        routes_built,
        routes_exhaustive,
        can_exchange_to_target: routes_built > 0,
        entry_sources: entry_sources.to_vec(),
        exit_sources: exit_sources.to_vec(),
        entry_discovery_source: entry_discovery_source.map(str::to_owned),
        exit_discovery_source: exit_discovery_source.map(str::to_owned),
    };
    if let Some(current) = statuses
        .iter_mut()
        .find(|current| current.asset == status.asset)
    {
        *current = status;
    } else {
        statuses.push(status);
    }
}

fn positive_number(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn covers_target(offer: &P2pOffer, target_amount: f64) -> bool {
    let Some(minimum) = positive_number(&offer.min_fiat) else {
        return false;
    };
    let Some(maximum) = positive_number(&offer.max_fiat) else {
        return false;
    };
    minimum <= target_amount && target_amount <= maximum
}

fn route_target(route: &P2pRoute) -> f64 {
    route.target_amount.parse().unwrap_or_default()
}

fn fixed(value: f64, scale: usize) -> String {
    format!("{value:.scale$}")
}

#[cfg(test)]
mod tests;

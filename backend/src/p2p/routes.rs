use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::p2p::service::{
    normalize_sources, P2pOffer, P2pOfferMarket, P2pSearchQuery, P2pSearchResponse,
    P2pSearchService, P2pSide, PaymentMethodMatch, SourceStatus,
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
    pub can_exchange_to_target: bool,
    pub entry_sources: Vec<SourceStatus>,
    pub exit_sources: Vec<SourceStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct P2pRouteSearchResponse {
    pub search_id: Uuid,
    pub routes_found: usize,
    pub searched_at: DateTime<Utc>,
    pub source_fiat: String,
    pub target_fiat: String,
    pub source_amount: String,
    pub assets_searched: Vec<String>,
    pub can_exchange_to_target: bool,
    pub routes: Vec<P2pRoute>,
    pub asset_statuses: Vec<RouteAssetStatus>,
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
}

async fn quote_all_provider_refs(
    providers: Arc<[Arc<dyn PublicRouteProvider>]>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Vec<(String, PublicRouteQuote)> {
    let mut searches = FuturesUnordered::new();
    for provider in providers.iter().cloned() {
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
        let provider_assets = self.provider_assets().await;
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
        if source_is_crypto && target_is_crypto {
            merge_routes(&mut routes, self.search_provider_routes(&query).await);
            if query
                .source_network
                .as_ref()
                .zip(query.target_network.as_ref())
                .is_some_and(|(source, target)| source != target)
            {
                return Ok(response_snapshot(
                    search_id,
                    &query,
                    &routes,
                    &asset_statuses,
                ));
            }
        }
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
                            let (entry, exit) =
                                tokio::join!(self.search(entry_query), self.search(exit_query));
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
                    let search = self.stream_search(leg, leg_updates);
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
                    let response = self.search(leg).await?;
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
                    let search = self.stream_search(leg, leg_updates);
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
                    let response = self.search(leg).await?;
                    apply_crypto_to_fiat_response(
                        &mut routes,
                        &mut asset_statuses,
                        &query,
                        &asset,
                        &response,
                    );
                }
            }
            (true, true) => {
                let source_asset = query.source_currency.clone();
                let target_asset = query.target_currency.clone();
                if updates.is_some() {
                    let (market_updates, mut market_snapshots) = mpsc::channel(8);
                    let search =
                        self.stream_market_tickers(query.sources.as_deref(), market_updates);
                    tokio::pin!(search);
                    loop {
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
                            () = &mut search => {
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
                                break;
                            }
                        }
                    }
                } else {
                    for result in self.search_market_tickers(query.sources.as_deref()).await {
                        apply_crypto_market_result(
                            &mut routes,
                            &query,
                            &source_asset,
                            &target_asset,
                            result,
                        );
                    }
                }
            }
        }
        let mut provider_routes = match (source_is_crypto, target_is_crypto) {
            (false, true) => self.search_fiat_to_crypto_provider_routes(&query).await,
            (true, false) => self.search_crypto_to_fiat_provider_routes(&query).await,
            (false, false) => self.search_fiat_provider_routes(&query).await,
            (true, true) => Vec::new(),
        };
        if !source_is_crypto && !target_is_crypto {
            provider_routes.extend(self.search_direct_fiat_routes(&query).await);
        }
        if merge_routes(&mut routes, provider_routes) > 0 {
            publish_update(
                updates.as_ref(),
                response_snapshot(search_id, &query, &routes, &asset_statuses),
            )
            .await;
        }
        Ok(response_snapshot(
            search_id,
            &query,
            &routes,
            &asset_statuses,
        ))
    }

    async fn search_provider_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let providers = self.route_providers_for_query(query);
        let provider_assets = Self::provider_assets_for(&providers).await;
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
            .take(MAX_PROVIDER_NETWORK_PAIRS)
            .collect::<Vec<_>>();
        let mut searches = FuturesUnordered::new();
        for (source, target) in network_pairs {
            for provider in providers.iter().cloned() {
                let from = source.clone();
                let to = target.clone();
                let Ok(amount) = Amount::from_f64(query.source_amount, from.clone()) else {
                    continue;
                };
                searches.push(quote_provider_many(
                    provider,
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
        Self::provider_assets_for(&self.route_providers).await
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
        let provider_assets = Self::provider_assets_for(&providers).await;
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
        let providers = self.route_providers_for_query(query);
        let provider_assets = Self::provider_assets_for(&providers).await;
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
                    .search(leg_query(
                        &query.source_currency,
                        &symbol,
                        P2pSide::BuyCrypto,
                        Some(query.source_amount),
                        query.source_payment_method.clone(),
                        &query,
                    ))
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
                for offer in entry.offers.iter().take(MAX_PROVIDER_OFFERS_PER_LEG) {
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
                        let providers = providers.clone();
                        let intermediary = intermediary.clone();
                        let target = target.clone();
                        let offer = offer.clone();
                        let query = query.clone();
                        let quote_semaphore = self.quote_semaphore.clone();
                        searches.push(async move {
                            quote_all_provider_refs(
                                providers,
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
        let providers = self.route_providers_for_query(query);
        let provider_assets = Self::provider_assets_for(&providers).await;
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
                .search(leg_query(
                    &query.target_currency,
                    &intermediary.symbol,
                    P2pSide::SellCrypto,
                    None,
                    query.target_payment_method.clone(),
                    query,
                ))
                .await
            else {
                continue;
            };
            let exit_offers = exit
                .offers
                .into_iter()
                .take(MAX_PROVIDER_OFFERS_PER_LEG)
                .filter(|offer| offer_matches_network(offer, intermediary.location.as_deref()))
                .collect::<Vec<_>>();
            for source in &source_assets {
                if *source == intermediary {
                    continue;
                }
                let Ok(amount) = Amount::from_f64(query.source_amount, source.clone()) else {
                    continue;
                };
                let providers = providers.clone();
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
                        providers,
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

    async fn search_fiat_provider_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let mut searches = FuturesUnordered::new();
        let providers = self.route_providers_for_query(query);
        let provider_assets = Self::provider_assets_for(&providers).await;
        for asset in self.intermediary_assets(query).await {
            let source_networks = self.assets_on_network(
                &asset.symbol,
                query.source_network.as_deref(),
                &provider_assets,
            );
            let target_networks = self.assets_on_network(
                &asset.symbol,
                query.target_network.as_deref(),
                &provider_assets,
            );
            if source_networks.is_empty() || target_networks.is_empty() {
                continue;
            }
            let network_pairs = source_networks
                .iter()
                .flat_map(|source| {
                    target_networks
                        .iter()
                        .filter(move |target| source.location != target.location)
                        .map(move |target| (source.clone(), target.clone()))
                })
                .take(MAX_PROVIDER_NETWORK_PAIRS)
                .collect::<Vec<_>>();
            if network_pairs.is_empty() {
                continue;
            }
            let Ok(entry) = self
                .search(leg_query(
                    &query.source_currency,
                    &asset.symbol,
                    P2pSide::BuyCrypto,
                    Some(query.source_amount),
                    query.source_payment_method.clone(),
                    query,
                ))
                .await
            else {
                continue;
            };
            let Ok(exit) = self
                .search(leg_query(
                    &query.target_currency,
                    &asset.symbol,
                    P2pSide::SellCrypto,
                    None,
                    query.target_payment_method.clone(),
                    query,
                ))
                .await
            else {
                continue;
            };
            let exit_offers = exit
                .offers
                .into_iter()
                .take(MAX_PROVIDER_OFFERS_PER_LEG)
                .collect::<Vec<_>>();
            for entry_offer in entry.offers.into_iter().take(MAX_PROVIDER_OFFERS_PER_LEG) {
                let Some(entry_price) = positive_number(&entry_offer.price) else {
                    continue;
                };
                let source_amount = query.source_amount / entry_price;
                if positive_number(&entry_offer.available_asset)
                    .is_none_or(|available| available < source_amount)
                {
                    continue;
                }
                for (source_network, target_network) in &network_pairs {
                    let from = source_network.clone();
                    let to = target_network.clone();
                    let Ok(amount) = Amount::from_f64(source_amount, from.clone()) else {
                        continue;
                    };
                    for provider in providers.iter().cloned() {
                        let entry_offer = entry_offer.clone();
                        let exit_offers = exit_offers.clone();
                        let from = from.clone();
                        let to = to.clone();
                        let amount = amount.clone();
                        let quote_semaphore = self.quote_semaphore.clone();
                        searches.push(async move {
                            let (provider_name, quote) =
                                quote_provider(provider, from, to, amount, quote_semaphore).await?;
                            Some((provider_name, quote, entry_offer, exit_offers))
                        });
                    }
                }
            }
        }

        let mut routes = Vec::new();
        while let Some(result) = searches.next().await {
            let Some((provider_name, quote, entry, exit_offers)) = result else {
                continue;
            };
            let Ok(output_asset) = quote.output.value.parse::<f64>() else {
                continue;
            };
            if !output_asset.is_finite() || output_asset <= 0.0 {
                continue;
            }
            for exit in exit_offers {
                let Some(exit_price) = positive_number(&exit.price) else {
                    continue;
                };
                let target_amount = output_asset * exit_price;
                if !covers_target(&exit, target_amount)
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
                    exit_offer: Some(exit),
                    warnings: vec![
                        "Live fiat entry/exit offers plus a live dry cross-network quote; platform limits and execution are not verified.".into(),
                        "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.".into(),
                    ],
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
            }
        }
        routes
    }

    async fn search_direct_fiat_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let mut searches = FuturesUnordered::new();
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
            searches.push(async move {
                provider
                    .quote(&source_currency, &target_currency, source_amount)
                    .await
                    .ok()
            });
        }

        let mut routes = Vec::new();
        while let Some(Some(quote)) = searches.next().await {
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
    progress: mpsc::Sender<FiatAssetProgress>,
) -> Result<(String, P2pSearchResponse, P2pSearchResponse)> {
    let (entry_updates, mut entry_snapshots) = mpsc::channel(16);
    let (exit_updates, mut exit_snapshots) = mpsc::channel(16);
    let entry_search = service.stream_search(entry_query, entry_updates);
    let exit_search = service.stream_search(exit_query, exit_updates);
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
    let entry_offers = reject_price_outliers(entry.offers.clone(), query.max_price_deviation_bps);
    let exit_offers = reject_price_outliers(exit.offers.clone(), query.max_price_deviation_bps);
    let mut discovered = Vec::new();
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
    );
}

fn apply_fiat_to_crypto_response(
    routes: &mut HashMap<String, P2pRoute>,
    asset_statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = reject_price_outliers(response.offers.clone(), query.max_price_deviation_bps);
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
    );
}

fn apply_crypto_to_fiat_response(
    routes: &mut HashMap<String, P2pRoute>,
    asset_statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = reject_price_outliers(response.offers.clone(), query.max_price_deviation_bps);
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
    visible_routes.truncate(query.limit);
    for (index, route) in visible_routes.iter_mut().enumerate() {
        route.rank = index + 1;
    }
    P2pRouteSearchResponse {
        search_id,
        routes_found,
        searched_at: Utc::now(),
        source_fiat: query.source_currency.clone(),
        target_fiat: query.target_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        assets_searched: query.assets.clone(),
        can_exchange_to_target: routes_found > 0,
        routes: visible_routes,
        asset_statuses: asset_statuses.to_vec(),
    }
}

fn sort_routes(routes: &mut [P2pRoute]) {
    routes.sort_by(|left, right| {
        right
            .payment_methods_verified
            .cmp(&left.payment_methods_verified)
            .then_with(|| {
                route_target(right)
                    .partial_cmp(&route_target(left))
                    .unwrap_or(Ordering::Equal)
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

fn compose_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
) {
    for entry in entry_offers {
        let Some(entry_price) = positive_number(&entry.price) else {
            continue;
        };
        let acquired_asset = query.source_amount / entry_price;
        if positive_number(&entry.available_asset)
            .is_none_or(|available| available < acquired_asset)
        {
            continue;
        }
        for exit in exit_offers {
            if !offer_networks_compatible(entry, exit) {
                continue;
            }
            let same_venue = entry.source == exit.source;
            if !same_venue && !query.allow_cross_venue {
                continue;
            }
            let Some(exit_price) = positive_number(&exit.price) else {
                continue;
            };
            if positive_number(&exit.available_asset)
                .is_none_or(|available| available < acquired_asset)
            {
                continue;
            }
            let target_amount = acquired_asset * exit_price;
            if !covers_target(exit, target_amount) {
                continue;
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
            routes.push(P2pRoute {
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
            });
        }
    }
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
) {
    let status = RouteAssetStatus {
        asset,
        entry_offers: entry_offers.len(),
        exit_offers: exit_offers.len(),
        routes_built,
        can_exchange_to_target: routes_built > 0,
        entry_sources: entry_sources.to_vec(),
        exit_sources: exit_sources.to_vec(),
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
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::config::Config;
    use crate::p2p::service::P2pSource;
    use crate::p2p::{Advertiser, FiatRouteQuote, PublicFiatRouteProvider};
    use crate::route_engine::{PublicRouteProvider, PublicRouteQuote};
    use async_trait::async_trait;

    struct ProgressiveSource;

    struct DelayedRouteSource {
        name: &'static str,
        delay: Duration,
        price: &'static str,
    }

    struct FixedIntentProvider;

    struct FixedFiatRouteProvider;

    #[async_trait]
    impl PublicFiatRouteProvider for FixedFiatRouteProvider {
        fn name(&self) -> &str {
            "id-pay"
        }

        fn supports_pair(&self, source_currency: &str, target_currency: &str) -> bool {
            matches!(
                (source_currency, target_currency),
                ("AMD", "RUB") | ("RUB", "AMD")
            )
        }

        async fn quote(
            &self,
            source_currency: &str,
            target_currency: &str,
            source_amount: f64,
        ) -> Result<FiatRouteQuote> {
            let target_amount = if source_currency == "RUB" {
                source_amount * 4.0
            } else {
                source_amount / 4.0
            };
            Ok(FiatRouteQuote {
                provider: self.name().into(),
                source_url: "https://id-pay.ru/".into(),
                source_currency: source_currency.into(),
                target_currency: target_currency.into(),
                source_amount,
                target_amount,
            })
        }
    }

    struct PricedRouteProvider {
        name: &'static str,
        multiplier: f64,
        fee: &'static str,
    }

    #[async_trait]
    impl PublicRouteProvider for PricedRouteProvider {
        fn name(&self) -> &str {
            self.name
        }

        async fn supported_assets(&self) -> Vec<Asset> {
            ["USDT", "USDC"]
                .into_iter()
                .map(|symbol| Asset::new(symbol, Some("ethereum")).unwrap())
                .collect()
        }

        async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
            let output = amount.value.parse::<f64>().unwrap() * self.multiplier;
            Ok(PublicRouteQuote {
                provider: self.name.into(),
                quote_id: None,
                description: None,
                from: from.clone(),
                to: to.clone(),
                input: amount,
                output: Amount::from_f64(output, to.clone())?,
                fees: vec![Amount::new(self.fee, from.clone())?],
                expires_at: DateTime::from_timestamp(1_900_000_000, 0),
                path: vec![from, to],
            })
        }
    }

    #[async_trait]
    impl PublicRouteProvider for FixedIntentProvider {
        fn name(&self) -> &str {
            "test-intents"
        }

        async fn supported_assets(&self) -> Vec<Asset> {
            [
                ("USDT", "tron"),
                ("USDT", "avalanche-c"),
                ("USDT", "ethereum"),
                ("USDT", "gnosis"),
                ("USDT", "near"),
                ("USDT", "optimism"),
                ("USDT", "scroll"),
                ("USDC", "solana"),
                ("BTC", "bitcoin"),
                ("BTC", "near"),
            ]
            .into_iter()
            .map(|(symbol, network)| Asset::new(symbol, Some(network)).unwrap())
            .collect()
        }

        async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
            let value = amount.value.parse::<f64>().unwrap() * 0.98;
            Ok(PublicRouteQuote {
                provider: self.name().into(),
                quote_id: None,
                description: None,
                from: from.clone(),
                to: to.clone(),
                input: amount,
                output: Amount::from_f64(value, to.clone())?,
                fees: vec![Amount::new("0.5", from.clone())?],
                expires_at: None,
                path: vec![from, to],
            })
        }
    }

    #[async_trait]
    impl P2pSource for ProgressiveSource {
        fn name(&self) -> &str {
            "bybit"
        }

        async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            let delay = if query.asset == "USDT" { 5 } else { 20 };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let mut result = offer(
                "bybit",
                query.side,
                if query.side == P2pSide::BuyCrypto {
                    "400"
                } else {
                    "80"
                },
                "1000",
                "200000",
            );
            result.fiat.clone_from(&query.fiat);
            result.asset.clone_from(&query.asset);
            result.ad_id = format!("{}-{:?}", query.asset, query.side);
            if query.payment_method.is_some() {
                result.payment_methods = vec!["Other Bank".into()];
            }
            Ok(vec![result])
        }
    }

    #[async_trait]
    impl P2pSource for DelayedRouteSource {
        fn name(&self) -> &str {
            self.name
        }

        async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
            tokio::time::sleep(self.delay).await;
            let mut result = offer(self.name, query.side, self.price, "1", "1000000");
            result.fiat.clone_from(&query.fiat);
            result.asset.clone_from(&query.asset);
            result.ad_id = format!("{}-{:?}", self.name, query.side);
            Ok(vec![result])
        }
    }

    fn offer(source: &str, side: P2pSide, price: &str, min: &str, max: &str) -> P2pOffer {
        P2pOffer {
            market: crate::p2p::service::P2pOfferMarket::P2p,
            source: source.into(),
            ad_id: format!("{source}-{price}"),
            side,
            fiat: if side == P2pSide::BuyCrypto {
                "AMD"
            } else {
                "RUB"
            }
            .into(),
            asset: "USDT".into(),
            network: None,
            price: price.into(),
            available_asset: "10000".into(),
            min_fiat: min.into(),
            max_fiat: max.into(),
            payment_methods: Vec::new(),
            pay_time_limit_minutes: Some(15),
            advertiser: Advertiser {
                id: None,
                nickname: source.into(),
                user_type: None,
                is_merchant: true,
                is_verified: true,
                completed_orders_30d: Some(100),
                completion_rate_30d: Some(0.99),
                positive_rate: None,
            },
            advertiser_profile_url: None,
            source_url: "https://example.test".into(),
            source_url_is_exact: true,
        }
    }

    fn query(allow_cross_venue: bool) -> NormalizedRouteQuery {
        NormalizedRouteQuery {
            source_currency: "AMD".into(),
            target_currency: "RUB".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            assets: vec!["USDT".into()],
            assets_explicit: false,
            source_payment_method: None,
            target_payment_method: None,
            merchant_only: false,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue,
            max_price_deviation_bps: 1_000,
            limit: 20,
            sources: None,
        }
    }

    #[test]
    fn recognizes_assets_present_in_the_network_catalog_as_crypto() {
        let networks = crate::networks::NetworkCatalog::test_default();
        for asset in ["BTC", "ETH", "USDT", "USDC", "TRX", "TON"] {
            assert!(
                networks.is_supported_asset(asset),
                "asset should use crypto routing: {asset}"
            );
        }
        assert!(!networks.is_supported_asset("AMD"));
    }

    #[test]
    fn network_validation_rejects_incompatible_assets() {
        let networks = crate::networks::NetworkCatalog::test_default();
        let cases = [
            ("ethereum", "ETH", true),
            ("ethereum", "BTC", false),
            ("bitcoin", "BTC", true),
            ("bitcoin", "ETH", false),
            ("tron", "USDT", true),
            ("tron", "USDC", false),
            ("unknown", "ETH", false),
        ];

        for (network, asset, expected) in cases {
            assert_eq!(
                validate_network(&networks, &Some(network.to_string()), asset, &[]).is_ok(),
                expected,
                "network={network}, asset={asset}"
            );
        }
    }

    #[test]
    fn canonicalizes_common_network_aliases() {
        assert_eq!(canonical_network_id("avalanche"), "avalanche-c");
        assert_eq!(canonical_network_id("avax"), "avalanche-c");
        assert_eq!(canonical_network_id("sol"), "solana");
        assert_eq!(canonical_network_id("TRC20"), "tron");
        assert_eq!(canonical_network_id("ethereum"), "ethereum");
    }

    #[test]
    fn same_asset_on_different_networks_is_allowed_for_provider_search() {
        let query = normalize_query(
            P2pRouteSearchQuery {
                source_fiat: "USDT".into(),
                target_fiat: "USDT".into(),
                source_amount: 125.0,
                source_network: Some("tron".into()),
                target_network: Some("ton".into()),
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(false),
                max_price_deviation_bps: Some(1_000),
                limit: Some(20),
                sources: None,
            },
            &["USDT".into()],
            &crate::networks::NetworkCatalog::test_default(),
            &[],
        )
        .expect("same-asset cross-network request should reach provider search");

        assert_eq!(query.source_network.as_deref(), Some("tron"));
        assert_eq!(query.target_network.as_deref(), Some("ton"));
    }

    #[test]
    fn crypto_to_fiat_route_preserves_the_selected_source_network() {
        let query = normalize_query(
            P2pRouteSearchQuery {
                source_fiat: "USDT".into(),
                target_fiat: "AMD".into(),
                source_amount: 125.0,
                source_network: Some("ethereum".into()),
                target_network: None,
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: Some("Ameriabank".into()),
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(false),
                max_price_deviation_bps: Some(1_000),
                limit: Some(20),
                sources: None,
            },
            &["USDT".into()],
            &crate::networks::NetworkCatalog::test_default(),
            &[],
        )
        .unwrap();
        let mut routes = Vec::new();

        compose_crypto_to_fiat_routes(
            &mut routes,
            &query,
            "USDT",
            &[offer(
                "binance",
                P2pSide::SellCrypto,
                "359.12",
                "1000",
                "100000",
            )],
        );

        assert_eq!(routes.len(), 1);
        assert_eq!(
            serde_json::to_value(&routes[0]).unwrap()["entry_network"],
            "ethereum"
        );
    }

    #[test]
    fn fixed_provider_network_rejects_incompatible_direct_and_composed_routes() {
        let mut direct_query = query(false);
        direct_query.target_currency = "USDT".into();
        direct_query.target_network = Some("tron".into());
        let mut solana_offer = offer(
            "bitcoin-center",
            P2pSide::BuyCrypto,
            "372",
            "50000",
            "10000000",
        );
        solana_offer.network = Some("solana".into());
        let mut routes = Vec::new();

        compose_fiat_to_crypto_routes(&mut routes, &direct_query, "USDT", &[solana_offer.clone()]);

        assert!(routes.is_empty());

        let mut tron_exit = offer("bncex", P2pSide::SellCrypto, "353", "10000", "50000000");
        tron_exit.network = Some("tron".into());
        compose_fiat_routes(
            &mut routes,
            &query(true),
            "USDT",
            &[solana_offer],
            &[tron_exit],
        );

        assert!(routes.is_empty());
    }

    #[test]
    fn composes_two_feasible_legs() {
        let mut routes = Vec::new();
        compose_fiat_routes(
            &mut routes,
            &query(false),
            "USDT",
            &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
            &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
        );
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].acquired_asset_amount, "250.00000000");
        assert_eq!(routes[0].target_amount, "20000.00");
        assert!(routes[0].same_venue);
    }

    #[test]
    fn composes_rub_to_amd_with_bncex_as_the_exit() {
        let mut route_query = query(true);
        route_query.source_currency = "RUB".into();
        route_query.target_currency = "AMD".into();
        let mut entry = offer("bybit", P2pSide::BuyCrypto, "80", "1000", "1000000");
        entry.fiat = "RUB".into();
        let mut exit = offer("bncex", P2pSide::SellCrypto, "353.06", "10000", "50000000");
        exit.fiat = "AMD".into();
        exit.market = P2pOfferMarket::DirectExchange;

        let mut routes = Vec::new();
        compose_fiat_routes(&mut routes, &route_query, "USDT", &[entry], &[exit]);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].source_fiat, "RUB");
        assert_eq!(routes[0].target_fiat, "AMD");
        assert_eq!(routes[0].entry_offer.as_ref().unwrap().source, "bybit");
        assert_eq!(routes[0].exit_offer.as_ref().unwrap().source, "bncex");
        assert!(!routes[0].same_venue);
        assert!(routes[0].requires_asset_transfer);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn composes_fiat_provider_route_across_intermediary_networks() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(ProgressiveSource)],
            Duration::from_secs(1),
        )
        .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "RUB".into(),
                target_fiat: "AMD".into(),
                source_amount: 100_000.0,
                source_network: None,
                target_network: None,
                bridge_fiat: None,
                assets: Some("USDT".into()),
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(40),
                sources: None,
            })
            .await
            .unwrap();
        let route = response
            .routes
            .iter()
            .find(|route| route.route_provider.as_deref() == Some("test-intents"))
            .expect("provider route should be included");
        assert_eq!(route.route_path.first().map(String::as_str), Some("RUB"));
        assert_eq!(route.route_path.last().map(String::as_str), Some("AMD"));
        assert!(route.route_path.iter().any(|path| path == "USDT@tron"));
        assert_eq!(route.entry_offer.as_ref().unwrap().source, "bybit");
        assert_eq!(route.exit_offer.as_ref().unwrap().source, "bybit");
        assert!(route.route_fees[0].asset.starts_with("USDT@"));
    }

    #[tokio::test]
    async fn keeps_independent_quotes_from_multiple_route_providers() {
        let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
            .with_route_providers(vec![
                Arc::new(PricedRouteProvider {
                    name: "near-intents",
                    multiplier: 0.99,
                    fee: "0.25",
                }),
                Arc::new(PricedRouteProvider {
                    name: "cow-swap",
                    multiplier: 0.97,
                    fee: "2.5",
                }),
            ]);

        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "USDT".into(),
                target_fiat: "USDC".into(),
                source_amount: 100.0,
                source_network: Some("ethereum".into()),
                target_network: Some("ethereum".into()),
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: None,
                max_price_deviation_bps: None,
                limit: Some(20),
                sources: Some("near-intents,cow-swap".into()),
            })
            .await
            .unwrap();

        let near = response
            .routes
            .iter()
            .find(|route| route.route_provider.as_deref() == Some("near-intents"))
            .unwrap();
        let cow = response
            .routes
            .iter()
            .find(|route| route.route_provider.as_deref() == Some("cow-swap"))
            .unwrap();
        assert_eq!(near.target_amount, "99");
        assert_eq!(cow.target_amount, "97");
        assert_eq!(near.route_fees[0].amount, "0.25");
        assert_eq!(cow.route_fees[0].amount, "2.5");
        assert_eq!(near.quote_expires_at, cow.quote_expires_at);
        assert_ne!(near.route_id, cow.route_id);

        let excluded = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "USDT".into(),
                target_fiat: "USDC".into(),
                source_amount: 100.0,
                source_network: Some("ethereum".into()),
                target_network: Some("ethereum".into()),
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: None,
                max_price_deviation_bps: None,
                limit: Some(20),
                sources: Some("binance".into()),
            })
            .await
            .unwrap();
        assert!(excluded.routes.is_empty());
    }

    #[tokio::test]
    async fn adds_direct_fiat_quotes_in_both_amd_rub_directions() {
        let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
            .with_fiat_route_providers(vec![Arc::new(FixedFiatRouteProvider)]);
        assert!(service.route_provider_names().contains("id-pay"));
        for (source_fiat, target_fiat, source_amount, expected_target) in [
            ("AMD", "RUB", 400.0, "100.00"),
            ("RUB", "AMD", 100.0, "400.00"),
        ] {
            let response = service
                .search_routes(P2pRouteSearchQuery {
                    source_fiat: source_fiat.into(),
                    target_fiat: target_fiat.into(),
                    source_amount,
                    source_network: None,
                    target_network: None,
                    bridge_fiat: None,
                    assets: None,
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: None,
                    max_price_deviation_bps: None,
                    limit: Some(20),
                    sources: Some("id-pay".into()),
                })
                .await
                .unwrap();
            let route = response
                .routes
                .iter()
                .find(|route| {
                    route
                        .entry_offer
                        .as_ref()
                        .map(|offer| offer.source.as_str())
                        == Some("id-pay")
                })
                .unwrap();
            assert_eq!(route.target_amount, expected_target);
            assert_eq!(route.route_kind, "fiat_to_fiat");
            assert!(route.same_venue);
            assert!(!route.requires_asset_transfer);
            assert!(route.route_provider.is_none());
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn composes_fiat_to_crypto_route_through_provider() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(ProgressiveSource)],
            Duration::from_secs(1),
        )
        .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "RUB".into(),
                target_fiat: "USDT".into(),
                source_amount: 100_000.0,
                source_network: None,
                target_network: Some("ton".into()),
                bridge_fiat: None,
                assets: Some("USDT".into()),
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(40),
                sources: None,
            })
            .await
            .unwrap();
        let route = response
            .routes
            .iter()
            .find(|route| {
                route.route_kind == "fiat_to_crypto"
                    && route.route_provider.as_deref() == Some("test-intents")
            })
            .expect("fiat-to-crypto provider route should be included");
        assert_eq!(route.route_provider.as_deref(), Some("test-intents"));
        assert_eq!(route.route_path.first().map(String::as_str), Some("RUB"));
        assert_eq!(
            route.route_path.last().map(String::as_str),
            Some("USDT@ton")
        );
        assert!(route.entry_offer.is_some());
        assert!(route.exit_offer.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn composes_small_amd_to_btc_near_route_after_filter_fallback() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(ProgressiveSource)],
            Duration::from_secs(1),
        )
        .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "AMD".into(),
                target_fiat: "BTC".into(),
                source_amount: 10_000.0,
                source_network: None,
                target_network: Some("near".into()),
                bridge_fiat: None,
                assets: Some("USDT".into()),
                intermediary_assets: None,
                source_payment_method: Some("Ameriabank".into()),
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: Some(20),
                min_completion_rate: Some(0.9),
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(40),
                sources: None,
            })
            .await
            .unwrap();

        let paths = response
            .routes
            .iter()
            .filter(|route| route.route_provider.as_deref() == Some("test-intents"))
            .map(|route| {
                assert!(route.target_amount.parse::<f64>().unwrap() > 0.0);
                assert!(!route.payment_methods_verified);
                route.route_path.clone()
            })
            .collect::<std::collections::HashSet<_>>();
        let expected_paths = [
            "avalanche-c",
            "ethereum",
            "gnosis",
            "near",
            "optimism",
            "scroll",
        ]
        .into_iter()
        .map(|network| {
            vec![
                "AMD".to_string(),
                format!("USDT@{network}"),
                "BTC@near".to_string(),
            ]
        })
        .collect::<std::collections::HashSet<_>>();
        assert!(
            expected_paths.is_subset(&paths),
            "missing routes: {:?}",
            expected_paths.difference(&paths)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn composes_crypto_to_crypto_provider_routes_for_supported_pairs() {
        let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
        let cases = [
            ("USDT", "USDT", "ethereum", "avalanche-c"),
            ("USDT", "USDC", "ethereum", "solana"),
            ("USDT", "BTC", "ethereum", "near"),
        ];

        for (source_asset, target_asset, source_network, target_network) in cases {
            let response = service
                .search_routes(P2pRouteSearchQuery {
                    source_fiat: source_asset.into(),
                    target_fiat: target_asset.into(),
                    source_amount: 1_000.0,
                    source_network: Some(source_network.into()),
                    target_network: Some(target_network.into()),
                    bridge_fiat: None,
                    assets: None,
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    merchant_only: Some(false),
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: Some(true),
                    max_price_deviation_bps: Some(1_000),
                    limit: Some(40),
                    sources: None,
                })
                .await
                .unwrap();
            let route = response
                .routes
                .iter()
                .find(|route| route.route_provider.as_deref() == Some("test-intents"))
                .unwrap_or_else(|| {
                    panic!(
                        "missing route for {source_asset}@{source_network} -> {target_asset}@{target_network}"
                    )
                });

            assert!(route.target_amount.parse::<f64>().unwrap() > 0.0);
            assert_eq!(
                route.route_path,
                vec![
                    format!("{source_asset}@{source_network}"),
                    format!("{target_asset}@{target_network}"),
                ]
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn composes_crypto_to_fiat_route_through_provider() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(ProgressiveSource)],
            Duration::from_secs(1),
        )
        .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "USDT".into(),
                target_fiat: "AMD".into(),
                source_amount: 100.0,
                source_network: Some("ethereum".into()),
                target_network: None,
                bridge_fiat: None,
                assets: Some("USDT".into()),
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(40),
                sources: None,
            })
            .await
            .unwrap();
        let route = response
            .routes
            .iter()
            .find(|route| {
                route.route_kind == "crypto_to_fiat"
                    && route.route_provider.as_deref() == Some("test-intents")
            })
            .expect("crypto-to-fiat provider route should be included");
        assert_eq!(route.route_provider.as_deref(), Some("test-intents"));
        assert_eq!(
            route.route_path.first().map(String::as_str),
            Some("USDT@ethereum")
        );
        assert_eq!(route.route_path.last().map(String::as_str), Some("AMD"));
        assert!(route.entry_offer.is_none());
        assert!(route.exit_offer.is_some());
    }

    #[test]
    fn route_count_deduplicates_before_applying_the_display_limit() {
        let mut normalized = query(false);
        normalized.limit = 1;
        let mut discovered = Vec::new();
        compose_fiat_routes(
            &mut discovered,
            &normalized,
            "USDT",
            &[
                offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000"),
                offer("bybit", P2pSide::BuyCrypto, "410", "1000", "200000"),
            ],
            &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
        );
        let duplicate = discovered[0].clone();
        let mut all_routes = HashMap::new();
        assert_eq!(merge_routes(&mut all_routes, discovered), 2);
        assert_eq!(merge_routes(&mut all_routes, vec![duplicate]), 0);

        let snapshot = response_snapshot(Uuid::nil(), &normalized, &all_routes, &[]);
        assert_eq!(snapshot.routes_found, 2);
        assert_eq!(snapshot.routes.len(), 1);
        assert!(!snapshot.routes[0].route_id.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn route_stream_reports_each_completed_asset_batch() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(ProgressiveSource)],
            Duration::from_secs(1),
        );
        let (updates, mut snapshots) = mpsc::channel(4);
        let search_id = Uuid::new_v4();
        let task = tokio::spawn(async move {
            service
                .stream_routes(
                    P2pRouteSearchQuery {
                        source_fiat: "AMD".into(),
                        target_fiat: "RUB".into(),
                        source_amount: 100_000.0,
                        source_network: None,
                        target_network: None,
                        bridge_fiat: None,
                        assets: Some("USDT,USDC".into()),
                        intermediary_assets: None,
                        source_payment_method: None,
                        target_payment_method: None,
                        merchant_only: Some(false),
                        min_orders: None,
                        min_completion_rate: None,
                        allow_cross_venue: Some(false),
                        max_price_deviation_bps: Some(1_000),
                        limit: Some(40),
                        sources: Some("bybit".into()),
                    },
                    search_id,
                    updates,
                )
                .await
        });

        let first = snapshots.recv().await.expect("first asset snapshot");
        let second = snapshots.recv().await.expect("second asset snapshot");
        let final_response = task.await.unwrap().unwrap();
        assert_eq!(first.routes_found, 1);
        assert_eq!(second.routes_found, 2);
        assert_eq!(final_response.routes_found, 2);
        assert_eq!(final_response.search_id, search_id);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn route_stream_publishes_a_fast_provider_before_slower_providers_finish() {
        let service = P2pSearchService::with_sources(
            vec![
                Arc::new(DelayedRouteSource {
                    name: "fast",
                    delay: Duration::from_millis(10),
                    price: "84",
                }),
                Arc::new(DelayedRouteSource {
                    name: "slow",
                    delay: Duration::from_millis(250),
                    price: "85",
                }),
            ],
            Duration::from_secs(1),
        );
        let (updates, mut snapshots) = mpsc::channel(4);
        let search_id = Uuid::new_v4();
        let task = tokio::spawn(async move {
            service
                .stream_routes(
                    P2pRouteSearchQuery {
                        source_fiat: "USDC".into(),
                        target_fiat: "RUB".into(),
                        source_amount: 100.0,
                        source_network: Some("ethereum".into()),
                        target_network: None,
                        bridge_fiat: None,
                        assets: None,
                        intermediary_assets: None,
                        source_payment_method: None,
                        target_payment_method: Some("Sberbank".into()),
                        merchant_only: Some(false),
                        min_orders: None,
                        min_completion_rate: None,
                        allow_cross_venue: Some(true),
                        max_price_deviation_bps: Some(1_000),
                        limit: Some(40),
                        sources: None,
                    },
                    search_id,
                    updates,
                )
                .await
        });

        let first = tokio::time::timeout(Duration::from_millis(100), snapshots.recv())
            .await
            .expect("fast provider should stream before the slow provider finishes")
            .expect("first provider snapshot");
        assert_eq!(first.routes_found, 1);
        assert_eq!(first.routes[0].exit_offer.as_ref().unwrap().source, "fast");
        assert!(!task.is_finished());

        let second = snapshots.recv().await.expect("second provider snapshot");
        let final_response = task.await.unwrap().unwrap();
        assert_eq!(second.routes_found, 2);
        assert_eq!(final_response.routes_found, 2);
    }

    #[test]
    fn rejects_cross_venue_and_impossible_target_limit() {
        let entry = offer("binance", P2pSide::BuyCrypto, "400", "1000", "200000");
        let exit = offer("bybit", P2pSide::SellCrypto, "80", "30000", "100000");
        let mut routes = Vec::new();
        compose_fiat_routes(
            &mut routes,
            &query(false),
            "USDT",
            std::slice::from_ref(&entry),
            std::slice::from_ref(&exit),
        );
        assert!(routes.is_empty());
        compose_fiat_routes(&mut routes, &query(true), "USDT", &[entry], &[exit]);
        assert!(routes.is_empty());
    }

    #[test]
    fn removes_large_price_outlier() {
        let offers = vec![
            offer("a", P2pSide::SellCrypto, "80", "1", "100000"),
            offer("b", P2pSide::SellCrypto, "81", "1", "100000"),
            offer("c", P2pSide::SellCrypto, "200", "1", "100000"),
        ];
        let filtered = reject_price_outliers(offers, 1_000);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn keeps_direct_exchange_quotes_outside_the_p2p_median() {
        let mut direct = offer("whitebird", P2pSide::SellCrypto, "70", "1", "100000");
        direct.market = crate::p2p::service::P2pOfferMarket::DirectExchange;
        let offers = vec![
            offer("a", P2pSide::SellCrypto, "80", "1", "100000"),
            offer("b", P2pSide::SellCrypto, "81", "1", "100000"),
            offer("c", P2pSide::SellCrypto, "82", "1", "100000"),
            direct,
        ];

        let filtered = reject_price_outliers(offers, 1_000);

        assert_eq!(filtered.len(), 4);
        assert!(filtered.iter().any(|offer| offer.source == "whitebird"));
    }

    #[test]
    fn keeps_offers_without_exact_deep_links_for_manual_fallback() {
        let mut entry = offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000");
        entry.source_url_is_exact = false;
        let exit = offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000");

        let mut routes = Vec::new();
        compose_fiat_routes(&mut routes, &query(false), "USDT", &[entry], &[exit]);

        assert_eq!(routes.len(), 1);
        assert!(!routes[0].entry_offer.as_ref().unwrap().source_url_is_exact);
        assert!(routes[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("deep-link")));
    }

    #[test]
    fn labels_route_when_venue_payment_ids_are_opaque() {
        let mut query = query(true);
        query.source_payment_method = Some("Ameriabank".into());
        query.target_payment_method = Some("Sberbank".into());
        let mut entry = offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000");
        let mut exit = offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000");
        entry.payment_methods = vec!["18".into()];
        exit.payment_methods = vec!["40".into()];

        let mut routes = Vec::new();
        compose_fiat_routes(&mut routes, &query, "USDT", &[entry], &[exit]);

        assert_eq!(routes.len(), 1);
        assert!(!routes[0].payment_methods_verified);
        assert_eq!(routes[0].warnings.len(), 3);
    }

    #[test]
    fn composes_crypto_to_crypto_route_without_fiat() {
        let query = NormalizedRouteQuery {
            source_currency: "ETH".into(),
            target_currency: "USDC".into(),
            source_amount: 0.04,
            source_network: Some("ethereum".into()),
            target_network: Some("ethereum".into()),
            assets: vec!["USDT".into()],
            assets_explicit: false,
            source_payment_method: None,
            target_payment_method: None,
            merchant_only: false,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: true,
            max_price_deviation_bps: 1_000,
            limit: 20,
            sources: None,
        };
        let tickers = vec![
            CryptoTicker {
                symbol: "ETHUSDT".into(),
                bid: 2_500.0,
                ask: 2_501.0,
            },
            CryptoTicker {
                symbol: "USDCUSDT".into(),
                bid: 0.999,
                ask: 1.001,
            },
        ];
        let mut routes = Vec::new();

        compose_crypto_market_routes(&mut routes, &query, "bybit", "ETH", "USDC", &tickers);

        assert_eq!(routes.len(), 1);
        let route = &routes[0];
        assert_eq!(route.source_fiat, "ETH");
        assert_eq!(route.target_fiat, "USDC");
        assert_eq!(route.source_network.as_deref(), Some("ethereum"));
        assert_eq!(route.target_network.as_deref(), Some("ethereum"));
        assert_eq!(route.bridge_currency.as_deref(), Some("USDT"));
        assert!(route.entry_offer.is_none());
        assert!(route.exit_offer.is_none());
        assert_eq!(route.market_path.as_ref().unwrap().venue, "bybit");
        assert!(route.target_amount.parse::<f64>().unwrap() > 99.0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "calls live Binance, Bybit, OKX, Bitget and Rapira public P2P endpoints"]
    async fn live_amd_to_rub_route_search() {
        let config = Config::from_env().unwrap();
        let service =
            P2pSearchService::from_config(&config, crate::networks::NetworkCatalog::test_default())
                .unwrap();
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "AMD".into(),
                target_fiat: "RUB".into(),
                source_amount: 100_000.0,
                source_network: None,
                target_network: None,
                bridge_fiat: None,
                assets: Some("USDT,USDC,BTC,ETH".into()),
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(false),
                max_price_deviation_bps: Some(1_000),
                limit: Some(10),
                sources: None,
            })
            .await
            .unwrap();

        eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
        assert!(response.routes.iter().all(|route| route.same_venue));
        assert_eq!(response.asset_statuses.len(), 4);
        assert!(response.asset_statuses.iter().all(|status| status
            .entry_sources
            .iter()
            .any(|source| source.ok)
            && status.exit_sources.iter().any(|source| source.ok)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "calls live Binance, Bybit, OKX, Bitget and Rapira public P2P endpoints"]
    async fn live_bank_filtered_amd_to_rub_route_search() {
        let config = Config::from_env().unwrap();
        let service =
            P2pSearchService::from_config(&config, crate::networks::NetworkCatalog::test_default())
                .unwrap();
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: "AMD".into(),
                target_fiat: "RUB".into(),
                source_amount: 100_000.0,
                source_network: None,
                target_network: None,
                bridge_fiat: None,
                assets: Some("USDT,USDC,BTC,ETH".into()),
                intermediary_assets: None,
                source_payment_method: Some("Ameriabank".into()),
                target_payment_method: Some("Sberbank".into()),
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(10),
                sources: None,
            })
            .await
            .unwrap();

        eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
        assert!(response.routes.iter().all(|route| {
            route
                .entry_offer
                .as_ref()
                .is_some_and(|offer| offer.side == P2pSide::BuyCrypto)
                && route
                    .exit_offer
                    .as_ref()
                    .is_some_and(|offer| offer.side == P2pSide::SellCrypto)
        }));
    }
}

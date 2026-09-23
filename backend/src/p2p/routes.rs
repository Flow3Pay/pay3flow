use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::p2p::service::{
    normalize_sources, P2pOffer, P2pSearchQuery, P2pSearchService, P2pSide, PaymentMethodMatch,
    SourceStatus,
};
use crate::p2p::spot::CryptoTicker;
use crate::service_reputation::{CombinedReputation, RouteServiceStats, ServiceLink};

const DEFAULT_ROUTE_LIMIT: usize = 20;
const MAX_ROUTE_LIMIT: usize = 100;
const LEG_SEARCH_LIMIT: usize = 60;
const DEFAULT_MAX_PRICE_DEVIATION_BPS: u32 = 1_000;

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
    /// Defaults to `P2P_SEARCH_ASSETS` when omitted.
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub service_links: Vec<ServiceLink>,
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

struct NormalizedRouteQuery {
    source_currency: String,
    target_currency: String,
    source_amount: f64,
    source_network: Option<String>,
    target_network: Option<String>,
    assets: Vec<String>,
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
        let query = normalize_query(query, &self.default_assets)?;
        let mut routes = HashMap::new();
        let mut asset_statuses = Vec::new();
        match (
            is_crypto(&query.source_currency),
            is_crypto(&query.target_currency),
        ) {
            (false, false) => {
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
                    let entry = entry?;
                    let exit = exit?;
                    let entry_offers =
                        reject_price_outliers(entry.offers, query.max_price_deviation_bps);
                    let exit_offers =
                        reject_price_outliers(exit.offers, query.max_price_deviation_bps);
                    let mut discovered = Vec::new();
                    compose_fiat_routes(
                        &mut discovered,
                        &query,
                        &asset,
                        &entry_offers,
                        &exit_offers,
                    );
                    let routes_built = merge_routes(&mut routes, discovered);
                    push_asset_status(
                        &mut asset_statuses,
                        asset,
                        &entry.sources,
                        &exit.sources,
                        &entry_offers,
                        &exit_offers,
                        routes_built,
                    );
                    publish_update(
                        updates.as_ref(),
                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                    )
                    .await;
                }
            }
            (false, true) => {
                let asset = query.target_currency.clone();
                let response = self
                    .search(leg_query(
                        &query.source_currency,
                        &asset,
                        P2pSide::BuyCrypto,
                        Some(query.source_amount),
                        query.source_payment_method.clone(),
                        &query,
                    ))
                    .await?;
                let offers = reject_price_outliers(response.offers, query.max_price_deviation_bps);
                let mut discovered = Vec::new();
                compose_fiat_to_crypto_routes(&mut discovered, &query, &asset, &offers);
                let routes_built = merge_routes(&mut routes, discovered);
                push_asset_status(
                    &mut asset_statuses,
                    asset,
                    &response.sources,
                    &[],
                    &offers,
                    &[],
                    routes_built,
                );
                publish_update(
                    updates.as_ref(),
                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                )
                .await;
            }
            (true, false) => {
                let asset = query.source_currency.clone();
                let response = self
                    .search(leg_query(
                        &query.target_currency,
                        &asset,
                        P2pSide::SellCrypto,
                        None,
                        query.target_payment_method.clone(),
                        &query,
                    ))
                    .await?;
                let offers = reject_price_outliers(response.offers, query.max_price_deviation_bps);
                let mut discovered = Vec::new();
                compose_crypto_to_fiat_routes(&mut discovered, &query, &asset, &offers);
                let routes_built = merge_routes(&mut routes, discovered);
                push_asset_status(
                    &mut asset_statuses,
                    asset,
                    &[],
                    &response.sources,
                    &[],
                    &offers,
                    routes_built,
                );
                publish_update(
                    updates.as_ref(),
                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                )
                .await;
            }
            (true, true) => {
                let source_asset = query.source_currency.clone();
                let target_asset = query.target_currency.clone();
                for (venue, result) in self.search_market_tickers(query.sources.as_deref()).await {
                    let Ok(tickers) = result else {
                        continue;
                    };
                    let mut discovered = Vec::new();
                    compose_crypto_market_routes(
                        &mut discovered,
                        &query,
                        &venue,
                        &source_asset,
                        &target_asset,
                        &tickers,
                    );
                    merge_routes(&mut routes, discovered);
                    publish_update(
                        updates.as_ref(),
                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                    )
                    .await;
                }
            }
        }
        Ok(response_snapshot(
            search_id,
            &query,
            &routes,
            &asset_statuses,
        ))
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
        .map(|offer| format!("{}:{}", offer.source, offer.ad_id))
        .unwrap_or_default();
    let exit = route
        .exit_offer
        .as_ref()
        .map(|offer| format!("{}:{}", offer.source, offer.ad_id))
        .unwrap_or_default();
    let market = route
        .market_path
        .as_ref()
        .map(|path| format!("{}:{}:{}", path.venue, path.source_pair, path.target_pair))
        .unwrap_or_default();
    let identity = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        route.route_kind,
        route.asset,
        route.source_network.as_deref().unwrap_or_default(),
        route.target_network.as_deref().unwrap_or_default(),
        route.bridge_currency.as_deref().unwrap_or_default(),
        entry,
        exit,
        market,
        route.source_amount,
    );
    format!("{:x}", Sha256::digest(identity.as_bytes()))
}

fn normalize_query(
    query: P2pRouteSearchQuery,
    default_assets: &[String],
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
    let source_network = validate_network(&query.source_network, &source_currency)?;
    let target_network = validate_network(&query.target_network, &target_currency)?;
    if source_currency.eq_ignore_ascii_case(&target_currency) {
        match (&source_network, &target_network) {
            (Some(source_network), Some(target_network)) if source_network != target_network => {
                bail!(
                    "cross-network bridge routes are not available: no bridge quote provider is configured for {source_currency} from {source_network} to {target_network}"
                );
            }
            _ => bail!("source and target asset/network must differ"),
        }
    }

    Ok(NormalizedRouteQuery {
        source_currency,
        target_currency,
        source_amount: query.source_amount,
        source_network,
        target_network,
        assets,
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

fn validate_network(network_id: &Option<String>, currency: &str) -> Result<Option<String>> {
    let Some(network_id) = network_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let network = crate::networks::catalog().into_iter().find(|network| {
        network.id.eq_ignore_ascii_case(network_id)
            && network
                .currencies
                .iter()
                .any(|item| item.eq_ignore_ascii_case(currency))
    });
    match network {
        Some(network) => Ok(Some(network.id)),
        None => bail!("network {network_id} is not compatible with {currency}"),
    }
}

fn is_crypto(currency: &str) -> bool {
    crate::networks::is_supported_asset(currency)
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
            offer
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
                entry_network: None,
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
                payment_methods_verified,
                entry_offer: Some(entry.clone()),
                exit_offer: Some(exit.clone()),
                warnings,
                services: Vec::new(),
                reputation: None,
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
            entry_network: query.target_network.clone(),
            source_network: query.source_network.clone(),
            target_network: query.target_network.clone(),
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
            payment_methods_verified,
            entry_offer: Some(offer.clone()),
            exit_offer: None,
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
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
            entry_network: query.source_network.clone(),
            source_network: query.source_network.clone(),
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
            payment_methods_verified,
            entry_offer: None,
            exit_offer: Some(offer.clone()),
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            service_links: Vec::new(),
        });
    }
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
            payment_methods_verified: true,
            entry_offer: None,
            exit_offer: None,
            warnings: vec![
                "Spot-market estimate only: trading fees, slippage and execution are not guaranteed.".into(),
                "Deposit and withdrawal network availability and fees are not verified by the selected venue.".into(),
            ],
            services: Vec::new(),
            reputation: None,
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

fn push_asset_status(
    statuses: &mut Vec<RouteAssetStatus>,
    asset: String,
    entry_sources: &[SourceStatus],
    exit_sources: &[SourceStatus],
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
    routes_built: usize,
) {
    statuses.push(RouteAssetStatus {
        asset,
        entry_offers: entry_offers.len(),
        exit_offers: exit_offers.len(),
        routes_built,
        can_exchange_to_target: routes_built > 0,
        entry_sources: entry_sources.to_vec(),
        exit_sources: exit_sources.to_vec(),
    });
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
    use crate::p2p::Advertiser;
    use async_trait::async_trait;

    struct ProgressiveSource;

    #[async_trait]
    impl P2pSource for ProgressiveSource {
        fn name(&self) -> &'static str {
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
            Ok(vec![result])
        }
    }

    fn offer(source: &str, side: P2pSide, price: &str, min: &str, max: &str) -> P2pOffer {
        P2pOffer {
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
    fn recognizes_every_exchange_asset_as_crypto() {
        for asset in crate::networks::CRYPTO_ASSETS {
            assert!(is_crypto(asset), "asset should use crypto routing: {asset}");
        }
        assert!(!is_crypto("AMD"));
    }

    #[test]
    fn network_validation_rejects_incompatible_assets() {
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
                validate_network(&Some(network.to_string()), asset).is_ok(),
                expected,
                "network={network}, asset={asset}"
            );
        }
    }

    #[test]
    fn same_asset_on_different_networks_reports_missing_bridge_provider() {
        let error = normalize_query(
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
        )
        .err()
        .expect("same-asset cross-network request should be rejected without a bridge provider");

        assert_eq!(
            error.to_string(),
            "cross-network bridge routes are not available: no bridge quote provider is configured for USDT from tron to ton"
        );
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
        let service = P2pSearchService::from_config(&config).unwrap();
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
        let service = P2pSearchService::from_config(&config).unwrap();
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

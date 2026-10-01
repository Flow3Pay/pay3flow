use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
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
mod composition;
mod execution;
mod provider_catalog;
mod provider_discovery;
mod provider_fiat;
mod provider_fiat_crypto;
mod providers;
mod query;
mod response;
use providers::*;
use query::*;
use response::*;
mod model;
use composition::*;
pub use model::{
    CryptoMarketPath, ExchangeMode, P2pRoute, P2pRouteSearchQuery, P2pRouteSearchResponse,
    RouteAssetStatus, RouteFee,
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

#[cfg(test)]
mod tests;

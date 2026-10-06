use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::mpsc;

use crate::p2p::routes::{
    compose_crypto_to_fiat_routes, compose_fiat_routes, compose_fiat_to_crypto_routes,
    matching_offers, merge_routes, reject_price_outliers, upsert_asset_status,
    NormalizedRouteQuery,
};
use crate::p2p::{
    P2pOfferMarket, P2pRoute, P2pSearchQuery, P2pSearchResponse, P2pSearchService, P2pSide,
    RouteAssetStatus,
};

use super::super::model::RouteBatch;
use super::super::streaming::send_batch;

pub(super) async fn produce_fiat_to_fiat(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let mut searches = query
        .assets
        .iter()
        .map(|asset| {
            let entry =
                super::leg_query_for(query, &query.source_currency, asset, P2pSide::BuyCrypto);
            let exit =
                super::leg_query_for(query, &query.target_currency, asset, P2pSide::SellCrypto);
            stream_asset(
                service,
                asset.clone(),
                entry,
                exit,
                query.offer_market(),
                batches.clone(),
            )
        })
        .collect::<FuturesUnordered<_>>();
    drop(batches);

    while let Some(result) = searches.next().await {
        result?;
    }
    Ok(())
}

async fn stream_asset(
    service: &P2pSearchService,
    asset: String,
    entry_query: P2pSearchQuery,
    exit_query: P2pSearchQuery,
    market: Option<P2pOfferMarket>,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
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
    let mut published = HashSet::new();

    while !entry_done || !exit_done {
        tokio::select! {
            snapshot = entry_snapshots.recv(), if entry_channel_open => match snapshot {
                Some(snapshot) => entry = Some(snapshot),
                None => entry_channel_open = false,
            },
            snapshot = exit_snapshots.recv(), if exit_channel_open => match snapshot {
                Some(snapshot) => exit = Some(snapshot),
                None => exit_channel_open = false,
            },
            result = &mut entry_search, if !entry_done => {
                entry = Some(result?);
                entry_done = true;
                entry_channel_open = false;
            },
            result = &mut exit_search, if !exit_done => {
                exit = Some(result?);
                exit_done = true;
                exit_channel_open = false;
            },
        }

        if let (Some(entry), Some(exit)) = (&entry, &exit) {
            let signature = (entry.sources.len(), exit.sources.len());
            if published.insert(signature)
                && !send_batch(
                    &batches,
                    RouteBatch::FiatAsset {
                        asset: asset.clone(),
                        entry: Box::new(entry.clone()),
                        exit: Box::new(exit.clone()),
                    },
                )
                .await
            {
                return Ok(());
            }
        }
    }

    if entry.is_none() || exit.is_none() {
        return Err(anyhow!("fiat route leg search returned no response"));
    }
    Ok(())
}

pub(super) async fn produce_fiat_to_crypto(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let asset = query.target_currency.clone();
    let leg = super::leg_query_for(query, &query.source_currency, &asset, P2pSide::BuyCrypto);
    stream_single_leg(service, query, asset, leg, true, batches).await
}

pub(super) async fn produce_crypto_to_fiat(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let asset = query.source_currency.clone();
    let mut leg = super::leg_query_for(query, &query.target_currency, &asset, P2pSide::SellCrypto);
    leg.asset_amount = Some(query.source_amount);
    stream_single_leg(service, query, asset, leg, false, batches).await
}

async fn stream_single_leg(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    asset: String,
    leg: P2pSearchQuery,
    is_entry: bool,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let (updates, mut snapshots) = mpsc::channel(16);
    let search = service.stream_search_market(leg, updates, query.offer_market());
    tokio::pin!(search);
    let mut published_source_counts = HashSet::new();
    let mut channel_open = true;

    loop {
        let (response, finished) = tokio::select! {
            snapshot = snapshots.recv(), if channel_open => match snapshot {
                Some(snapshot) => (snapshot, false),
                None => {
                    channel_open = false;
                    continue;
                },
            },
            result = &mut search => {
                let response = result?;
                if published_source_counts.contains(&response.sources.len()) {
                    break;
                }
                (response, true)
            },
        };
        published_source_counts.insert(response.sources.len());
        let batch = if is_entry {
            RouteBatch::FiatToCrypto {
                asset: asset.clone(),
                response: Box::new(response),
            }
        } else {
            RouteBatch::CryptoToFiat {
                asset: asset.clone(),
                response: Box::new(response),
            }
        };
        if !send_batch(&batches, batch).await {
            break;
        }
        if finished {
            break;
        }
    }
    Ok(())
}

pub(super) fn apply_fiat_asset(
    routes: &mut HashMap<String, P2pRoute>,
    statuses: &mut Vec<RouteAssetStatus>,
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
    let exhaustive =
        compose_fiat_routes(&mut discovered, query, asset, &entry_offers, &exit_offers);
    let routes_built = discovered.len();
    routes.retain(|_, route| {
        route.route_kind != "fiat_to_fiat"
            || route.asset != asset
            || route.route_provider.is_some()
            || route.entry_offer.is_none()
            || route.exit_offer.is_none()
    });
    merge_routes(routes, discovered);
    upsert_asset_status(
        statuses,
        asset.to_string(),
        &entry.sources,
        &exit.sources,
        &entry_offers,
        &exit_offers,
        routes_built,
        exhaustive,
        Some(entry.source.as_str()),
        Some(exit.source.as_str()),
    );
}

pub(super) fn apply_fiat_to_crypto(
    routes: &mut HashMap<String, P2pRoute>,
    statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = matching_offers(&response.offers, query);
    let mut discovered = Vec::new();
    compose_fiat_to_crypto_routes(&mut discovered, query, asset, &offers);
    let routes_built = discovered.len();
    routes
        .retain(|_, route| route.route_kind != "fiat_to_crypto" || route.route_provider.is_some());
    merge_routes(routes, discovered);
    upsert_asset_status(
        statuses,
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

pub(super) fn apply_crypto_to_fiat(
    routes: &mut HashMap<String, P2pRoute>,
    statuses: &mut Vec<RouteAssetStatus>,
    query: &NormalizedRouteQuery,
    asset: &str,
    response: &P2pSearchResponse,
) {
    let offers = matching_offers(&response.offers, query);
    let mut discovered = Vec::new();
    compose_crypto_to_fiat_routes(&mut discovered, query, asset, &offers);
    let routes_built = discovered.len();
    routes
        .retain(|_, route| route.route_kind != "crypto_to_fiat" || route.route_provider.is_some());
    merge_routes(routes, discovered);
    upsert_asset_status(
        statuses,
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

use std::sync::Arc;

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::mpsc;

use crate::p2p::routes::{
    compose_fiat_spot_routes, conversion_quote, intermediary_asset_priority, leg_query,
    matching_offers, reject_price_outliers, NormalizedRouteQuery, MAX_PROVIDER_ASSETS,
};
use crate::p2p::{P2pSearchService, P2pSide};

use super::super::model::RouteBatch;
use super::super::streaming::send_batch;

/// Streams same-venue P2P entry plus Spot conversion as each market finishes discovery.
pub(super) async fn produce_fiat_spot_routes(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let (updates, mut markets) = mpsc::channel(8);
    let market_search = service.stream_market_tickers(query.sources.as_deref(), updates);
    tokio::pin!(market_search);
    let mut market_search_done = false;
    let mut market_channel_open = true;
    let mut entries = FuturesUnordered::new();

    while !market_search_done || market_channel_open || !entries.is_empty() {
        tokio::select! {
            () = &mut market_search, if !market_search_done => market_search_done = true,
            market = markets.recv(), if market_channel_open => {
                let (venue, tickers) = match market {
                    Some((venue, Ok(tickers))) => (venue, tickers),
                    Some((_, Err(_))) => continue,
                    None => {
                        market_channel_open = false;
                        continue;
                    }
                };
                let mut assets = query.assets.iter()
                    .filter(|asset| !asset.eq_ignore_ascii_case(&query.target_currency))
                    .filter(|asset| conversion_quote(&tickers, asset, &query.target_currency).is_some())
                    .cloned().collect::<Vec<_>>();
                assets.sort_by_key(|asset| intermediary_asset_priority(asset, &query.target_currency));
                assets.dedup();
                let tickers = Arc::new(tickers);
                for asset in assets.into_iter().take(MAX_PROVIDER_ASSETS) {
                    let tickers = Arc::clone(&tickers);
                    let venue = venue.clone();
                    entries.push(async move {
                        let mut leg = leg_query(
                            &query.source_currency, &asset, P2pSide::BuyCrypto,
                            Some(query.source_amount), query.source_payment_method.clone(), query,
                        );
                        // P2P purchase and Spot trade must belong to the same venue.
                        leg.sources = Some(venue.clone());
                        let response = service.search_market(leg, query.offer_market()).await.ok()?;
                        let offers = reject_price_outliers(
                            matching_offers(&response.offers, query), query.max_price_deviation_bps,
                        );
                        Some(compose_fiat_spot_routes(query, &venue, &asset, &offers, &tickers))
                    });
                }
            }
            result = entries.next(), if !entries.is_empty() => {
                if let Some(Some(routes)) = result {
                    if !routes.is_empty() && !send_batch(&batches, RouteBatch::Routes { routes, exhaustive: true }).await {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

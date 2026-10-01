use anyhow::Result;
use tokio::sync::mpsc;

use crate::p2p::routes::{compose_crypto_market_routes, NormalizedRouteQuery};
use crate::p2p::P2pSearchService;

use super::super::model::RouteBatch;
use super::super::streaming::send_batch;

pub(super) async fn produce_market_routes(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let (updates, mut snapshots) = mpsc::channel(8);
    let search = service.stream_market_tickers(query.sources.as_deref(), updates);
    tokio::pin!(search);
    let mut finished = false;
    let mut channel_open = true;

    while !finished {
        tokio::select! {
            result = snapshots.recv(), if channel_open => {
                let Some(result) = result else {
                    channel_open = false;
                    continue;
                };
                if let Some(routes) = market_batch(query, result) {
                    if !send_batch(&batches, RouteBatch::Routes { routes, exhaustive: true }).await {
                        return Ok(());
                    }
                }
            }
            () = &mut search => {
                while let Ok(result) = snapshots.try_recv() {
                    if let Some(routes) = market_batch(query, result) {
                        if !send_batch(&batches, RouteBatch::Routes { routes, exhaustive: true }).await {
                            return Ok(());
                        }
                    }
                }
                finished = true;
            }
        }
    }
    Ok(())
}

fn market_batch(
    query: &NormalizedRouteQuery,
    (venue, result): (String, Result<Vec<crate::p2p::spot::CryptoTicker>>),
) -> Option<Vec<crate::p2p::P2pRoute>> {
    let tickers = result.ok()?;
    let mut routes = Vec::new();
    compose_crypto_market_routes(
        &mut routes,
        query,
        &venue,
        &query.source_currency,
        &query.target_currency,
        &tickers,
    );
    Some(routes)
}

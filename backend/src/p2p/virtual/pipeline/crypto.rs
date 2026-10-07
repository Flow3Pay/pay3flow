use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::p2p::routes::{
    compose_crypto_market_cycles, compose_crypto_market_routes, NormalizedRouteQuery,
};
use crate::p2p::{P2pSearchService, SourceStatus};

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
    let started = Instant::now();
    let mut markets = HashMap::new();
    let mut channel_open = true;

    while !finished {
        tokio::select! {
            result = snapshots.recv(), if channel_open => {
                let Some(result) = result else {
                    channel_open = false;
                    continue;
                };
                if !send_batch(&batches, market_batch(query, result, &mut markets, started)).await {
                        return Ok(());
                }
            }
            () = &mut search => {
                while let Ok(result) = snapshots.try_recv() {
                    if !send_batch(&batches, market_batch(query, result, &mut markets, started)).await {
                            return Ok(());
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
    markets: &mut HashMap<String, Vec<crate::p2p::spot::CryptoTicker>>,
    started: Instant,
) -> RouteBatch {
    let status = SourceStatus {
        source: venue.clone(),
        ok: result.is_ok(),
        cached: false,
        latency_ms: started.elapsed().as_millis(),
        offers_found: result.as_ref().map(Vec::len).unwrap_or(0),
        error: result.as_ref().err().map(ToString::to_string),
    };
    let mut routes = Vec::new();
    if let Ok(tickers) = result {
        if query.is_crypto_cycle() {
            markets.insert(venue, tickers);
            routes = compose_crypto_market_cycles(query, markets);
        } else {
            compose_crypto_market_routes(
                &mut routes,
                query,
                &venue,
                &query.source_currency,
                &query.target_currency,
                &tickers,
            );
        }
    }
    RouteBatch::ProviderResult {
        status,
        routes,
        exhaustive: !query.is_crypto_cycle(),
    }
}

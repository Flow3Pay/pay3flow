mod cache;
mod crypto;
mod fiat;
mod provider;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::p2p::routes::{
    is_crypto_currency, leg_query, merge_routes, normalize_query, response_snapshot,
    NormalizedRouteQuery,
};
use crate::p2p::{
    P2pRouteSearchQuery, P2pRouteSearchResponse, P2pSearchQuery, P2pSearchService, P2pSide,
};

use super::model::RouteBatch;
use super::streaming::{publish_snapshot, route_batch_channel};

type Producer<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

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
        let route_cache_key = cache::route_cache_key(&query);
        if let Some(key) = route_cache_key.as_deref() {
            if let Some(response) = cache::cached_routes(self, key, search_id).await {
                publish_snapshot(updates.as_ref(), response.clone()).await;
                return Ok(response);
            }
        }
        let source_is_crypto =
            is_crypto_currency(&query.source_currency, &self.networks, &provider_assets);
        let target_is_crypto =
            is_crypto_currency(&query.target_currency, &self.networks, &provider_assets);
        let (batch_sender, mut batches) = route_batch_channel();
        let mut producers = FuturesUnordered::<Producer<'_>>::new();

        match (source_is_crypto, target_is_crypto) {
            (false, false) => producers.push(Box::pin(fiat::produce_fiat_to_fiat(
                self,
                &query,
                batch_sender.clone(),
            ))),
            (false, true) => producers.push(Box::pin(fiat::produce_fiat_to_crypto(
                self,
                &query,
                batch_sender.clone(),
            ))),
            (true, false) => producers.push(Box::pin(fiat::produce_crypto_to_fiat(
                self,
                &query,
                batch_sender.clone(),
            ))),
            (true, true) if query.includes_exchangers() => {
                producers.push(Box::pin(crypto::produce_market_routes(
                    self,
                    &query,
                    batch_sender.clone(),
                )));
            }
            (true, true) => {}
        }
        if query.includes_exchangers() {
            producers.push(Box::pin(provider::produce_workflow_routes(
                self,
                &query,
                source_is_crypto,
                target_is_crypto,
                batch_sender.clone(),
            )));
            if !source_is_crypto && !target_is_crypto {
                producers.push(Box::pin(provider::produce_direct_fiat_routes(
                    self,
                    &query,
                    batch_sender.clone(),
                )));
            }
        }
        drop(batch_sender);

        let mut routes = HashMap::new();
        let mut asset_statuses = Vec::new();
        let mut provider_statuses = HashMap::new();
        let mut routes_exhaustive = true;
        let mut channel_open = true;
        while channel_open || !producers.is_empty() {
            tokio::select! {
                batch = batches.recv(), if channel_open => {
                    let Some(batch) = batch else {
                        channel_open = false;
                        continue;
                    };
                    let changed = match batch {
                        RouteBatch::FiatAsset { asset, entry, exit } => {
                            fiat::apply_fiat_asset(
                                &mut routes,
                                &mut asset_statuses,
                                &query,
                                &asset,
                                &entry,
                                &exit,
                            );
                            true
                        }
                        RouteBatch::FiatToCrypto { asset, response } => {
                            fiat::apply_fiat_to_crypto(
                                &mut routes,
                                &mut asset_statuses,
                                &query,
                                &asset,
                                &response,
                            );
                            true
                        }
                        RouteBatch::CryptoToFiat { asset, response } => {
                            fiat::apply_crypto_to_fiat(
                                &mut routes,
                                &mut asset_statuses,
                                &query,
                                &asset,
                                &response,
                            );
                            true
                        }
                        RouteBatch::Routes { routes: discovered, exhaustive } => {
                            routes_exhaustive &= exhaustive;
                            merge_routes(&mut routes, discovered) > 0
                        }
                        RouteBatch::ProviderResult { status, routes: discovered, exhaustive } => {
                            provider_statuses.insert(status.source.clone(), status);
                            routes_exhaustive &= exhaustive;
                            merge_routes(&mut routes, discovered);
                            true
                        }
                    };
                    if changed {
                        publish_snapshot(
                            updates.as_ref(),
                            response_snapshot(
                                search_id,
                                &query,
                                &routes,
                                &asset_statuses,
                                &provider_statuses,
                            ),
                        ).await;
                    }
                }
                result = producers.next(), if !producers.is_empty() => {
                    if let Some(result) = result {
                        result?;
                    }
                }
            }
        }

        let mut response = response_snapshot(
            search_id,
            &query,
            &routes,
            &asset_statuses,
            &provider_statuses,
        );
        response.routes_exhaustive &= routes_exhaustive;
        if let Some(key) = route_cache_key.as_deref() {
            cache::cache_routes(self, key, &response).await;
        }
        tracing::info!(
            source_currency = %query.source_currency,
            target_currency = %query.target_currency,
            routes_found = response.routes_found,
            routes_exhaustive = response.routes_exhaustive,
            provider_assets_ms = provider_assets_elapsed.as_millis(),
            pipeline_ms = started.elapsed().saturating_sub(provider_assets_elapsed).as_millis(),
            total_ms = started.elapsed().as_millis(),
            "p2p.route_search.completed"
        );
        Ok(response)
    }
}

fn leg_query_for(
    query: &NormalizedRouteQuery,
    fiat: &str,
    asset: &str,
    side: P2pSide,
) -> P2pSearchQuery {
    let (amount, payment_method) = match side {
        P2pSide::BuyCrypto => (
            Some(query.source_amount),
            query.source_payment_method.clone(),
        ),
        P2pSide::SellCrypto => (None, query.target_payment_method.clone()),
    };
    leg_query(fiat, asset, side, amount, payment_method, query)
}

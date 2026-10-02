mod discovery;
mod fiat;
mod fiat_crypto;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::mpsc;

use crate::p2p::routes::*;
use crate::p2p::service::PaymentMethodMatch;
use crate::p2p::{
    Advertiser, FiatRouteQuote, P2pOffer, P2pOfferMarket, P2pRoute, P2pSearchService, P2pSide,
    SourceStatus,
};
use crate::route_engine::{Amount, Asset, PublicRouteQuote};

use super::super::model::RouteBatch;
use super::super::streaming::send_batch;

pub(super) async fn produce_workflow_routes(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    source_is_crypto: bool,
    target_is_crypto: bool,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    if !query.includes_exchangers() {
        return Ok(());
    }
    let (_routes, exhaustive) = match (source_is_crypto, target_is_crypto) {
        (false, false) => {
            service
                .search_fiat_provider_routes(query, Some(&batches))
                .await
        }
        (false, true) => (
            service
                .search_fiat_to_crypto_provider_routes(query, Some(&batches))
                .await,
            true,
        ),
        (true, false) => (
            service
                .search_crypto_to_fiat_provider_routes(query, Some(&batches))
                .await,
            true,
        ),
        (true, true) => (
            service.search_provider_routes(query, Some(&batches)).await,
            true,
        ),
    };
    if !exhaustive {
        send_batch(
            &batches,
            RouteBatch::Routes {
                routes: Vec::new(),
                exhaustive,
            },
        )
        .await;
    }
    Ok(())
}

async fn emit_routes(batches: Option<&mpsc::Sender<RouteBatch>>, routes: &[P2pRoute]) {
    if let Some(batches) = batches.filter(|_| !routes.is_empty()) {
        send_batch(
            batches,
            RouteBatch::Routes {
                routes: routes.to_vec(),
                exhaustive: true,
            },
        )
        .await;
    }
}

pub(super) async fn produce_direct_fiat_routes(
    service: &P2pSearchService,
    query: &NormalizedRouteQuery,
    batches: mpsc::Sender<RouteBatch>,
) -> Result<()> {
    let mut searches = service
        .fiat_route_providers
        .iter()
        .filter(|provider| source_selected(query, provider.name()))
        .cloned()
        .map(|provider| async move {
            let started = Instant::now();
            let provider_name = provider.name().to_string();
            if !provider.supports_pair(&query.source_currency, &query.target_currency) {
                return (
                    None,
                    SourceStatus {
                        source: provider_name,
                        ok: true,
                        cached: false,
                        latency_ms: started.elapsed().as_millis(),
                        offers_found: 0,
                        error: None,
                        avg_latency_ms: None,
                    },
                );
            }
            let key = fiat_quote_key(
                provider.name(),
                &query.source_currency,
                &query.target_currency,
                query.source_amount,
            );
            if let Some(quote) = service.cached_fiat_quote(&key) {
                return (
                    Some(quote),
                    SourceStatus {
                        source: provider_name,
                        ok: true,
cached: true,
                        latency_ms: 0,
                        offers_found: 1,
                        error: None,
                        avg_latency_ms: None,
                    },
                );
            }
            if service.has_fmatch_backend() {
                service.refresh_fiat_quote(
                    key,
                    provider,
                    query.source_currency.clone(),
                    query.target_currency.clone(),
                    query.source_amount,
                );
                return (
                    None,
                    SourceStatus {
                        source: provider_name,
                        ok: true,
                        cached: false,
                        latency_ms: started.elapsed().as_millis(),
                        offers_found: 0,
                        error: None,
                        avg_latency_ms: None,
                    },
                );
            }
            let result = provider
                .quote(
                    &query.source_currency,
                    &query.target_currency,
                    query.source_amount,
                )
                .await;
            let status = SourceStatus {
                source: provider_name,
                ok: result.is_ok(),
                cached: false,
                latency_ms: started.elapsed().as_millis(),
                offers_found: usize::from(result.is_ok()),
                error: result.as_ref().err().map(ToString::to_string),
                avg_latency_ms: None,
            };
            (result.ok(), status)
        })
        .collect::<FuturesUnordered<_>>();

    while let Some((quote, status)) = searches.next().await {
        let routes = quote
            .and_then(|quote| direct_fiat_route(query, quote))
            .into_iter()
            .collect();
        if !send_batch(
            &batches,
            RouteBatch::ProviderResult {
                status,
                routes,
                exhaustive: true,
            },
        )
        .await
        {
            break;
        }
    }
    Ok(())
}

fn direct_fiat_route(query: &NormalizedRouteQuery, quote: FiatRouteQuote) -> Option<P2pRoute> {
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
        return None;
    }
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
        price: fixed(quote.source_amount / quote.target_amount, 12),
        available_asset: "1000000000".into(),
        min_fiat: "1".into(),
        max_fiat: "1000000000".into(),
        payment_methods,
        pay_time_limit_minutes: None,
        advertiser: Advertiser {
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
    Some(P2pRoute {
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
    })
}

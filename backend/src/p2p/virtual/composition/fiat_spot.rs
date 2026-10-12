use super::*;
use crate::p2p::P2pSide;

/// Buy a listed intermediate asset, then convert it on the same venue's Spot market.
pub(in crate::p2p) fn compose_fiat_spot_routes(
    query: &NormalizedRouteQuery,
    venue: &str,
    intermediary: &str,
    offers: &[P2pOffer],
    tickers: &[CryptoTicker],
) -> Vec<P2pRoute> {
    let Some(conversion) = conversion_quote(tickers, intermediary, &query.target_currency) else {
        return Vec::new();
    };
    if !conversion.rate.is_finite() || conversion.rate <= 0.0 {
        return Vec::new();
    }
    offers
        .iter()
        .filter(|offer| {
            query.accepts_offer(offer)
                && offer.source.eq_ignore_ascii_case(venue)
                && offer.asset.eq_ignore_ascii_case(intermediary)
                && offer.fiat.eq_ignore_ascii_case(&query.source_currency)
                && offer.side == P2pSide::BuyCrypto
                && covers_target(offer, query.source_amount)
        })
        .filter_map(|offer| {
            let acquired = query.source_amount / positive_number(&offer.price)?;
            if positive_number(&offer.available_asset)? < acquired {
                return None;
            }
            // Use the existing Spot estimate convention; final venue fees remain unverified.
            let output = acquired * conversion.rate * (1.0 - SPOT_FEE_RATE);
            if !output.is_finite() || output <= 0.0 {
                return None;
            }
            let payment_methods_verified = query.source_payment_method.as_deref().is_none_or(|method| {
                offer.payment_method_match(method) == PaymentMethodMatch::Exact
            });
            let target = query.target_network.as_deref().map_or_else(
                || query.target_currency.clone(),
                |network| format!("{}@{network}", query.target_currency),
            );
            let mut warnings = vec![
                "Buy the intermediate asset using the selected P2P listing, then trade it on the same venue's Spot market.".into(),
                "Spot estimate includes a 0.1% estimated trading fee; final fees, fills and withdrawal network availability must be checked on the venue.".into(),
            ];
            if !payment_methods_verified {
                warnings.push("The selected payment method was not confirmed by the P2P listing; check it before placing an order.".into());
            }
            Some(P2pRoute {
                route_id: String::new(),
                rank: 0,
                asset: intermediary.into(),
                entry_network: offer.network.clone(),
                source_network: None,
                target_network: query.target_network.clone(),
                source_fiat: query.source_currency.clone(),
                source_amount: fixed(query.source_amount, 8),
                acquired_asset_amount: fixed(acquired, 12),
                provider_input_amount: Some(fixed(acquired, 12)),
                target_fiat: query.target_currency.clone(),
                target_amount: fixed(output, 12),
                effective_rate: fixed(output / query.source_amount, 12),
                same_venue: true,
                requires_asset_transfer: false,
                transfer_fee_included: false,
                route_kind: "fiat_to_crypto".into(),
                profitability: None,
                cycle_legs: Vec::new(),
                profitability_decimals: None,
                bridge_currency: None,
                market_path: Some(CryptoMarketPath {
                    venue: venue.into(),
                    source_pair: conversion.pair.clone(),
                    target_pair: conversion.pair.clone(),
                    source_rate: fixed(conversion.rate, 12),
                    target_rate: "1".into(),
                    intermediary_amount: fixed(output, 12),
                }),
                route_provider: None,
                route_provider_url: None,
                provider_quote_id: None,
                route_path: vec![query.source_currency.clone(), intermediary.into(), target],
                route_fees: Vec::new(),
                quote_expires_at: None,
                execution: None,
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
        .collect()
}

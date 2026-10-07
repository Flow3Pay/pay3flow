use std::cmp::Ordering;

mod diversity;
mod top_k;

pub(in crate::p2p) use top_k::compose_fiat_routes;

use crate::p2p::routes::{CryptoMarketPath, NormalizedRouteQuery, P2pRoute, RouteAssetStatus};
use crate::p2p::service::{P2pOffer, PaymentMethodMatch, SourceStatus};
use crate::p2p::spot::CryptoTicker;
use crate::p2p::P2pOfferMarket;
use crate::route_engine::canonical_network_id;

pub(in crate::p2p) fn matching_offers(
    offers: &[P2pOffer],
    query: &NormalizedRouteQuery,
) -> Vec<P2pOffer> {
    offers
        .iter()
        .filter(|offer| query.accepts_offer(offer))
        .cloned()
        .collect()
}

pub(in crate::p2p) fn reject_price_outliers(
    offers: Vec<P2pOffer>,
    max_deviation_bps: u32,
) -> Vec<P2pOffer> {
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

pub(in crate::p2p) fn compose_fiat_route(
    query: &NormalizedRouteQuery,
    asset: &str,
    entry: &P2pOffer,
    exit: &P2pOffer,
    acquired_asset: f64,
    target_amount: f64,
) -> Option<P2pRoute> {
    if !offer_networks_compatible(entry, exit) {
        return None;
    }
    let same_venue = entry.source == exit.source;
    if !same_venue && !query.allow_cross_venue {
        return None;
    }
    if positive_number(&exit.available_asset).is_none_or(|available| available < acquired_asset)
        || !covers_target(exit, target_amount)
    {
        return None;
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
    Some(P2pRoute {
        route_id: String::new(),
        rank: 0,
        asset: asset.into(),
        entry_network: entry.network.clone().or_else(|| exit.network.clone()),
        source_network: query.source_network.clone(),
        target_network: query.target_network.clone(),
        source_fiat: query.source_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        acquired_asset_amount: fixed(acquired_asset, 8),
        provider_input_amount: None,
        target_fiat: query.target_currency.clone(),
        target_amount: fixed(target_amount, 2),
        effective_rate: fixed(target_amount / query.source_amount, 8),
        same_venue,
        requires_asset_transfer: !same_venue,
        transfer_fee_included: same_venue,
        route_kind: if query.source_currency == query.target_currency {
            "crypto_cycle".into()
        } else {
            "fiat_to_fiat".into()
        },
        profitability: None,
        cycle_legs: Vec::new(),
        profitability_decimals: None,
        bridge_currency: None,
        market_path: None,
        route_provider: None,
        route_provider_url: None,
        provider_quote_id: None,
        route_path: Vec::new(),
        route_fees: Vec::new(),
        quote_expires_at: None,
        execution: None,
        payment_methods_verified,
        entry_offer: Some(entry.clone()),
        exit_offer: Some(exit.clone()),
        warnings,
        services: Vec::new(),
        reputation: None,
        feedback: None,
        service_links: Vec::new(),
    })
}

pub(in crate::p2p) fn compose_fiat_to_crypto_routes(
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
            provider_input_amount: None,
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 8),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_crypto".into(),
            profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            execution: None,
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

pub(in crate::p2p) fn compose_crypto_to_fiat_routes(
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
            provider_input_amount: None,
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 2),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat".into(),
            profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            execution: None,
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

pub(in crate::p2p) fn offer_matches_network(offer: &P2pOffer, requested: Option<&str>) -> bool {
    offer
        .network
        .as_deref()
        .zip(requested)
        .is_none_or(|(actual, requested)| {
            canonical_network_id(actual) == canonical_network_id(requested)
        })
}

pub(in crate::p2p) fn offer_networks_compatible(entry: &P2pOffer, exit: &P2pOffer) -> bool {
    entry
        .network
        .as_deref()
        .zip(exit.network.as_deref())
        .is_none_or(|(entry, exit)| canonical_network_id(entry) == canonical_network_id(exit))
}

pub(in crate::p2p) fn compose_crypto_market_routes(
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
            provider_input_amount: None,
            target_fiat: target_asset.into(),
            target_amount: fixed(target_amount, 12),
            effective_rate: fixed(target_amount / query.source_amount, 12),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto".into(),
            profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
            bridge_currency,
            market_path: Some(path),
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            execution: None,
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

pub(in crate::p2p) struct ConversionQuote {
    pair: String,
    rate: f64,
}

pub(in crate::p2p) fn conversion_quote(
    tickers: &[CryptoTicker],
    from: &str,
    to: &str,
) -> Option<ConversionQuote> {
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

pub(in crate::p2p) fn upsert_asset_status(
    statuses: &mut Vec<RouteAssetStatus>,
    asset: String,
    entry_sources: &[SourceStatus],
    exit_sources: &[SourceStatus],
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
    routes_built: usize,
    routes_exhaustive: bool,
    entry_discovery_source: Option<&str>,
    exit_discovery_source: Option<&str>,
) {
    let status = RouteAssetStatus {
        asset,
        entry_offers: entry_offers.len(),
        exit_offers: exit_offers.len(),
        routes_built,
        routes_exhaustive,
        can_exchange_to_target: routes_built > 0,
        entry_sources: entry_sources.to_vec(),
        exit_sources: exit_sources.to_vec(),
        entry_discovery_source: entry_discovery_source.map(str::to_owned),
        exit_discovery_source: exit_discovery_source.map(str::to_owned),
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

pub(in crate::p2p) fn positive_number(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(in crate::p2p) fn covers_target(offer: &P2pOffer, target_amount: f64) -> bool {
    let Some(minimum) = positive_number(&offer.min_fiat) else {
        return false;
    };
    let Some(maximum) = positive_number(&offer.max_fiat) else {
        return false;
    };
    minimum <= target_amount && target_amount <= maximum
}

pub(in crate::p2p) fn route_target(route: &P2pRoute) -> f64 {
    route.target_amount.parse().unwrap_or_default()
}

pub(in crate::p2p) fn fixed(value: f64, scale: usize) -> String {
    format!("{value:.scale$}")
}

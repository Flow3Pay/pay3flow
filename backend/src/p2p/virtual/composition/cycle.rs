use crate::p2p::routes::*;
use crate::p2p::{CryptoCycleLeg, P2pRoute};
use crate::route_engine::PublicRouteQuote;

fn cycle_leg(quote: &PublicRouteQuote) -> CryptoCycleLeg {
    CryptoCycleLeg {
        market_pair: None,
        provider: quote.provider.clone(),
        description: quote.description.clone(),
        from_asset: quote.from.to_string(),
        to_asset: quote.to.to_string(),
        input_amount: quote.input.value.clone(),
        output_amount: quote.output.value.clone(),
        source_url: quote.source_url.clone(),
        quote_id: quote.quote_id.clone(),
        expires_at: quote.expires_at,
    }
}

pub(in crate::p2p) fn crypto_cycle_route(
    query: &NormalizedRouteQuery,
    entry: &PublicRouteQuote,
    exit: PublicRouteQuote,
) -> Option<P2pRoute> {
    crypto_cycle_from_quotes(query, &[entry.clone(), exit])
}

pub(in crate::p2p) fn crypto_cycle_from_quotes(
    query: &NormalizedRouteQuery,
    quotes: &[PublicRouteQuote],
) -> Option<P2pRoute> {
    let entry = quotes.first()?;
    let exit = quotes.last()?;
    if quotes.len() < 2
        || entry.from != exit.to
        || quotes.windows(2).any(|legs| {
            legs[0].to != legs[1].from
                || positive_number(&legs[0].output.value) != positive_number(&legs[1].input.value)
        })
    {
        return None;
    }
    // Compare at the display's crypto precision; a sub-unit gain is not a route.
    let input = crypto_profit_units(&entry.input.value)?;
    let output = crypto_profit_units(&exit.output.value)?;
    if output <= input {
        return None;
    }
    let input_value = positive_number(&entry.input.value)?;
    let output_value = positive_number(&exit.output.value)?;
    let expires_at = quotes.iter().filter_map(|quote| quote.expires_at).min();
    if expires_at.is_some_and(|expiry| expiry <= chrono::Utc::now()) {
        return None;
    }
    let same_venue = quotes.iter().all(|quote| {
        quote.provider == entry.provider
            && (entry.provider != "bestchange"
                || quote.description == entry.description && quote.source_url == entry.source_url)
    });
    if !query.allow_cross_venue && !same_venue {
        return None;
    }
    let mut path = if entry.path.is_empty() {
        vec![entry.from.to_string(), entry.to.to_string()]
    } else {
        entry.path.iter().map(ToString::to_string).collect()
    };
    for quote in quotes.iter().skip(1) {
        if quote.path.is_empty() {
            path.push(quote.to.to_string());
        } else {
            path.extend(quote.path.iter().skip(1).map(ToString::to_string));
        }
    }
    Some(P2pRoute {
        route_id: String::new(),
        rank: 0,
        asset: entry.to.symbol.clone(),
        entry_network: query.source_network.clone(),
        source_network: query.source_network.clone(),
        target_network: query.target_network.clone(),
        source_fiat: query.source_currency.clone(),
        source_amount: entry.input.value.clone(),
        acquired_asset_amount: entry.output.value.clone(),
        provider_input_amount: None,
        target_fiat: query.target_currency.clone(),
        target_amount: exit.output.value.clone(),
        effective_rate: fixed(output_value / input_value, 12),
        same_venue,
        requires_asset_transfer: true,
        // Public swap quotes do not establish the cost of sending from the wallet
        // on both legs (gas, approvals, deposits). Never confirm net arbitrage here.
        transfer_fee_included: false,
        route_kind: "crypto_cycle".into(),
        profitability: None,
        profitability_decimals: Some(8),
        cycle_legs: quotes.iter().map(cycle_leg).collect(),
        bridge_currency: Some(entry.to.symbol.clone()),
        market_path: None,
        route_provider: same_venue.then(|| entry.provider.clone()),
        route_provider_url: None,
        provider_quote_id: None,
        route_path: path,
        route_fees: quotes.iter().flat_map(|quote| &quote.fees).map(|fee| RouteFee {
            asset: fee.asset.to_string(),
            amount: fee.value.clone(),
        }).collect(),
        quote_expires_at: expires_at,
        execution: None,
        payment_methods_verified: true,
        entry_offer: None,
        exit_offer: None,
        warnings: vec![
            "Positive quoted cycle output; wallet gas, approvals and deposit costs are not fully verified. Net profit is unconfirmed.".into(),
            "The return quote uses the first swap's output. Both rates can change before the sequential swaps finish; re-quote every step before sending.".into(),
        ],
        services: Vec::new(),
        reputation: None,
        feedback: None,
        service_links: Vec::new(),
    })
}

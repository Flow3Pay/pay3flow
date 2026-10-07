//! Spot cycles are estimates from executable sides of public bid/ask tickers.
use crate::p2p::routes::{
    crypto_cycle_from_quotes, fixed, source_selected, NormalizedRouteQuery, P2pRoute,
};
use crate::p2p::spot::CryptoTicker;
use crate::route_engine::{Amount, Asset, PublicRouteQuote};
use std::collections::HashMap;

const ESTIMATED_TRADING_FEE: f64 = 0.001;
const MAX_MARKET_INTERMEDIARIES: usize = 96;
const QUOTE_ASSETS: &[&str] = &[
    "FDUSD", "USDT", "USDC", "TUSD", "DAI", "BTC", "ETH", "BNB", "EUR", "USD",
];

struct Edge {
    from: String,
    to: String,
    venue: String,
    pair: String,
    rate: f64,
}

pub(in crate::p2p) fn compose_crypto_market_cycles(
    query: &NormalizedRouteQuery,
    markets: &HashMap<String, Vec<CryptoTicker>>,
) -> Vec<P2pRoute> {
    let Ok(origin) = Asset::new(&query.source_currency, query.source_network.as_deref()) else {
        return Vec::new();
    };
    let mut graph = HashMap::<String, Vec<Edge>>::new();
    for (venue, tickers) in markets
        .iter()
        .filter(|(venue, _)| source_selected(query, venue))
    {
        for ticker in tickers {
            if !ticker.bid.is_finite()
                || !ticker.ask.is_finite()
                || ticker.bid <= 0.0
                || ticker.ask <= 0.0
                || ticker.bid > ticker.ask
            {
                continue;
            }
            let symbol = ticker.symbol.to_ascii_uppercase();
            let Some((base, quote)) = QUOTE_ASSETS.iter().find_map(|quote| {
                symbol
                    .strip_suffix(quote)
                    .filter(|base| !base.is_empty())
                    .map(|base| (base.to_owned(), (*quote).to_owned()))
            }) else {
                continue;
            };
            if Asset::new(&base, None).is_err() || base == quote {
                continue;
            }
            for (from, to, rate) in [
                (base.clone(), quote.clone(), ticker.bid),
                (quote, base, 1.0 / ticker.ask),
            ] {
                if !rate.is_finite() || rate <= 0.0 {
                    continue;
                }
                graph.entry(from.clone()).or_default().push(Edge {
                    from,
                    to,
                    venue: venue.clone(),
                    pair: ticker.symbol.clone(),
                    rate,
                });
            }
        }
    }
    let Some(entries) = graph.get(&origin.symbol) else {
        return Vec::new();
    };
    let allowed = |symbol: &str| {
        symbol != origin.symbol
            && !matches!(symbol, "USD" | "EUR")
            && (!query.assets_explicit || query.assets.iter().any(|allowed| allowed == symbol))
    };
    let mut intermediaries = entries
        .iter()
        .filter(|edge| allowed(&edge.to))
        .map(|edge| edge.to.clone())
        .collect::<Vec<_>>();
    intermediaries.sort_by_key(|symbol| {
        (
            QUOTE_ASSETS
                .iter()
                .position(|asset| *asset == symbol)
                .unwrap_or(QUOTE_ASSETS.len()),
            symbol.clone(),
        )
    });
    intermediaries.dedup();
    intermediaries.truncate(MAX_MARKET_INTERMEDIARIES);
    let mut routes = Vec::new();
    for first in entries
        .iter()
        .filter(|edge| intermediaries.contains(&edge.to))
    {
        let Some(seconds) = graph.get(&first.to) else {
            continue;
        };
        for second in seconds {
            if second.to == origin.symbol {
                if query.allow_cross_venue || first.venue == second.venue {
                    if let Some(route) = market_cycle(query, &origin, &[first, second]) {
                        routes.push(route);
                    }
                }
            } else if second.venue == first.venue && allowed(&second.to) {
                // Three trades within one venue avoid intermediate withdrawals.
                if let Some(thirds) = graph.get(&second.to) {
                    for third in thirds
                        .iter()
                        .filter(|edge| edge.venue == first.venue && edge.to == origin.symbol)
                    {
                        if let Some(route) = market_cycle(query, &origin, &[first, second, third]) {
                            routes.push(route);
                        }
                    }
                }
            }
        }
    }
    routes
}

fn market_cycle(query: &NormalizedRouteQuery, origin: &Asset, edges: &[&Edge]) -> Option<P2pRoute> {
    let mut amount = Amount::from_f64(query.source_amount, origin.clone()).ok()?;
    let mut quotes = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        let from = amount.asset.clone();
        if from.symbol != edge.from {
            return None;
        }
        let to = if index + 1 == edges.len() {
            origin.clone()
        } else {
            Asset::new(&edge.to, None).ok()?
        };
        let input_value = amount.value.parse::<f64>().ok()?;
        let output = Amount::new(
            fixed(input_value * edge.rate * (1.0 - ESTIMATED_TRADING_FEE), 12),
            to.clone(),
        )
        .ok()?;
        quotes.push(PublicRouteQuote {
            provider: edge.venue.clone(),
            quote_id: None,
            description: Some(format!("Spot {}", edge.pair)),
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output: output.clone(),
            fees: vec![
                Amount::new(fixed(input_value * ESTIMATED_TRADING_FEE, 12), from.clone()).ok()?,
            ],
            expires_at: None,
            path: vec![from, to],
        });
        amount = output;
    }
    let mut route = crypto_cycle_from_quotes(query, &quotes)?;
    let output = route.target_amount.parse::<f64>().ok()?;
    let gain_bps = (output / query.source_amount - 1.0) * 10_000.0;
    if query.max_price_deviation_bps > 0 && gain_bps > f64::from(query.max_price_deviation_bps) {
        return None;
    }
    for (leg, edge) in route.cycle_legs.iter_mut().zip(edges) {
        leg.market_pair = Some(edge.pair.clone());
    }
    route.warnings = vec![
        "Spot-market cycle estimate using bid/ask prices and an estimated 0.1% trading fee per trade. Actual trading fees, order-book depth and slippage are not verified; refresh and verify every trade before execution.".into(),
        "Deposit/withdrawal availability on the selected network, wallet gas, transfer fees and minimum amounts are not verified. Cross-venue cycles require transferring the intermediate asset; net profit is unconfirmed.".into(),
    ];
    Some(route)
}

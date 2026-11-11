use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::SourceStatus;

#[derive(Debug, Clone)]
pub(crate) struct FuturesTicker {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
}

#[async_trait]
pub(crate) trait PerpetualMarketSource: Send + Sync {
    fn name(&self) -> &str;

    fn timeout(&self, default: Duration) -> Duration {
        default
    }

    async fn perpetual_tickers(&self) -> Result<Vec<FuturesTicker>>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuturesSearchQuery {
    pub asset: String,
    pub notional_usdt: f64,
    pub sources: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuturesOpportunity {
    pub asset: String,
    pub contract: String,
    pub long_venue: String,
    pub long_ask: f64,
    pub long_url: Option<String>,
    pub short_venue: String,
    pub short_bid: f64,
    pub short_url: Option<String>,
    pub spread_bps: f64,
    pub estimated_gross_profit_usdt: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FuturesSearchResponse {
    pub asset: String,
    pub notional_usdt: f64,
    pub searched_at: chrono::DateTime<chrono::Utc>,
    pub opportunities: Vec<FuturesOpportunity>,
    pub sources: Vec<SourceStatus>,
    pub warnings: Vec<String>,
}

pub(crate) fn compose_opportunities(
    asset: &str,
    notional_usdt: f64,
    markets: &[(String, Vec<FuturesTicker>)],
) -> Vec<FuturesOpportunity> {
    let contract = format!("{asset}USDT");
    let mut quotes = Vec::new();
    for (venue, tickers) in markets {
        for ticker in tickers {
            if ticker.symbol.eq_ignore_ascii_case(&contract)
                && ticker.bid.is_finite()
                && ticker.ask.is_finite()
                && ticker.bid > 0.0
                && ticker.ask >= ticker.bid
            {
                quotes.push((venue, ticker.bid, ticker.ask));
            }
        }
    }
    quotes.sort_by(|left, right| left.0.cmp(right.0));

    let mut opportunities = Vec::new();
    for (long_venue, _, long_ask) in &quotes {
        for (short_venue, short_bid, _) in &quotes {
            if long_venue == short_venue || short_bid <= long_ask {
                continue;
            }
            let spread_bps = (short_bid / long_ask - 1.0) * 10_000.0;
            let estimated_gross_profit_usdt = notional_usdt * spread_bps / 10_000.0;
            if !spread_bps.is_finite()
                || !estimated_gross_profit_usdt.is_finite()
                || spread_bps <= 0.0
            {
                continue;
            }
            opportunities.push(FuturesOpportunity {
                asset: asset.into(),
                contract: contract.clone(),
                long_venue: (*long_venue).clone(),
                long_ask: *long_ask,
                long_url: futures_trade_url(long_venue, &contract),
                short_venue: (*short_venue).clone(),
                short_bid: *short_bid,
                short_url: futures_trade_url(short_venue, &contract),
                spread_bps,
                estimated_gross_profit_usdt,
            });
        }
    }
    opportunities.sort_by(|left, right| {
        right
            .spread_bps
            .total_cmp(&left.spread_bps)
            .then_with(|| left.long_venue.cmp(&right.long_venue))
            .then_with(|| left.short_venue.cmp(&right.short_venue))
    });
    opportunities
}

fn futures_trade_url(venue: &str, contract: &str) -> Option<String> {
    let base = contract.strip_suffix("USDT")?;
    match venue {
        "binance" => Some(format!("https://www.binance.com/en/futures/{contract}")),
        "bybit" => Some(format!("https://www.bybit.com/trade/usdt/{contract}")),
        "okx" => Some(format!(
            "https://www.okx.com/trade-swap/{}-usdt-swap",
            base.to_ascii_lowercase()
        )),
        "bitget" => Some(format!("https://www.bitget.com/futures/usdt/{contract}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticker(symbol: &str, bid: f64, ask: f64) -> FuturesTicker {
        FuturesTicker {
            symbol: symbol.into(),
            bid,
            ask,
        }
    }

    #[test]
    fn finds_only_cross_venue_positive_linear_perpetual_spreads() {
        let results = compose_opportunities(
            "BTC",
            1_000.0,
            &[
                ("binance".into(), vec![ticker("BTCUSDT", 99.0, 100.0)]),
                ("bybit".into(), vec![ticker("BTCUSDT", 102.0, 103.0)]),
                ("okx".into(), vec![ticker("ETHUSDT", 200.0, 201.0)]),
            ],
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].long_venue, "binance");
        assert_eq!(results[0].short_venue, "bybit");
        assert!((results[0].spread_bps - 200.0).abs() < 1e-6);
        assert!((results[0].estimated_gross_profit_usdt - 20.0).abs() < 1e-6);
        assert!(results[0].long_url.is_some());
        assert!(results[0].short_url.is_some());
    }

    #[test]
    fn rejects_same_venue_and_non_positive_spreads() {
        let results = compose_opportunities(
            "ETH",
            1_000.0,
            &[
                ("binance".into(), vec![ticker("ETHUSDT", 100.0, 101.0)]),
                ("bybit".into(), vec![ticker("ETHUSDT", 100.5, 102.0)]),
            ],
        );
        assert!(results.is_empty());
    }
}

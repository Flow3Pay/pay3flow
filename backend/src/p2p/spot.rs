use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub(crate) struct CryptoTicker {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
}

pub(crate) const SPOT_QUOTE_ASSETS: &[&str] = &[
    "USDT", "USDC", "FDUSD", "BTC", "ETH", "BNB", "DAI", "TUSD", "USDE", "USD1", "RLUSD", "GUSD",
    "USDD", "USDS", "EUR", "USD", "TRY", "BRL", "IDR", "RUB",
];

pub(crate) fn split_market_symbol(symbol: &str) -> Option<(String, String)> {
    let symbol = symbol.to_ascii_uppercase();
    SPOT_QUOTE_ASSETS
        .iter()
        .filter_map(|quote| {
            symbol
                .strip_suffix(quote)
                .filter(|base| !base.is_empty())
                .map(|base| (base.to_owned(), (*quote).to_owned()))
        })
        .max_by_key(|(_, quote)| quote.len())
}

impl CryptoTicker {
    pub(crate) fn is_valid(&self) -> bool {
        self.bid.is_finite()
            && self.ask.is_finite()
            && self.bid > 0.0
            && self.ask > 0.0
            && self.bid <= self.ask
    }
}

#[async_trait]
pub(crate) trait CryptoMarketSource: Send + Sync {
    fn name(&self) -> &str;

    fn timeout(&self, default: Duration) -> Duration {
        default
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>>;
}

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct CryptoTicker {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
}

#[async_trait]
pub(crate) trait CryptoMarketSource: Send + Sync {
    fn name(&self) -> &str;

    fn timeout(&self, default: Duration) -> Duration {
        default
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>>;
}

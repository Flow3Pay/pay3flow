use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub(crate) struct CryptoTicker {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
}

#[async_trait]
pub(crate) trait CryptoMarketSource: Send + Sync {
    fn name(&self) -> &'static str;
    async fn tickers(&self) -> Result<Vec<CryptoTicker>>;
}

pub(crate) struct BinanceSpotSource {
    client: Client,
}

impl BinanceSpotSource {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CryptoMarketSource for BinanceSpotSource {
    fn name(&self) -> &'static str {
        "binance"
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        let response = self
            .client
            .get("https://api.binance.com/api/v3/ticker/bookTicker")
            .send()
            .await
            .context("Binance spot ticker request failed")?
            .error_for_status()
            .context("Binance spot ticker returned an HTTP error")?
            .json::<Vec<BinanceTicker>>()
            .await
            .context("invalid Binance spot ticker response")?;
        Ok(response
            .into_iter()
            .filter_map(BinanceTicker::into_ticker)
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceTicker {
    symbol: String,
    bid_price: String,
    ask_price: String,
}

impl BinanceTicker {
    fn into_ticker(self) -> Option<CryptoTicker> {
        ticker(self.symbol, &self.bid_price, &self.ask_price)
    }
}

pub(crate) struct BybitSpotSource {
    client: Client,
}

impl BybitSpotSource {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CryptoMarketSource for BybitSpotSource {
    fn name(&self) -> &'static str {
        "bybit"
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        let response = self
            .client
            .get("https://api.bybit.com/v5/market/tickers")
            .query(&[("category", "spot")])
            .send()
            .await
            .context("Bybit spot ticker request failed")?
            .error_for_status()
            .context("Bybit spot ticker returned an HTTP error")?
            .json::<BybitResponse>()
            .await
            .context("invalid Bybit spot ticker response")?;
        if response.ret_code != 0 {
            anyhow::bail!(
                "Bybit spot ticker error {}: {}",
                response.ret_code,
                response.ret_msg
            );
        }
        Ok(response
            .result
            .list
            .into_iter()
            .filter_map(|item| ticker(item.symbol, &item.bid_price, &item.ask_price))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitResponse {
    ret_code: i64,
    ret_msg: String,
    result: BybitResult,
}

#[derive(Debug, Deserialize)]
struct BybitResult {
    #[serde(default)]
    list: Vec<BybitTicker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BybitTicker {
    symbol: String,
    #[serde(rename = "bid1Price")]
    bid_price: String,
    #[serde(rename = "ask1Price")]
    ask_price: String,
}

pub(crate) struct OkxSpotSource {
    client: Client,
}

impl OkxSpotSource {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CryptoMarketSource for OkxSpotSource {
    fn name(&self) -> &'static str {
        "okx"
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        let response = self
            .client
            .get("https://www.okx.com/api/v5/market/tickers")
            .query(&[("instType", "SPOT")])
            .send()
            .await
            .context("OKX spot ticker request failed")?
            .error_for_status()
            .context("OKX spot ticker returned an HTTP error")?
            .json::<OkxResponse>()
            .await
            .context("invalid OKX spot ticker response")?;
        if response.code != "0" {
            anyhow::bail!("OKX spot ticker error {}", response.code);
        }
        Ok(response
            .data
            .into_iter()
            .filter_map(|item| ticker(item.inst_id.replace('-', ""), &item.bid_px, &item.ask_px))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct OkxResponse {
    code: String,
    #[serde(default)]
    data: Vec<OkxTicker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkxTicker {
    inst_id: String,
    bid_px: String,
    ask_px: String,
}

pub(crate) struct BitgetSpotSource {
    client: Client,
}

impl BitgetSpotSource {
    pub(crate) fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl CryptoMarketSource for BitgetSpotSource {
    fn name(&self) -> &'static str {
        "bitget"
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        let response = self
            .client
            .get("https://api.bitget.com/api/v2/spot/market/tickers")
            .send()
            .await
            .context("Bitget spot ticker request failed")?
            .error_for_status()
            .context("Bitget spot ticker returned an HTTP error")?
            .json::<BitgetResponse>()
            .await
            .context("invalid Bitget spot ticker response")?;
        if response.code != "00000" {
            anyhow::bail!(
                "Bitget spot ticker error {}: {}",
                response.code,
                response.msg
            );
        }
        Ok(response
            .data
            .into_iter()
            .filter_map(|item| ticker(item.symbol, &item.bid_pr, &item.ask_pr))
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct BitgetResponse {
    code: String,
    msg: String,
    #[serde(default)]
    data: Vec<BitgetTicker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BitgetTicker {
    symbol: String,
    bid_pr: String,
    ask_pr: String,
}

fn ticker(symbol: String, bid: &str, ask: &str) -> Option<CryptoTicker> {
    let bid = bid
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let ask = ask
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)?;
    Some(CryptoTicker { symbol, bid, ask })
}

//! Indicative USD values for the crypto assets in the public picker.
//!
//! One cached Gate.io tickers request serves every visitor and every asset.
//! Symbols are mapped to explicit pairs because looking a token up by ticker
//! alone can select a different coin. DefiLlama is only kept as a chart
//! fallback for the rare case where Gate has no candles for an asset.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::MissedTickBehavior;

use crate::core::error::AppError;
use crate::core::state::AppState;

const REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const MAX_STALE_AGE: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_PROVIDER_PRICE_AGE_SECS: i64 = 15 * 60;
const DEFILLAMA_ROOT: &str = "https://coins.llama.fi/";
const GATE_ROOT: &str = "https://api.gateio.ws/api/v4/";
/// The legacy public tickers feed returns every listed pair, keyed by a
/// lowercase `asset_quote` name, with the last trade in the `last` field.
const GATE_TICKERS: &str = "https://data.gateapi.io/api2/1/tickers";
const HISTORY_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const EMPTY_TTL: Duration = Duration::from_secs(60);
/// The OTC chart polls once a minute, so a short candle cache keeps the
/// provider load bounded without making the newest candle visibly late.
const CANDLE_TTL: Duration = Duration::from_secs(60);

// Gate pairs are listed lowercase here because the tickers feed lowercases its
// keys; the candle endpoint gets them uppercased. The first pair with a
// positive last trade wins, which also covers delisted quotes.
//
// TON maps to GRAM because the native coin was renamed and Gate's own TON_USDT
// is a different token. WETH has no pair of its own and trades 1:1 with ETH.
// MATIC's delisted USDT quote is replaced by POL, the token it migrated to.
const GATE_PAIRS: &[(&str, &[&str])] = &[
    ("AAVE", &["aave_usdt"]),
    ("ADA", &["ada_usdt"]),
    ("APT", &["apt_usdt"]),
    ("ATOM", &["atom_usdt"]),
    ("AVAX", &["avax_usdt"]),
    ("BCH", &["bch_usdt"]),
    ("BNB", &["bnb_usdt"]),
    ("BTC", &["btc_usdt"]),
    ("DAI", &["dai_usdt"]),
    ("DOGE", &["doge_usdt"]),
    ("DOT", &["dot_usdt"]),
    ("ETH", &["eth_usdt"]),
    ("EVER", &["ever_usdt"]),
    ("10SET", &["10set_usdt"]),
    ("FDUSD", &["fdusd_usdt"]),
    ("LINK", &["link_usdt"]),
    ("LTC", &["ltc_usdt"]),
    ("MATIC", &["pol_usdt"]),
    ("NEAR", &["near_usdt"]),
    ("POL", &["pol_usdt"]),
    ("SOL", &["sol_usdt"]),
    ("SUI", &["sui_usdt"]),
    ("GRAM", &["gram_usdt"]),
    ("TON", &["gram_usdt"]),
    ("TRX", &["trx_usdt"]),
    ("UNI", &["uni_usdt"]),
    ("USDC", &["usdc_usdt"]),
    ("USDT", &["usdt_usd"]),
    ("WBTC", &["wbtc_usdt"]),
    ("WETH", &["eth_usdt"]),
    ("XRP", &["xrp_usdt"]),
    ("ZEC", &["zec_usdt"]),
];

/// CoinGecko identifiers are no longer used for pricing, but they still name
/// the daily series that DefiLlama exposes as a chart fallback.
const ASSETS: &[(&str, &str)] = &[
    ("AAVE", "coingecko:aave"),
    ("ADA", "coingecko:cardano"),
    ("APT", "coingecko:aptos"),
    ("ATOM", "coingecko:cosmos"),
    ("AVAX", "coingecko:avalanche-2"),
    ("BCH", "coingecko:bitcoin-cash"),
    ("BNB", "coingecko:binancecoin"),
    ("BTC", "coingecko:bitcoin"),
    ("DAI", "coingecko:dai"),
    ("DOGE", "coingecko:dogecoin"),
    ("DOT", "coingecko:polkadot"),
    ("ETH", "coingecko:ethereum"),
    ("EVER", "coingecko:everscale"),
    ("10SET", "coingecko:tenset"),
    ("FDUSD", "coingecko:first-digital-usd"),
    ("LINK", "coingecko:chainlink"),
    ("LTC", "coingecko:litecoin"),
    (
        "MATIC",
        "ethereum:0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0",
    ),
    ("NEAR", "coingecko:near"),
    ("POL", "coingecko:polygon-ecosystem-token"),
    ("SOL", "coingecko:solana"),
    ("SUI", "coingecko:sui"),
    ("GRAM", "coingecko:the-open-network"),
    ("TON", "coingecko:the-open-network"),
    ("TRX", "coingecko:tron"),
    ("UNI", "coingecko:uniswap"),
    ("USDC", "coingecko:usd-coin"),
    ("USDT", "coingecko:tether"),
    ("WBTC", "coingecko:wrapped-bitcoin"),
    ("WETH", "coingecko:weth"),
    ("XRP", "coingecko:ripple"),
    ("ZEC", "coingecko:zcash"),
];

#[derive(Clone)]
pub struct MarketPriceService {
    client: reqwest::Client,
    tickers_url: String,
    chart_root: String,
    exchange_root: String,
    cache: Arc<RwLock<Option<PriceSnapshot>>>,
    history: Arc<RwLock<HashMap<String, HistorySnapshot>>>,
    candles: Arc<RwLock<HashMap<String, CandleSnapshot>>>,
}

#[derive(Clone)]
struct PriceSnapshot {
    fetched_at: Instant,
    updated_at: i64,
    prices: HashMap<&'static str, MarketPrice>,
}

#[derive(Clone)]
struct HistorySnapshot {
    fetched_at: Instant,
    points: Vec<SpotPoint>,
}

#[derive(Clone)]
struct CandleSnapshot {
    fetched_at: Instant,
    candles: Vec<ExchangeCandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SpotPoint {
    pub t: i64,
    pub usd: f64,
}

/// A real exchange OHLC candle. `volume` is quoted in the pair's quote asset
/// (USDT for `EVER_USDT`), matching the desk's own volume unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ExchangeCandle {
    pub t: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone)]
struct MarketPrice {
    usd: f64,
    timestamp: i64,
}

#[derive(Deserialize)]
struct ChartResponse {
    coins: HashMap<String, ChartSeries>,
}

#[derive(Deserialize)]
struct ChartSeries {
    prices: Vec<ChartPoint>,
}

#[derive(Deserialize)]
struct ChartPoint {
    timestamp: i64,
    price: f64,
}

/// Exchange rows mix quoted numbers and plain JSON scalars, so every field is
/// read through one tolerant conversion.
fn as_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::String(text) => text.parse::<f64>().ok(),
        serde_json::Value::Number(number) => number.as_f64(),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketValuesRequest {
    items: Vec<MarketValueInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketValueInput {
    asset: String,
    amount: String,
}

#[derive(Serialize)]
pub struct MarketValuesResponse {
    source: &'static str,
    stale: bool,
    values: Vec<MarketValue>,
}

#[derive(Serialize)]
pub struct MarketPricesResponse {
    source: &'static str,
    stale: bool,
    updated_at: Option<i64>,
    prices: HashMap<&'static str, f64>,
}

#[derive(Serialize)]
struct MarketValue {
    asset: String,
    amount: String,
    price_usd: Option<f64>,
    value_usd: Option<f64>,
    price_timestamp: Option<i64>,
}

impl MarketPriceService {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()?;
        Ok(Self::with_client(
            client,
            DEFILLAMA_ROOT.to_owned(),
            GATE_ROOT.to_owned(),
        ))
    }

    fn with_client(client: reqwest::Client, root: String, exchange_root: String) -> Self {
        Self {
            client,
            tickers_url: GATE_TICKERS.to_owned(),
            chart_root: root,
            exchange_root,
            cache: Arc::new(RwLock::new(None)),
            history: Arc::new(RwLock::new(HashMap::new())),
            candles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetch at startup, refresh hourly, and retry failures each minute.
    pub async fn initialize(&self) {
        let started_at = tokio::time::Instant::now();
        self.refresh().await;
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval_at(started_at + RETRY_INTERVAL, RETRY_INTERVAL);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let refresh_due = service
                    .cache
                    .read()
                    .await
                    .as_ref()
                    .is_none_or(|snapshot| snapshot.fetched_at.elapsed() >= REFRESH_INTERVAL);
                if refresh_due {
                    service.refresh().await;
                }
            }
        });
    }

    async fn refresh(&self) {
        match self.fetch().await {
            Ok(mut snapshot) => {
                let mut cache = self.cache.write().await;
                if let Some(previous) = cache.as_ref() {
                    for (&asset, price) in &previous.prices {
                        if snapshot.updated_at.saturating_sub(price.timestamp)
                            < MAX_STALE_AGE.as_secs() as i64
                        {
                            snapshot
                                .prices
                                .entry(asset)
                                .or_insert_with(|| price.clone());
                        }
                    }
                }
                *cache = Some(snapshot);
            }
            Err(error) => {
                tracing::warn!(%error, "market price refresh failed");
            }
        }
    }

    async fn prices(&self) -> Option<(PriceSnapshot, bool)> {
        self.cache.read().await.as_ref().and_then(|snapshot| {
            let age = snapshot.fetched_at.elapsed();
            (age < MAX_STALE_AGE).then(|| (snapshot.clone(), age >= REFRESH_INTERVAL))
        })
    }

    /// Only fresh, identified prices may participate in discovery rankings.
    pub(crate) async fn discovery_prices(
        &self,
    ) -> Vec<(String, f64, chrono::DateTime<chrono::Utc>)> {
        let Some((snapshot, stale)) = self.prices().await else {
            return Vec::new();
        };
        if stale {
            return Vec::new();
        }
        snapshot
            .prices
            .into_iter()
            .filter_map(|(asset, price)| {
                if chrono::Utc::now()
                    .timestamp()
                    .saturating_sub(price.timestamp)
                    > MAX_PROVIDER_PRICE_AGE_SECS
                    || !price.usd.is_finite()
                    || price.usd <= 0.0
                {
                    return None;
                }
                Some((
                    asset.to_string(),
                    price.usd,
                    chrono::DateTime::from_timestamp(price.timestamp, 0)?,
                ))
            })
            .collect()
    }

    pub async fn list(&self) -> MarketPricesResponse {
        let snapshot = self.prices().await;
        MarketPricesResponse {
            source: "Gate.io",
            stale: snapshot.as_ref().is_some_and(|(_, stale)| *stale),
            updated_at: snapshot.as_ref().map(|(snapshot, _)| snapshot.updated_at),
            prices: snapshot
                .map(|(snapshot, _)| {
                    snapshot
                        .prices
                        .into_iter()
                        .map(|(asset, price)| (asset, price.usd))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    async fn fetch(&self) -> anyhow::Result<PriceSnapshot> {
        let response = self
            .client
            .get(&self.tickers_url)
            .send()
            .await?
            .error_for_status()?;
        let payload = response
            .json::<HashMap<String, serde_json::Value>>()
            .await?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let prices: HashMap<&'static str, MarketPrice> = GATE_PAIRS
            .iter()
            .filter_map(|(asset, pairs)| {
                let (_, usd) = pairs.iter().find_map(|pair| {
                    let last = as_number(payload.get(*pair)?.get("last")?)?;
                    (last.is_finite() && last > 0.0).then_some((*pair, last))
                })?;
                Some((
                    *asset,
                    MarketPrice {
                        usd,
                        timestamp: now,
                    },
                ))
            })
            .collect();
        if prices.is_empty() {
            anyhow::bail!("Gate tickers returned no usable supported assets");
        }
        Ok(PriceSnapshot {
            fetched_at: Instant::now(),
            updated_at: now,
            prices,
        })
    }

    pub async fn value(&self, items: Vec<(String, String, f64)>) -> MarketValuesResponse {
        let snapshot = self.prices().await;
        let stale = snapshot.as_ref().is_some_and(|(_, stale)| *stale);
        let values = items
            .into_iter()
            .map(|(asset, amount, quantity)| {
                let price = snapshot
                    .as_ref()
                    .and_then(|(snapshot, _)| snapshot.prices.get(asset.as_str()));
                let value_usd = price
                    .filter(|_| quantity > 0.0)
                    .map(|price| price.usd * quantity)
                    .filter(|value| value.is_finite());
                MarketValue {
                    asset,
                    amount,
                    price_usd: price.map(|price| price.usd),
                    value_usd,
                    price_timestamp: price.map(|price| price.timestamp),
                }
            })
            .collect();
        MarketValuesResponse {
            source: "Gate.io",
            stale,
            values,
        }
    }

    /// Daily indicative USD series for one listed asset, cached per length.
    pub async fn daily_history(&self, asset: &str, days: u32) -> (Vec<SpotPoint>, bool) {
        self.spot_history(asset, "1d", days.clamp(2, 400)).await
    }

    /// Indicative USD series for one listed asset. Gate candles are the
    /// primary series because they come from the same provider as the spot
    /// prices; DefiLlama daily points remain a fallback for assets whose Gate
    /// pair is missing or too new to cover the requested window.
    pub async fn spot_history(
        &self,
        asset: &str,
        period: &str,
        span: u32,
    ) -> (Vec<SpotPoint>, bool) {
        let span = span.clamp(2, 500);
        let Some(id) = ASSETS
            .iter()
            .find(|(symbol, _)| *symbol == asset)
            .map(|(_, id)| *id)
        else {
            return (Vec::new(), true);
        };
        let key = format!("{asset}:{period}:{span}");
        if let Some(cached) = self.history.read().await.get(&key) {
            // An empty series retries soon: a provider hiccup should not blank
            // the chart for the whole full-history window.
            let ttl = if cached.points.is_empty() {
                EMPTY_TTL
            } else {
                HISTORY_TTL
            };
            if cached.fetched_at.elapsed() < ttl {
                return (cached.points.clone(), false);
            }
        }
        let exchange: Vec<SpotPoint> = if period == "1h" {
            self.exchange_history(asset, period, span).await
        } else {
            self.exchange_candles(asset, "1d", span)
                .await
                .0
                .into_iter()
                .map(|candle| SpotPoint {
                    t: candle.t,
                    usd: candle.close,
                })
                .collect()
        };
        if !exchange.is_empty() {
            self.history.write().await.insert(
                key,
                HistorySnapshot {
                    fetched_at: Instant::now(),
                    points: exchange.clone(),
                },
            );
            return (exchange, false);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or_default();
        let step = if period == "1h" { 3_600 } else { 86_400 };
        let url = format!(
            "{}chart/{id}?start={}&span={}&period={period}",
            self.chart_root,
            now - i64::from(span) * step,
            span + 2
        );
        let fetched = async {
            Ok::<_, reqwest::Error>(
                self.client
                    .get(&url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<ChartResponse>()
                    .await?,
            )
        };
        let mut points: Vec<SpotPoint> = match fetched.await {
            Ok(payload) => payload
                .coins
                .get(id)
                .map(|series| {
                    series
                        .prices
                        .iter()
                        .filter(|point| point.price.is_finite() && point.price > 0.0)
                        .map(|point| SpotPoint {
                            t: point.timestamp,
                            usd: point.price,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(error) => {
                tracing::warn!(%error, "market history refresh failed");
                let stale = self
                    .history
                    .read()
                    .await
                    .get(&key)
                    .map(|cached| cached.points.clone())
                    .unwrap_or_default();
                return (stale, true);
            }
        };
        if points.is_empty() {
            points = self.exchange_history(asset, period, span).await;
        }
        self.history.write().await.insert(
            key,
            HistorySnapshot {
                fetched_at: Instant::now(),
                points: points.clone(),
            },
        );
        (points, false)
    }

    /// Real exchange OHLC candles for one listed asset. The chart uses these
    /// because they carry an exact open, high, low, close and volume for every
    /// requested interval, unlike the indicative point series.
    pub async fn exchange_candles(
        &self,
        asset: &str,
        interval: &str,
        limit: u32,
    ) -> (Vec<ExchangeCandle>, bool) {
        let Some(pair) = GATE_PAIRS
            .iter()
            .find(|(symbol, _)| *symbol == asset)
            .and_then(|(_, pairs)| pairs.first().copied())
        else {
            return (Vec::new(), true);
        };
        if !matches!(
            interval,
            "1m" | "5m" | "15m" | "30m" | "1h" | "4h" | "8h" | "1d" | "7d"
        ) {
            return (Vec::new(), true);
        }
        let pair = pair.to_ascii_uppercase();
        let limit = limit.clamp(2, 1_000);
        let key = format!("{asset}:{interval}:{limit}");
        if let Some(cached) = self.candles.read().await.get(&key) {
            // An empty series retries soon, exactly like the point series.
            let ttl = if cached.candles.is_empty() {
                EMPTY_TTL
            } else {
                CANDLE_TTL
            };
            if cached.fetched_at.elapsed() < ttl {
                return (cached.candles.clone(), false);
            }
        }
        let url = format!(
            "{}spot/candlesticks?currency_pair={pair}&interval={interval}&limit={limit}",
            self.exchange_root
        );
        let response = match self.client.get(&url).send().await {
            Ok(response) => match response.error_for_status() {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!(%error, "exchange candle refresh failed");
                    return (self.stale_candles(&key).await, true);
                }
            },
            Err(error) => {
                tracing::warn!(%error, "exchange candle refresh failed");
                return (self.stale_candles(&key).await, true);
            }
        };
        let mut candles: Vec<ExchangeCandle> =
            match response.json::<Vec<Vec<serde_json::Value>>>().await {
                Ok(payload) => payload
                    .into_iter()
                    .filter_map(|row| {
                        // [timestamp, quote volume, close, high, low, open, base volume, closed]
                        let t = as_number(row.first()?)? as i64;
                        let close = as_number(row.get(2)?)?;
                        let high = as_number(row.get(3)?)?;
                        let low = as_number(row.get(4)?)?;
                        let open = as_number(row.get(5)?)?;
                        let raw_volume = row.get(1).and_then(as_number).unwrap_or(0.0);
                        if t <= 0
                            || [open, high, low, close]
                                .iter()
                                .any(|value| !value.is_finite() || *value <= 0.0)
                        {
                            return None;
                        }
                        Some(ExchangeCandle {
                            t,
                            open,
                            high,
                            low,
                            close,
                            volume: if raw_volume.is_finite() && raw_volume > 0.0 {
                                raw_volume
                            } else {
                                0.0
                            },
                        })
                    })
                    .collect(),
                Err(error) => {
                    tracing::warn!(%error, "exchange candle response invalid");
                    return (self.stale_candles(&key).await, true);
                }
            };
        candles.sort_by_key(|candle| candle.t);
        candles.dedup_by_key(|candle| candle.t);
        self.candles.write().await.insert(
            key,
            CandleSnapshot {
                fetched_at: Instant::now(),
                candles: candles.clone(),
            },
        );
        (candles, false)
    }

    async fn stale_candles(&self, key: &str) -> Vec<ExchangeCandle> {
        self.candles
            .read()
            .await
            .get(key)
            .map(|cached| cached.candles.clone())
            .unwrap_or_default()
    }

    /// Intraday exchange series. Gate.io keeps recent candlesticks for every
    /// listed pair, which is the only intraday resolution available for the
    /// short chart ranges.
    async fn exchange_history(&self, asset: &str, period: &str, span: u32) -> Vec<SpotPoint> {
        if period != "1h" {
            return Vec::new();
        }
        // One candlestick per bucket hour minimum; finer steps while the range
        // still fits in the provider's 1000-candle page.
        let (interval, per_hour) = match span {
            0..=24 => ("5m", 12),
            25..=166 => ("15m", 4),
            _ => ("1h", 1),
        };
        let limit = (span * per_hour + 2).min(1_000);
        self.exchange_candles(asset, interval, limit)
            .await
            .0
            .into_iter()
            .map(|candle| SpotPoint {
                t: candle.t,
                usd: candle.close,
            })
            .collect()
    }
}

pub async fn market_prices(State(state): State<AppState>) -> Json<MarketPricesResponse> {
    Json(state.market_prices.list().await)
}

pub async fn market_values(
    State(state): State<AppState>,
    Json(request): Json<MarketValuesRequest>,
) -> Result<Json<MarketValuesResponse>, AppError> {
    if request.items.is_empty() || request.items.len() > 4 {
        return Err(AppError::BadRequest("Expected 1–4 asset amounts".into()));
    }
    let mut items = Vec::with_capacity(request.items.len());
    for item in request.items {
        let asset = item.asset.trim().to_ascii_uppercase();
        if !(2..=12).contains(&asset.len())
            || !asset.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(AppError::BadRequest("Invalid asset".into()));
        }
        let amount = item.amount.trim();
        if amount.len() > 40
            || !amount
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return Err(AppError::BadRequest("Invalid amount".into()));
        }
        let quantity = amount
            .parse::<f64>()
            .map_err(|_| AppError::BadRequest("Invalid amount".into()))?;
        if !quantity.is_finite() || !(0.0..=1e15).contains(&quantity) {
            return Err(AppError::BadRequest("Invalid amount".into()));
        }
        items.push((asset, amount.to_owned(), quantity));
    }
    Ok(Json(state.market_prices.value(items).await))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use axum::Router;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn initialized_prices_serve_multiple_valuations_without_refetching() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hit_count = calls.clone();
        let app = Router::new().route(
            "/api2/1/tickers",
            get(move || {
                let call = hit_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if call > 0 {
                        return Json(serde_json::json!({
                            "btc_usdt": { "last": "100" }
                        }));
                    }
                    Json(serde_json::json!({
                        "btc_usdt": { "last": "100" },
                        "usdt_usd": { "last": "0.99" },
                        "ever_usdt": { "last": "0.002" },
                        "matic_usdt": { "last": "0" }
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = app.route(
            "/chart/:id",
            get(|| async {
                Json(serde_json::json!({
                    "coins": { "coingecko:everscale": { "prices": [
                        { "timestamp": 1_700_000_000, "price": 0.002 },
                        { "timestamp": 1_700_086_400, "price": 0.0021 },
                        { "timestamp": 1_700_172_800, "price": 0.0 }
                    ] } }
                }))
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut service = MarketPriceService::with_client(
            reqwest::Client::new(),
            format!("http://{address}/"),
            format!("http://{address}/"),
        );
        service.tickers_url = format!("http://{address}/api2/1/tickers");
        service.initialize().await;
        let pair = || {
            vec![
                ("BTC".to_owned(), "2".to_owned(), 2.0),
                ("USDT".to_owned(), "3".to_owned(), 3.0),
            ]
        };
        let (first, second) = tokio::join!(service.value(pair()), service.value(pair()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.values[0].value_usd, Some(200.0));
        assert!((first.values[1].value_usd.unwrap() - 2.97).abs() < 1e-10);
        assert_eq!(second.values[0].value_usd, Some(200.0));
        assert!(!first.stale);
        let ever = service
            .value(vec![("EVER".into(), "100".into(), 100.0)])
            .await;
        assert_eq!(ever.values[0].value_usd, Some(0.2));

        service.refresh().await;
        assert!((service.value(pair()).await.values[1].value_usd.unwrap() - 2.97).abs() < 1e-10);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let (points, stale) = service.daily_history("EVER", 30).await;
        assert!(!stale);
        assert_eq!(
            points,
            vec![
                SpotPoint {
                    t: 1_700_000_000,
                    usd: 0.002
                },
                SpotPoint {
                    t: 1_700_086_400,
                    usd: 0.0021
                },
            ]
        );
        assert!(service.daily_history("EVER", 30).await.0.len() == 2);
        assert!(service.daily_history("NOPE", 30).await.1);

        service.tickers_url = "http://127.0.0.1:1/unavailable".to_owned();
        service.refresh().await;
        assert_eq!(service.value(pair()).await.values[0].value_usd, Some(200.0));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hourly_history_uses_exchange_candles_when_available() {
        let app = Router::new()
            .route(
                "/chart/:id",
                get(|| async { Json(serde_json::json!({ "coins": {} })) }),
            )
            .route(
                "/spot/candlesticks",
                get(|| async {
                    Json(serde_json::json!([
                        [
                            "1700000000",
                            "1",
                            "0.002",
                            "0.002",
                            "0.002",
                            "0.002",
                            "2",
                            true
                        ],
                        [
                            "1700000300",
                            "1",
                            "0.0021",
                            "0.002",
                            "0.002",
                            "0.002",
                            "2",
                            true
                        ],
                        ["1700000600", "0", "0", "0", "0", "0", "0", false],
                        [1700000900, 0, 0.0022, 0.0022, 0.0022, 0.0022, 0, false]
                    ]))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let service = MarketPriceService::with_client(
            reqwest::Client::new(),
            format!("http://{address}/"),
            format!("http://{address}/"),
        );
        let (points, stale) = service.spot_history("EVER", "1h", 24).await;
        assert!(!stale);
        assert_eq!(
            points,
            vec![
                SpotPoint {
                    t: 1_700_000_000,
                    usd: 0.002
                },
                SpotPoint {
                    t: 1_700_000_300,
                    usd: 0.0021
                },
                SpotPoint {
                    t: 1_700_000_900,
                    usd: 0.0022
                },
            ]
        );
    }
}

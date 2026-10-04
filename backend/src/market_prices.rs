//! Indicative USD values for the crypto assets in the public picker.
//!
//! A single cached DefiLlama request serves all visitors. Symbols are mapped
//! explicitly because looking up a token by ticker can select a different coin.

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
const MAX_STALE_AGE: Duration = Duration::from_secs(6 * 60 * 60);
const MAX_PROVIDER_PRICE_AGE_SECS: i64 = 15 * 60;
const DEFILLAMA_BASE: &str = "https://coins.llama.fi/prices/current/";

// The Open Network's CoinGecko ID now reports GRAM, the renamed native TON.
// MATIC's legacy ticker is priced through its wrapped Ethereum contract.
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
    endpoint: String,
    cache: Arc<RwLock<Option<PriceSnapshot>>>,
}

#[derive(Clone)]
struct PriceSnapshot {
    fetched_at: Instant,
    updated_at: i64,
    prices: HashMap<&'static str, MarketPrice>,
}

#[derive(Clone)]
struct MarketPrice {
    usd: f64,
    timestamp: i64,
}

#[derive(Deserialize)]
struct ProviderResponse {
    coins: HashMap<String, ProviderPrice>,
}

#[derive(Deserialize)]
struct ProviderPrice {
    price: f64,
    timestamp: i64,
    confidence: Option<f64>,
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
        Ok(Self::with_client(client, DEFILLAMA_BASE.to_owned()))
    }

    fn with_client(client: reqwest::Client, base: String) -> Self {
        let ids = ASSETS
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>()
            .join(",");
        Self {
            client,
            endpoint: format!("{base}{ids}"),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch immediately at server startup, then on a one-hour timer anchored to that startup.
    pub async fn initialize(&self) {
        let started_at = tokio::time::Instant::now();
        self.refresh().await;
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval_at(started_at + REFRESH_INTERVAL, REFRESH_INTERVAL);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                service.refresh().await;
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

    pub async fn list(&self) -> MarketPricesResponse {
        let snapshot = self.prices().await;
        MarketPricesResponse {
            source: "DefiLlama",
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
            .get(&self.endpoint)
            .send()
            .await?
            .error_for_status()?;
        let payload = response.json::<ProviderResponse>().await?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let prices: HashMap<&'static str, MarketPrice> = ASSETS
            .iter()
            .filter_map(|(asset, id)| {
                let price = payload.coins.get(*id)?;
                let age = now - price.timestamp;
                if !price.price.is_finite()
                    || price.price <= 0.0
                    || !(0..=MAX_PROVIDER_PRICE_AGE_SECS).contains(&age)
                    || price.confidence.is_some_and(|confidence| confidence < 0.8)
                {
                    return None;
                }
                Some((
                    *asset,
                    MarketPrice {
                        usd: price.price,
                        timestamp: price.timestamp,
                    },
                ))
            })
            .collect();
        if prices.is_empty() {
            anyhow::bail!("price provider returned no recent supported assets");
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
            source: "DefiLlama",
            stale,
            values,
        }
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
            "/prices/current/:coins",
            get(move || {
                let call = hit_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if call > 0 {
                        return Json(serde_json::json!({ "coins": {
                            "coingecko:bitcoin": { "price": 100.0, "timestamp": now_epoch(), "confidence": 0.99 }
                        }}));
                    }
                    Json(serde_json::json!({ "coins": {
                        "coingecko:bitcoin": { "price": 100.0, "timestamp": now_epoch(), "confidence": 0.99 },
                        "coingecko:tether": { "price": 0.99, "timestamp": now_epoch(), "confidence": 0.99 }
                    }}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut service = MarketPriceService::with_client(
            reqwest::Client::new(),
            format!("http://{address}/prices/current/"),
        );
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

        service.refresh().await;
        assert!((service.value(pair()).await.values[1].value_usd.unwrap() - 2.97).abs() < 1e-10);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        service.endpoint = "http://127.0.0.1:1/unavailable".to_owned();
        service.refresh().await;
        assert_eq!(service.value(pair()).await.values[0].value_usd, Some(200.0));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    fn now_epoch() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

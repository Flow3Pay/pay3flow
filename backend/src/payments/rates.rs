use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::payments::model::Minor;

/// In-memory foreign-exchange client with a TTL cache (PLAN #42).
///
/// Source is configurable: a deterministic local table (`mock`) or a live HTTP
/// endpoint that returns `{"rates": {"<to>": x}}` for a base currency (`http`).
/// Conversion applies a configurable margin against the recorded rate (PLAN #43:
/// "конвертация на маршруте, margin в худшем случае").
#[derive(Clone)]
pub struct Rates {
    source: RateSource,
    cache: std::sync::Arc<Mutex<HashMap<(String, String), Cached>>>,
    ttl: Duration,
    base: String,
    margin_percent: f64,
    client: reqwest::Client,
}

#[derive(Clone)]
enum RateSource {
    Mock,
    Http { url: String },
}

struct Cached {
    rate: f64,
    fetched_at: Instant,
}

impl Rates {
    pub fn mock() -> Self {
        Self::new(RateSource::Mock, "EUR".into())
    }

    pub fn http(url: String, base: String) -> Self {
        Self::new(RateSource::Http { url }, base)
    }

    fn new(source: RateSource, base: String) -> Self {
        Self {
            source,
            cache: std::sync::Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(300),
            base,
            margin_percent: 0.0,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client builds"),
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_margin_percent(mut self, margin_percent: f64) -> Self {
        self.margin_percent = margin_percent;
        self
    }

    /// Mid-market rate `from -> to` (no margin, no fees).
    pub async fn rate(&self, from: &str, to: &str) -> Result<f64> {
        let from = from.to_uppercase();
        let to = to.to_uppercase();
        if from == to {
            return Ok(1.0);
        }
        if let Some(rate) = self.cached(&from, &to) {
            return Ok(rate);
        }
        let rate = self.fetch(&from, &to).await?;
        self.remember(&from, &to, rate);
        // Reverse direction can be derived for free.
        if rate != 0.0 {
            self.remember(&to, &from, 1.0 / rate);
        }
        Ok(rate)
    }

    /// Conversion `from_amount` (minor units in `from`) to minor units in `to`.
    /// Applies the configured margin: the user is quoted the worst case
    /// (rate reduced / inflated by `margin_percent`).
    pub async fn convert(&self, from_amount: Minor, from: &str, to: &str) -> Result<Minor> {
        if from.eq_ignore_ascii_case(to) {
            return Ok(from_amount);
        }
        let mid = self.rate(from, to).await?;
        let eff = mid * (1.0 - self.margin_percent / 100.0);
        Ok((from_amount as f64 * eff).round() as Minor)
    }

    pub fn margin_percent(&self) -> f64 {
        self.margin_percent
    }

    fn cached(&self, from: &str, to: &str) -> Option<f64> {
        let cache = self.cache.lock().unwrap();
        cache
            .get(&(from.to_string(), to.to_string()))
            .and_then(|c| {
                if c.fetched_at.elapsed() < self.ttl {
                    Some(c.rate)
                } else {
                    None
                }
            })
    }

    fn remember(&self, from: &str, to: &str, rate: f64) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(
            (from.to_string(), to.to_string()),
            Cached {
                rate,
                fetched_at: Instant::now(),
            },
        );
    }

    async fn fetch(&self, from: &str, to: &str) -> Result<f64> {
        match &self.source {
            RateSource::Mock => mock_rate(&self.base, from, to),
            RateSource::Http { url } => {
                let url = format!("{url}?base={from}&symbols={to}");
                let body: serde_json::Value = self
                    .client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| anyhow!("fx request failed: {e}"))?
                    .error_for_status()
                    .map_err(|e| anyhow!("fx request failed: {e}"))?
                    .json()
                    .await
                    .map_err(|e| anyhow!("fx response not json: {e}"))?;
                body.get("rates")
                    .and_then(|r| r.get(to).and_then(serde_json::Value::as_f64))
                    .ok_or_else(|| anyhow!("fx response has no rate for {to}"))
            }
        }
    }
}

/// Deterministic mock rates from a fixed EUR-base table (used in dev and tests
/// so the whole pipeline runs with no network access).
fn mock_rate(base: &str, from: &str, to: &str) -> Result<f64> {
    fn to_eur(currency: &str) -> Option<f64> {
        Some(match currency {
            "EUR" => 1.0,
            "USD" => 1.09,
            "GBP" => 0.86,
            "JPY" => 158.0,
            "PLN" => 4.31,
            "BYN" => 3.5,
            "RUB" => 98.0,
            _ => return None,
        })
    }
    let from_eur = to_eur(from).ok_or_else(|| anyhow!("mock rate missing {from}"))?;
    let to_eur = to_eur(to).ok_or_else(|| anyhow!("mock rate missing {to}"))?;
    let _ = base;
    Ok(to_eur / from_eur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identity_currency_is_one() {
        let rates = Rates::mock();
        assert_eq!(rates.rate("USD", "USD").await.unwrap(), 1.0);
    }

    #[tokio::test]
    async fn mock_table_has_consistent_cross() {
        let rates = Rates::mock();
        let usd_eur = rates.rate("USD", "EUR").await.unwrap();
        let eur_usd = rates.rate("EUR", "USD").await.unwrap();
        assert!(usd_eur < 1.0);
        assert!((usd_eur * eur_usd - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn conversion_applies_margin_and_rounds() {
        let rates = Rates::mock().with_margin_percent(2.0);
        // 100 EUR -> USD at mid 1.09, margin 2% => eff 1.0682 => 106.82 (ints).
        let converted = rates.convert(10000, "EUR", "USD").await.unwrap();
        assert_eq!(converted, 10682); // 106.82 USD in cents
    }

    #[tokio::test]
    async fn cached_rate_served_second_time() {
        let rates = Rates::mock();
        rates.rate("EUR", "USD").await.unwrap();
        let cache = rates.cache.lock().unwrap();
        assert!(cache.contains_key(&("EUR".to_string(), "USD".to_string())));
        assert!(cache.contains_key(&("USD".to_string(), "EUR".to_string())));
    }

    #[tokio::test]
    async fn unknown_currency_errors() {
        let rates = Rates::mock();
        assert!(rates.rate("EUR", "XYZ").await.is_err());
    }
}

//! Authenticated Crypto Office pair discovery and indicative, amount-specific quotes.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::{Client, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Deserializer};
use serde_json::value::RawValue;
use tokio::sync::Mutex;

use crate::route_engine::{
    atomic_to_decimal, Amount, Asset, PublicRouteProvider, PublicRouteQuote, QuoteUnavailable,
};

const API_URL: &str = "https://public.crypto-office.com/api/v1/";
const SOURCE_URL: &str = "https://crypto-office.com/ru/exchange";
const CATALOG_TTL: Duration = Duration::from_secs(300);
const PRECISION: u8 = 18;

/// Decimal fixed-point units parsed directly from JSON lexemes, without f64 rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Decimal(u128);

impl Decimal {
    fn parse(value: &str) -> Result<Self> {
        if value.len() > 128 {
            bail!("Crypto Office decimal is too long");
        }
        let (mantissa, exponent) = value.split_once(['e', 'E']).unwrap_or((value, "0"));
        let exponent: i32 = exponent.parse().context("invalid decimal exponent")?;
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if whole.is_empty()
            || !whole
                .bytes()
                .chain(fraction.bytes())
                .all(|b| b.is_ascii_digit())
        {
            bail!("Crypto Office decimal must be non-negative");
        }
        let coefficient: u128 = format!("{whole}{fraction}")
            .parse()
            .context("decimal overflow")?;
        let scale = i32::from(PRECISION)
            .checked_add(exponent)
            .and_then(|value| value.checked_sub(fraction.len() as i32))
            .context("decimal scale overflow")?;
        let factor = 10_u128
            .checked_pow(scale.unsigned_abs())
            .context("unsupported decimal scale")?;
        let units = if scale >= 0 {
            coefficient
                .checked_mul(factor)
                .context("decimal overflow")?
        } else {
            if !coefficient.is_multiple_of(factor) {
                bail!("decimal exceeds supported precision");
            }
            coefficient / factor
        };
        Ok(Self(units))
    }

    fn text(self) -> Result<String> {
        atomic_to_decimal(&self.0.to_string(), PRECISION)
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        let value = raw.get();
        let text = if value.starts_with('"') {
            serde_json::from_str::<String>(value).map_err(serde::de::Error::custom)?
        } else {
            value.to_owned()
        };
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    status: bool,
    data: Option<T>,
}

#[derive(Deserialize)]
struct Pair {
    from: String,
    to: String,
    min_from: Decimal,
    max_from: Decimal,
    min_to: Decimal,
    max_to: Option<Decimal>,
}

#[derive(Deserialize)]
struct Rate {
    swap_rate_id: i64,
    currency_from: String,
    currency_to: String,
    amount: Decimal,
    result_amount: Decimal,
    network_fee: Decimal,
}

struct Catalog {
    pairs: HashMap<(Asset, Asset), Pair>,
    assets: Vec<Asset>,
}
struct CachedCatalog {
    loaded_at: Instant,
    catalog: Arc<Catalog>,
}

/// Enabled only when the server has an External Auth token. Never creates orders.
#[derive(Clone)]
pub struct CryptoOfficeRouteProvider {
    client: Client,
    api_url: Url,
    authorization: HeaderValue,
    catalog: Arc<RwLock<Option<CachedCatalog>>>,
    refresh: Arc<Mutex<()>>,
}

impl CryptoOfficeRouteProvider {
    pub fn from_env() -> Result<Option<Self>> {
        let Some(token) = std::env::var("CRYPTO_OFFICE_API_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty())
        else {
            return Ok(None);
        };
        Self::with_endpoint(API_URL, token.trim()).map(Some)
    }

    fn with_endpoint(api_url: &str, token: &str) -> Result<Self> {
        let mut authorization = HeaderValue::from_str(&format!("External {token}"))
            .map_err(|_| anyhow!("invalid CRYPTO_OFFICE_API_TOKEN header"))?;
        authorization.set_sensitive(true);
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Pay3Flow/0.1 (+https://pay3flow.lefine.pro)")
                .build()?,
            api_url: Url::parse(api_url)?,
            authorization,
            catalog: Arc::new(RwLock::new(None)),
            refresh: Arc::new(Mutex::new(())),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let response = self
            .client
            .get(self.api_url.join(path)?)
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, self.authorization.clone())
            .query(query)
            .send()
            .await
            .context("Crypto Office request failed")?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND && path == "swap/get-rate-info" {
            return Err(QuoteUnavailable {
                provider: "crypto-office",
                reason: "swap unavailable for this amount".into(),
            }
            .into());
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            bail!("Crypto Office authentication rejected; check CRYPTO_OFFICE_API_TOKEN and account access");
        }
        if !status.is_success() {
            bail!("Crypto Office API returned HTTP {status}");
        }
        let response: ApiResponse<T> = response
            .json()
            .await
            .context("invalid Crypto Office API response")?;
        if !response.status {
            bail!("Crypto Office API returned status=false");
        }
        response
            .data
            .context("Crypto Office API response is missing data")
    }

    fn cached_catalog(&self, fresh_only: bool) -> Result<Option<Arc<Catalog>>> {
        let guard = self
            .catalog
            .read()
            .map_err(|_| anyhow!("Crypto Office catalog lock poisoned"))?;
        Ok(guard
            .as_ref()
            .filter(|cached| !fresh_only || cached.loaded_at.elapsed() < CATALOG_TTL)
            .map(|cached| Arc::clone(&cached.catalog)))
    }

    async fn catalog(&self) -> Result<Arc<Catalog>> {
        if let Some(catalog) = self.cached_catalog(true)? {
            return Ok(catalog);
        }
        // Serialize refreshes without holding a synchronous guard across network I/O.
        let _refresh = self.refresh.lock().await;
        if let Some(catalog) = self.cached_catalog(true)? {
            return Ok(catalog);
        }
        let pairs: HashMap<String, Pair> = self.get("swap/list-available-pairs", &[]).await?;
        let mut supported = HashMap::new();
        for pair in pairs.into_values() {
            let (Some(from), Some(to)) = (provider_asset(&pair.from), provider_asset(&pair.to))
            else {
                continue;
            };
            if from == to
                || pair.min_from > pair.max_from
                || pair.max_to.is_some_and(|max| max < pair.min_to)
            {
                continue;
            }
            supported.insert((from, to), pair);
        }
        let mut assets = supported
            .keys()
            .flat_map(|(from, to)| [from.clone(), to.clone()])
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        assets.sort_by_key(ToString::to_string);
        let catalog = Arc::new(Catalog {
            pairs: supported,
            assets,
        });
        *self
            .catalog
            .write()
            .map_err(|_| anyhow!("Crypto Office catalog lock poisoned"))? = Some(CachedCatalog {
            loaded_at: Instant::now(),
            catalog: Arc::clone(&catalog),
        });
        Ok(catalog)
    }
}

/// Provider coin identifiers qualify the network; plain USDT is ambiguous.
fn provider_asset(code: &str) -> Option<Asset> {
    let code = code.to_ascii_lowercase();
    let (symbol, network) = match code.as_str() {
        "btc" => ("BTC", "bitcoin"),
        "eth" => ("ETH", "ethereum"),
        "bnb" => ("BNB", "bnb-smart-chain"),
        "sol" => ("SOL", "solana"),
        "trx" => ("TRX", "tron"),
        _ => {
            let (symbol, suffix) = code.rsplit_once('_')?;
            let network = match suffix {
                "eth" => "ethereum",
                "bnb" => "bnb-smart-chain",
                "trx" => "tron",
                _ => return None,
            };
            (symbol, network)
        }
    };
    Asset::new(symbol, Some(network)).ok()
}

#[async_trait]
impl PublicRouteProvider for CryptoOfficeRouteProvider {
    fn name(&self) -> &str {
        "crypto-office"
    }

    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        self.cached_catalog(false)
            .ok()
            .flatten()
            .is_some_and(|catalog| catalog.pairs.contains_key(&(from.clone(), to.clone())))
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        match self.catalog().await {
            Ok(catalog) => catalog.assets.clone(),
            Err(error) => {
                tracing::warn!(%error, provider = "crypto-office", "Crypto Office pair catalog unavailable");
                Vec::new()
            }
        }
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if amount.asset != from {
            bail!("Crypto Office input asset does not match the route");
        }
        let input = Decimal::parse(&amount.value)?;
        let catalog = self.catalog().await?;
        let pair = catalog
            .pairs
            .get(&(from.clone(), to.clone()))
            .ok_or_else(|| QuoteUnavailable {
                provider: "crypto-office",
                reason: "direction is not in the available pair catalog".into(),
            })?;
        if input.0 == 0 || input < pair.min_from || input > pair.max_from {
            return Err(QuoteUnavailable {
                provider: "crypto-office",
                reason: "input amount is outside the swap limits".into(),
            }
            .into());
        }
        let rate: Rate = self
            .get(
                "swap/get-rate-info",
                &[
                    ("currency_from", pair.from.clone()),
                    ("currency_to", pair.to.clone()),
                    ("amount", input.text()?),
                ],
            )
            .await?;
        if rate.swap_rate_id <= 0
            || rate.currency_from != pair.from
            || rate.currency_to != pair.to
            || rate.amount != input
        {
            bail!("Crypto Office returned a mismatched swap quote");
        }
        if rate.result_amount < pair.min_to
            || pair.max_to.is_some_and(|max| rate.result_amount > max)
        {
            return Err(QuoteUnavailable {
                provider: "crypto-office",
                reason: "output amount is outside the swap limits".into(),
            }
            .into());
        }
        let net = rate
            .result_amount
            .0
            .checked_sub(rate.network_fee.0)
            .filter(|value| *value > 0)
            .context("Crypto Office swap fees consume the quoted output")?;
        Ok(PublicRouteQuote {
            provider: self.name().into(), quote_id: Some(rate.swap_rate_id.to_string()),
            description: Some("Crypto Office indicative swap; confirm recipient-dependent fees and account funding before execution.".into()),
            source_url: Some(SOURCE_URL.into()), from: from.clone(), to: to.clone(), input: amount,
            output: Amount::new(Decimal(net).text()?, to.clone())?,
            fees: vec![Amount::new(rate.network_fee.text()?, to.clone())?],
            expires_at: Some(chrono::Utc::now() + chrono::Duration::seconds(30)), path: vec![from, to],
        })
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/providers/crypto-office/tests.rs"
    ));
}

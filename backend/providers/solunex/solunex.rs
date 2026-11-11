//! Public calculator discovery only. Fixed commissions require an exact-amount
//! quote; neither quote tokens nor customer/order endpoints are exposed here.
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::networks::NetworkCatalog;
use crate::route_engine::{Amount, Asset, PublicRouteProvider, PublicRouteQuote};

const HOME: &str = "https://solunex.ge/en";
const CATALOG_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Office {
    id: String,
    name_en: String,
    address_en: String,
    #[serde(default)]
    working_hours_en: String,
    is_active: bool,
    // Missing availability must never make an unopened office bookable.
    #[serde(default = "unavailable")]
    coming_soon: bool,
}
fn unavailable() -> bool {
    true
}

#[derive(Clone)]
struct OfficeCatalog {
    office: Office,
    codes: BTreeMap<String, Asset>,
    pairs: HashSet<(String, String)>,
}

#[derive(Clone)]
struct Catalog {
    loaded: Instant,
    offices: Vec<OfficeCatalog>,
}

// Record completion even when a caller cancels an in-flight catalog refresh.
struct CatalogAttempt<'a>(&'a mut Option<Instant>);
impl Drop for CatalogAttempt<'_> {
    fn drop(&mut self) { *self.0 = Some(Instant::now()); }
}

#[derive(Clone)]
pub struct SolunexProvider {
    client: Client,
    base_url: String,
    networks: NetworkCatalog,
    catalog: Arc<RwLock<Option<Catalog>>>,
    refresh: Arc<Mutex<Option<Instant>>>,
}

impl SolunexProvider {
    pub fn new(networks: NetworkCatalog) -> Result<Self> {
        Ok(Self {
            client: Client::builder().timeout(Duration::from_secs(8)).build()?,
            base_url: "https://solunex.ge".into(),
            networks,
            catalog: Arc::new(RwLock::new(None)),
            refresh: Arc::new(Mutex::new(None)),
        })
    }

    fn cached_catalog(&self) -> Option<Catalog> {
        self.catalog
            .read()
            .ok()?
            .as_ref()
            .filter(|c| c.loaded.elapsed() < CATALOG_TTL)
            .cloned()
    }

    async fn response_json(response: reqwest::Response) -> Result<Value> {
        let mut response = response.error_for_status()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                bail!("Solunex response too large");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn load_catalog(&self) -> Result<Catalog> {
        let mut attempt = self.refresh.lock().await;
        if let Some(catalog) = self.cached_catalog() {
            return Ok(catalog);
        }
        if attempt.is_some_and(|at| at.elapsed() < Duration::from_secs(30)) {
            bail!("Solunex catalog refresh is backing off");
        }
        let _completed = CatalogAttempt(&mut attempt);
        let data = Self::response_json(
            self.client
                .get(format!("{}/api/offices", self.base_url))
                .send()
                .await?,
        )
        .await?;
        let offices: Vec<Office> = serde_json::from_value(data["offices"].clone())?;
        let mut available = Vec::new();
        for office in offices
            .into_iter()
            .filter(|o| o.is_active && !o.coming_soon)
            .take(8)
        {
            let data = Self::response_json(
                self.client
                    .get(format!("{}/api/currencies", self.base_url))
                    .query(&[("officeId", office.id.as_str()), ("detailed", "1")])
                    .send()
                    .await?,
            )
            .await?;
            available.push(self.parse_catalog(office, &data)?);
        }
        if available.is_empty() {
            bail!("Solunex has no available offices");
        }
        let catalog = Catalog {
            loaded: Instant::now(),
            offices: available,
        };
        *self
            .catalog
            .write()
            .map_err(|_| anyhow::anyhow!("Solunex catalog lock poisoned"))? = Some(catalog.clone());
        Ok(catalog)
    }

    fn parse_catalog(&self, office: Office, data: &Value) -> Result<OfficeCatalog> {
        let mut codes = BTreeMap::new();
        for currency in data["detailed"]
            .as_array()
            .context("Solunex detailed currencies missing")?
        {
            let Some(code) = currency["code"].as_str() else {
                continue;
            };
            let Some(asset) = self.map_currency(currency) else {
                continue;
            };
            codes.insert(code.into(), asset);
        }
        let pairs = data["pairs"]
            .as_array()
            .context("Solunex directions missing")?
            .iter()
            .filter_map(|pair| {
                let from = pair["base"].as_str()?;
                let to = pair["quote"].as_str()?;
                (from != to && codes.contains_key(from) && codes.contains_key(to))
                    .then(|| (from.into(), to.into()))
            })
            .collect();
        Ok(OfficeCatalog {
            office,
            codes,
            pairs,
        })
    }

    fn map_currency(&self, currency: &Value) -> Option<Asset> {
        let code = currency["code"].as_str()?;
        let symbol = currency["symbol"].as_str()?;
        if currency["type"] == "FIAT" {
            return (code == symbol && matches!(code, "GEL" | "USD" | "EUR"))
                .then(|| Asset::new(code, None).ok())
                .flatten();
        }
        if currency["type"] != "CRYPTO" {
            return None;
        }
        let network = match currency["network"].as_str()? {
            "TRC20" => "tron",
            "ERC20" => "ethereum",
            "BEP20" => "bnb-smart-chain",
            "SOL" => "solana",
            "ARBITRUM" => "arbitrum-one",
            "BTC" => "bitcoin",
            "TON" => "ton",
            _ => return None,
        };
        self.networks.compatible_network(network, symbol)?;
        Asset::new(symbol, Some(network)).ok()
    }

    async fn quotes_for(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<Vec<PublicRouteQuote>> {
        if amount.asset != from {
            bail!("Solunex input asset mismatch");
        }
        positive(&amount.value).context("Solunex input must be positive")?;
        let catalog = self.load_catalog().await?;
        let mut quotes = Vec::new();
        for office in &catalog.offices {
            for (give, receive) in &office.pairs {
                if office.codes.get(give) != Some(&from) || office.codes.get(receive) != Some(&to) {
                    continue;
                }
                let raw = Self::response_json(self.client.post(format!("{}/api/rates/calculate", self.base_url))
                    .header(reqwest::header::ORIGIN, "https://solunex.ge")
                    .header(reqwest::header::REFERER, HOME)
                    .json(&json!({ "officeId": office.office.id, "giveCurrency": give, "receiveCurrency": receive,
                        "mode": "give", "giveAmount": amount.value }))
                    .send().await?).await?;
                // A response may be a successful HTTP request with a calculator error.
                match Self::parse_quote(
                    &raw,
                    &office.office,
                    from.clone(),
                    to.clone(),
                    amount.clone(),
                ) {
                    Ok(quote) => quotes.push(quote),
                    Err(error) => tracing::debug!(%error, "Solunex quote unavailable"),
                }
            }
        }
        if quotes.is_empty() {
            bail!("Solunex has no valid quote for this direction and amount");
        }
        Ok(quotes)
    }

    fn parse_quote(
        raw: &Value,
        office: &Office,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<PublicRouteQuote> {
        if raw.get("error").is_some_and(|value| !value.is_null()) {
            bail!("Solunex calculator rejected this amount");
        }
        let given = raw["giveAmount"]
            .as_str()
            .context("Solunex input amount missing")?;
        Amount::new(given, from.clone())?;
        if decimal_parts(given) != decimal_parts(&amount.value) {
            bail!("Solunex returned a different input amount");
        }
        let output = raw["receiveAmount"]
            .as_str()
            .context("Solunex output missing")?;
        positive(output).context("Solunex output must be positive")?;
        let expiry = raw["quoteExpiresAt"]
            .as_str()
            .context("Solunex expiry missing")?
            .parse::<DateTime<Utc>>()?;
        if expiry <= Utc::now() {
            bail!("Solunex quote expired");
        }
        let description = format!("{} · {} · {}. Fiat settlement is cash at this office. Confirm availability, AML/KYC requirements, network and final amount with Solunex. Calculator commission is included in the output.", office.name_en, office.address_en, office.working_hours_en);
        Ok(PublicRouteQuote {
            provider: "solunex".into(),
            // Provider tokens stay private. The office id is sufficient for attribution.
            quote_id: Some(office.id.clone()),
            description: Some(description),
            source_url: Some(HOME.into()),
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output: Amount::new(output, to.clone())?,
            fees: vec![],
            expires_at: Some(expiry),
            path: vec![from, to],
        })
    }
}

fn positive(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn decimal_parts(value: &str) -> (&str, &str) {
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
    (
        integer.trim_start_matches('0'),
        fraction.trim_end_matches('0'),
    )
}

#[async_trait]
impl PublicRouteProvider for SolunexProvider {
    fn name(&self) -> &str {
        "solunex"
    }
    fn allows_coefficient_quotes(&self) -> bool {
        false
    }
    fn supports_payment_method(&self, asset: &Asset, method: Option<&str>) -> bool {
        asset.location.is_some() || method.is_some_and(|m| m.eq_ignore_ascii_case("Cash"))
    }
    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        self.cached_catalog().is_some_and(|catalog| {
            catalog.offices.iter().any(|office| {
                office.pairs.iter().any(|(give, receive)| {
                    office.codes.get(give) == Some(from) && office.codes.get(receive) == Some(to)
                })
            })
        })
    }
    async fn supported_assets(&self) -> Vec<Asset> {
        match self.load_catalog().await {
            Ok(catalog) => {
                let mut assets = catalog
                    .offices
                    .into_iter()
                    .flat_map(|o| o.codes.into_values())
                    .collect::<Vec<_>>();
                assets.sort_by_key(ToString::to_string);
                assets.dedup();
                assets
            }
            Err(error) => {
                tracing::warn!(%error, "Solunex catalog unavailable");
                vec![]
            }
        }
    }
    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        self.quotes_for(from, to, amount)
            .await?
            .into_iter()
            .max_by(|a, b| {
                positive(&a.output.value)
                    .partial_cmp(&positive(&b.output.value))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .context("Solunex quote missing")
    }
    async fn quotes(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<Vec<PublicRouteQuote>> {
        self.quotes_for(from, to, amount).await
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/providers/solunex/tests.rs"
    ));
}

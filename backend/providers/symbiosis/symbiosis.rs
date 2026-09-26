use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::sync::RwLock;

use crate::route_engine::{
    atomic_to_decimal, canonical_network_id, decimal_to_atomic, ensure_success, Amount, Asset,
    PublicRouteProvider, PublicRouteQuote, DEFAULT_QUOTE_TTL,
};

const DEFAULT_API_URL: &str = "https://api.symbiosis.finance/crosschain";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbiosisToken {
    symbol: String,
    address: String,
    chain_id: u64,
    decimals: u8,
    #[serde(default)]
    attributes: Option<HashMap<String, String>>,
}

/// Read-only Symbiosis cross-chain quote provider.
///
/// Token metadata is loaded lazily from the provider's public catalog. Quotes
/// use a configured preview address because the route-search API does not have
/// a user's wallet context; executable swaps must be re-quoted with wallet
/// addresses by the execution layer.
#[derive(Clone)]
pub struct SymbiosisRouteProvider {
    client: Client,
    base_url: String,
    partner_id: Option<String>,
    quote_address: String,
    slippage_bps: u32,
    tokens: Arc<RwLock<Option<Vec<SymbiosisToken>>>>,
}

impl SymbiosisRouteProvider {
    pub fn new(
        base_url: impl Into<String>,
        partner_id: Option<String>,
        quote_address: impl Into<String>,
        slippage_bps: u32,
    ) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let base_url = if base_url.is_empty() {
            DEFAULT_API_URL.to_string()
        } else {
            base_url
        };
        if !base_url.starts_with("https://") && !base_url.starts_with("http://") {
            bail!("Symbiosis API URL must use http or https");
        }
        let quote_address = quote_address.into().trim().to_string();
        if quote_address.is_empty() {
            bail!("Symbiosis quote address cannot be empty");
        }
        if slippage_bps > 10_000 {
            bail!("Symbiosis slippage must be at most 10000 basis points");
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Pay3Flow-Symbiosis/0.1")
            .build()
            .context("failed to build Symbiosis client")?;
        Ok(Self {
            client,
            base_url,
            partner_id: partner_id.and_then(|value| {
                let value = value.trim().to_string();
                (!value.is_empty()).then_some(value)
            }),
            quote_address,
            slippage_bps,
            tokens: Arc::new(RwLock::new(None)),
        })
    }

    async fn load_tokens(&self) -> Result<Vec<SymbiosisToken>> {
        if let Some(tokens) = self.tokens.read().await.clone() {
            return Ok(tokens);
        }

        let mut request = self.client.get(self.endpoint("v2/tokens"));
        if let Some(partner_id) = &self.partner_id {
            request = request.header("X-Partner-Id", partner_id);
        }
        let response = request
            .send()
            .await
            .context("request Symbiosis token catalog")?;
        ensure_success(response.status(), "request Symbiosis token catalog")?;
        let tokens = response
            .json::<Vec<SymbiosisToken>>()
            .await
            .context("decode Symbiosis token catalog")?;
        *self.tokens.write().await = Some(tokens.clone());
        Ok(tokens)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    fn token_for<'a>(
        tokens: &'a [SymbiosisToken],
        asset: &Asset,
    ) -> Result<&'a SymbiosisToken> {
        let network = asset
            .location
            .as_deref()
            .map(canonical_network_id)
            .context("Symbiosis assets must include a blockchain network")?;
        tokens
            .iter()
            .find(|token| {
                token.symbol.eq_ignore_ascii_case(&asset.symbol)
                    && network_for_chain(token.chain_id) == Some(network.as_str())
            })
            .with_context(|| format!("Symbiosis does not support {asset}"))
    }

    fn token_payload(token: &SymbiosisToken, amount: Option<String>) -> Value {
        let mut payload = Map::from_iter([
            ("address".into(), Value::String(token.address.clone())),
            ("chainId".into(), Value::from(token.chain_id)),
            ("decimals".into(), Value::from(token.decimals)),
            ("symbol".into(), Value::String(token.symbol.clone())),
        ]);
        if let Some(amount) = amount {
            payload.insert("amount".into(), Value::String(amount));
        }
        if let Some(attributes) = &token.attributes {
            payload.insert("attributes".into(), json!(attributes));
        }
        Value::Object(payload)
    }

    fn parse_fee(
        value: &Value,
        tokens: &[SymbiosisToken],
    ) -> Result<Option<Amount>> {
        let Some(symbol) = value.get("symbol").and_then(Value::as_str) else {
            return Ok(None);
        };
        let Some(chain_id) = value.get("chainId").and_then(Value::as_u64) else {
            return Ok(None);
        };
        let Some(amount) = value.get("amount").and_then(Value::as_str) else {
            return Ok(None);
        };
        let decimals = value
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .or_else(|| {
                tokens
                    .iter()
                    .find(|token| token.chain_id == chain_id && token.symbol.eq_ignore_ascii_case(symbol))
                    .map(|token| token.decimals)
            });
        let Some(network) = network_for_chain(chain_id) else {
            return Ok(None);
        };
        let asset = Asset::new(symbol, Some(network))?;
        let amount = atomic_to_decimal(amount, decimals.context("Symbiosis fee decimals missing")?)?;
        Ok(Some(Amount::new(amount, asset)?))
    }

    fn quote_expiry(raw: &Value) -> Option<DateTime<Utc>> {
        raw.pointer("/tx/validUntil")
            .and_then(Value::as_i64)
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
            .or_else(|| Some(Utc::now() + DEFAULT_QUOTE_TTL))
    }
}

#[async_trait]
impl PublicRouteProvider for SymbiosisRouteProvider {
    fn name(&self) -> &str {
        "symbiosis"
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        let Ok(tokens) = self.load_tokens().await else {
            return Vec::new();
        };
        let mut assets = tokens
            .iter()
            .filter_map(|token| {
                network_for_chain(token.chain_id)
                    .and_then(|network| Asset::new(&token.symbol, Some(network)).ok())
            })
            .collect::<Vec<_>>();
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if from == to {
            bail!("Symbiosis source and destination assets must differ");
        }
        if amount.asset != from {
            bail!("Symbiosis quote amount asset must equal the origin asset");
        }
        let tokens = self.load_tokens().await?;
        let from_token = Self::token_for(&tokens, &from)?.clone();
        let to_token = Self::token_for(&tokens, &to)?.clone();
        let atomic_amount = decimal_to_atomic(&amount.value, Some(from_token.decimals))?;
        let body = json!({
            "tokenAmountIn": Self::token_payload(&from_token, Some(atomic_amount)),
            "tokenOut": Self::token_payload(&to_token, None),
            "from": self.quote_address,
            "to": self.quote_address,
            "slippage": self.slippage_bps,
        });

        // Keep the v1 operation documented by the provider request. Its
        // response is also the quote-and-calldata shape used by the route API.
        let mut request = self.client.post(self.endpoint("v1/swap")).json(&body);
        if let Some(partner_id) = &self.partner_id {
            request = request.header("X-Partner-Id", partner_id);
        }
        let response = request.send().await.context("request Symbiosis quote")?;
        let status = response.status();
        let raw = response
            .json::<Value>()
            .await
            .context("decode Symbiosis quote")?;
        if !status.is_success() {
            bail!("Symbiosis quote returned HTTP {status}: {}", raw);
        }
        let output = raw
            .get("tokenAmountOut")
            .context("Symbiosis quote has no tokenAmountOut")?;
        let output_amount = output
            .get("amount")
            .and_then(Value::as_str)
            .context("Symbiosis quote has no output amount")?;
        let output_decimals = output
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(to_token.decimals);
        let output = Amount::new(
            atomic_to_decimal(output_amount, output_decimals)?,
            to.clone(),
        )?;
        let mut fees = Vec::new();
        if let Some(values) = raw.get("fees").and_then(Value::as_array) {
            for value in values.iter().filter_map(|fee| fee.get("value")) {
                if let Some(fee) = Self::parse_fee(value, &tokens)? {
                    if !fees.iter().any(|existing: &Amount| existing.asset == fee.asset && existing.value == fee.value) {
                        fees.push(fee);
                    }
                }
            }
        }
        Ok(PublicRouteQuote {
            provider: self.name().into(),
            quote_id: Some(format!("symbiosis:{}>{}", from, to)),
            description: raw
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_string),
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output,
            fees,
            expires_at: Self::quote_expiry(&raw),
            path: vec![from, to],
        })
    }
}

fn network_for_chain(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "ethereum",
        56 => "bnb-smart-chain",
        137 => "polygon-pos",
        43114 => "avalanche-c",
        10 => "optimism",
        42161 => "arbitrum-one",
        8453 => "base",
        728126428 => "tron",
        85918 => "ton",
        3652501241 => "bitcoin",
        5426 => "solana",
        14400144 => "xrpl",
        300003 => "dogecoin",
        200002 => "litecoin",
        14500145 => "bitcoin-cash",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_chain_ids_to_pay3flow_networks() {
        assert_eq!(network_for_chain(1), Some("ethereum"));
        assert_eq!(network_for_chain(728126428), Some("tron"));
        assert_eq!(network_for_chain(5426), Some("solana"));
        assert_eq!(network_for_chain(999_999), None);
    }

    #[test]
    fn builds_native_token_payload_without_losing_amount() {
        let token = SymbiosisToken {
            symbol: "ETH".into(),
            address: String::new(),
            chain_id: 1,
            decimals: 18,
            attributes: None,
        };
        let payload = SymbiosisRouteProvider::token_payload(&token, Some("100".into()));
        assert_eq!(payload["address"], "");
        assert_eq!(payload["amount"], "100");
        assert_eq!(payload["chainId"], 1);
    }

    #[tokio::test]
    #[ignore = "requires Symbiosis network access"]
    async fn live_catalog_and_quote_support_cross_chain_assets() {
        let provider = SymbiosisRouteProvider::new(
            DEFAULT_API_URL,
            None,
            "0xf93d011544e89a28b5bdbdd833016cc5f26e82cd",
            300,
        )
        .unwrap();
        let assets = provider.supported_assets().await;
        assert!(assets.contains(&Asset::parse("ETH@ethereum").unwrap()));
        assert!(assets.contains(&Asset::parse("USDT@tron").unwrap()));
        let from = Asset::parse("ETH@ethereum").unwrap();
        let amount = Amount::new("0.01", from.clone()).unwrap();
        let quote = provider
            .quote(from, Asset::parse("USDT@tron").unwrap(), amount)
            .await
            .unwrap();
        assert!(quote.output.value.parse::<f64>().unwrap() > 0.0);
        assert!(!quote.fees.is_empty());
    }
}

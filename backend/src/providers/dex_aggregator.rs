use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

use crate::route_engine::{
    atomic_to_decimal, canonical_network_id, decimal_to_atomic, ensure_success, Amount, Asset,
    PublicRouteProvider, PublicRouteQuote, DEFAULT_QUOTE_TTL,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DexKind {
    Lifi,
    KyberSwap,
    Nordstern,
    Velora,
}

#[derive(Clone)]
pub struct DexAggregatorRouteProvider {
    kind: DexKind,
    name: &'static str,
    client: Client,
    tokens: Arc<HashMap<(String, String), String>>,
    quote_address: String,
}

impl DexAggregatorRouteProvider {
    pub fn from_config(
        name: &str,
        token_entries: &[String],
        quote_address: &str,
    ) -> Result<Option<Self>> {
        let kind = match name {
            "lifi" => DexKind::Lifi,
            "kyberswap" => DexKind::KyberSwap,
            "nordstern" => DexKind::Nordstern,
            "velora" => DexKind::Velora,
            _ => bail!("unsupported public DEX quote provider {name}"),
        };
        let tokens = token_entries
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .filter_map(|(key, address)| {
                let (chain, symbol) = key.split_once(':')?;
                let chain = canonical_network_id(chain);
                let symbol = symbol.trim().to_ascii_uppercase();
                let address = address.trim().to_string();
                (!chain.is_empty() && !symbol.is_empty() && !address.is_empty())
                    .then_some(((chain, symbol), address))
            })
            .collect::<HashMap<_, _>>();
        if tokens.is_empty() {
            return Ok(None);
        }
        let quote_address = quote_address.trim();
        if !is_evm_address(quote_address) {
            bail!("DEX quote address must be an EVM address");
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Pay3Flow/0.1 (+https://pay3flow.lefine.pro)")
            .build()
            .context("failed to build public DEX quote client")?;
        Ok(Some(Self {
            kind,
            name: match kind {
                DexKind::Lifi => "lifi",
                DexKind::KyberSwap => "kyberswap",
                DexKind::Nordstern => "nordstern",
                DexKind::Velora => "velora",
            },
            client,
            tokens: Arc::new(tokens),
            quote_address: quote_address.to_string(),
        }))
    }

    fn decimals(symbol: &str) -> u8 {
        match symbol.to_ascii_uppercase().as_str() {
            "USDC" | "USDT" | "FDUSD" | "USDE" => 6,
            "WBTC" | "BTCB" => 8,
            _ => 18,
        }
    }

    fn chain_id(chain: &str) -> Result<u64> {
        Ok(match canonical_network_id(chain).as_str() {
            "ethereum" => 1,
            "bnb-smart-chain" => 56,
            "gnosis" => 100,
            "polygon-pos" => 137,
            "optimism" => 10,
            "arbitrum-one" => 42161,
            "avalanche-c" => 43114,
            "base" => 8453,
            other => bail!("{} does not support EVM network {other}", chain),
        })
    }

    fn kyber_chain(chain: &str) -> Result<&'static str> {
        Ok(match canonical_network_id(chain).as_str() {
            "ethereum" => "ethereum",
            "bnb-smart-chain" => "bsc",
            "polygon-pos" => "polygon",
            "optimism" => "optimism",
            "arbitrum-one" => "arbitrum",
            "avalanche-c" => "avalanche",
            "base" => "base",
            other => bail!("KyberSwap does not support EVM network {other}"),
        })
    }

    fn token_address(&self, asset: &Asset) -> Result<String> {
        let chain = asset
            .location
            .as_deref()
            .map(canonical_network_id)
            .context("DEX quote requires a blockchain network")?;
        self.tokens
            .get(&(chain, asset.symbol.to_ascii_uppercase()))
            .cloned()
            .with_context(|| format!("{} token address is not configured for {asset}", self.name))
    }

    async fn request_quote(&self, from: &Asset, to: &Asset, atomic_amount: &str) -> Result<Value> {
        let chain = from
            .location
            .as_deref()
            .map(canonical_network_id)
            .context("DEX quote origin network is required")?;
        if to.location.as_deref().map(canonical_network_id).as_deref() != Some(chain.as_str()) {
            bail!("{} only quotes same-chain swaps", self.name);
        }
        let chain_id = Self::chain_id(&chain)?;
        let from_token = self.token_address(from)?;
        let to_token = self.token_address(to)?;
        let response = match self.kind {
            DexKind::Lifi => self
                .client
                .get("https://li.quest/v1/quote")
                .query(&[
                    ("fromChain", chain_id.to_string()),
                    ("toChain", chain_id.to_string()),
                    ("fromToken", from_token),
                    ("toToken", to_token),
                    ("fromAmount", atomic_amount.to_string()),
                    ("fromAddress", self.quote_address.clone()),
                ])
                .header("accept", "application/json")
                .send()
                .await
                .context("request LI.FI quote")?,
            DexKind::KyberSwap => {
                let endpoint = format!(
                    "https://aggregator-api.kyberswap.com/{}/api/v1/routes",
                    Self::kyber_chain(&chain)?
                );
                self.client
                    .get(endpoint)
                    .query(&[
                        ("tokenIn", from_token),
                        ("tokenOut", to_token),
                        ("amountIn", atomic_amount.to_string()),
                    ])
                    .header("X-Client-Id", "pay3flow")
                    .header("accept", "application/json")
                    .send()
                    .await
                    .context("request KyberSwap quote")?
            }
            DexKind::Nordstern => {
                let endpoint = format!("https://api.nordstern.finance/aggregator/{chain_id}");
                self.client
                    .get(endpoint)
                    .query(&[
                        ("src", from_token),
                        ("dst", to_token),
                        ("amount", atomic_amount.to_string()),
                        ("from", self.quote_address.clone()),
                        ("slippage", "0.5".to_string()),
                    ])
                    .header("Referer", "https://pay3flow.lefine.pro")
                    .header("accept", "application/json")
                    .send()
                    .await
                    .context("request Nordstern quote")?
            }
            DexKind::Velora => self
                .client
                .get("https://api.velora.xyz/prices")
                .query(&[
                    ("srcToken", from_token),
                    ("destToken", to_token),
                    ("srcDecimals", Self::decimals(&from.symbol).to_string()),
                    ("destDecimals", Self::decimals(&to.symbol).to_string()),
                    ("amount", atomic_amount.to_string()),
                    ("side", "SELL".to_string()),
                    ("network", chain_id.to_string()),
                    ("userAddress", self.quote_address.clone()),
                ])
                .header("accept", "application/json")
                .send()
                .await
                .context("request Velora quote")?,
        };
        ensure_success(response.status(), &format!("request {} quote", self.name))?;
        response
            .json::<Value>()
            .await
            .with_context(|| format!("decode {} quote", self.name))
    }

    fn output_atomic<'a>(&self, raw: &'a Value) -> Result<&'a str> {
        match self.kind {
            DexKind::Lifi => raw
                .pointer("/estimate/toAmount")
                .and_then(Value::as_str)
                .context("LI.FI quote has no estimate.toAmount"),
            DexKind::KyberSwap => raw
                .pointer("/data/routeSummary/amountOut")
                .and_then(Value::as_str)
                .context("KyberSwap quote has no data.routeSummary.amountOut"),
            DexKind::Nordstern => raw
                .get("toAmount")
                .and_then(Value::as_str)
                .context("Nordstern quote has no toAmount"),
            DexKind::Velora => raw
                .pointer("/priceRoute/destAmount")
                .and_then(Value::as_str)
                .context("Velora quote has no priceRoute.destAmount"),
        }
    }
}

#[async_trait]
impl PublicRouteProvider for DexAggregatorRouteProvider {
    fn name(&self) -> &str {
        self.name
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        let mut assets = self
            .tokens
            .keys()
            .filter_map(|(network, symbol)| Asset::new(symbol, Some(network)).ok())
            .collect::<Vec<_>>();
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if from == to || amount.asset != from {
            bail!(
                "{} quote requires a distinct pair and matching input amount",
                self.name
            );
        }
        let atomic_amount = decimal_to_atomic(&amount.value, Some(Self::decimals(&from.symbol)))?;
        let raw = self.request_quote(&from, &to, &atomic_amount).await?;
        let output = Amount::new(
            atomic_to_decimal(self.output_atomic(&raw)?, Self::decimals(&to.symbol))?,
            to.clone(),
        )?;
        let source_url = match self.kind {
            DexKind::Lifi => "https://jumper.exchange/",
            DexKind::KyberSwap => "https://kyberswap.com/",
            DexKind::Nordstern => "https://nordstern.finance/",
            DexKind::Velora => "https://app.velora.xyz/",
        };
        Ok(PublicRouteQuote {
            provider: self.name.to_string(),
            quote_id: raw
                .get("id")
                .or_else(|| raw.pointer("/data/routeSummary/routeID"))
                .and_then(Value::as_str)
                .map(str::to_string),
            description: Some(
                "Indicative provider quote; verify live details before wallet signing".into(),
            ),
            source_url: Some(source_url.into()),
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output,
            fees: Vec::new(),
            expires_at: Some(Utc::now() + chrono::Duration::from_std(DEFAULT_QUOTE_TTL)?),
            path: vec![from, to],
        })
    }
}

fn is_evm_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_documented_quote_response_shapes() {
        let cases = [
            ("lifi", r#"{"estimate":{"toAmount":"1200000"}}"#, "1200000"),
            (
                "kyberswap",
                r#"{"data":{"routeSummary":{"amountOut":"1200000"}}}"#,
                "1200000",
            ),
            ("nordstern", r#"{"toAmount":"1200000"}"#, "1200000"),
            (
                "velora",
                r#"{"priceRoute":{"destAmount":"1200000"}}"#,
                "1200000",
            ),
        ];
        for (name, body, expected) in cases {
            let provider = DexAggregatorRouteProvider::from_config(
                name,
                &["base:USDC=0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".into()],
                "0x0000000000000000000000000000000000000001",
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                provider
                    .output_atomic(&serde_json::from_str(body).unwrap())
                    .unwrap(),
                expected
            );
        }
    }
}

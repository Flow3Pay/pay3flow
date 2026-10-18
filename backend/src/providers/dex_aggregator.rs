use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::token_catalog::TokenCatalog;
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
    ZeroX,
    Barter,
}

#[derive(Clone)]
struct DexToken {
    address: String,
    decimals: u8,
}

#[derive(Clone)]
pub struct DexAggregatorRouteProvider {
    kind: DexKind,
    name: &'static str,
    client: Client,
    tokens: Arc<HashMap<(String, String), DexToken>>,
    quote_address: String,
    api_key: Option<String>,
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
            "0x" => DexKind::ZeroX,
            "barter" => DexKind::Barter,
            _ => bail!("unsupported public DEX quote provider {name}"),
        };
        let tokens = token_entries
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .filter_map(|(key, address)| {
                let (chain, symbol) = key.split_once(':')?;
                let chain = canonical_network_id(chain);
                let mut symbol = symbol.trim().to_ascii_uppercase();
                let mut address = address.trim().to_string();
                if symbol == "ETH"
                    && kind == DexKind::ZeroX
                    && matches!(
                        chain.as_str(),
                        "ethereum" | "optimism" | "arbitrum-one" | "base"
                    )
                {
                    address = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".into();
                } else if symbol == "ETH" && kind == DexKind::Barter {
                    // The shared CoW configuration maps ETH to wrapped Ether.
                    symbol = "WETH".into();
                }
                let asset = Asset::new(&symbol, Some(&chain)).ok()?;
                is_evm_address(&address).then_some((
                    (chain, symbol),
                    DexToken {
                        address,
                        decimals: Self::fallback_decimals(&asset),
                    },
                ))
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
                DexKind::ZeroX => "0x",
                DexKind::Barter => "barter",
            },
            client,
            tokens: Arc::new(tokens),
            quote_address: quote_address.to_string(),
            api_key: None,
        }))
    }

    pub fn with_api_key(mut self, key: String) -> Self {
        self.api_key = Some(key);
        self
    }

    fn supports_network(&self, network: &str) -> bool {
        if self.kind == DexKind::Barter {
            return network == "ethereum";
        }
        if self.kind == DexKind::ZeroX && network == "gnosis" {
            return false;
        }
        Self::chain_id(network).is_ok()
    }

    fn fallback_decimals(asset: &Asset) -> u8 {
        let symbol = asset.symbol.as_str();
        if asset.location.as_deref() == Some("bnb-smart-chain")
            && matches!(symbol, "USDT" | "USDC" | "FDUSD")
        {
            return 18;
        }
        match symbol.to_ascii_uppercase().as_str() {
            "USDC" | "USDT" => 6,
            "WBTC" | "BTCB" => 8,
            _ => 18,
        }
    }

    /// Load token addresses and precision from the shared public token list.
    pub async fn with_token_catalog(mut self, url: &str) -> Self {
        let result = async {
            self.client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json::<TokenCatalog>()
                .await
        }
        .await;
        match result {
            Ok(catalog) => {
                let mut tokens = (*self.tokens).clone();
                for token in catalog
                    .tokens()
                    .into_iter()
                    .filter(|token| token.is_erc20())
                {
                    let network = match token.chain_id {
                        1 => "ethereum",
                        56 => "bnb-smart-chain",
                        100 => "gnosis",
                        137 => "polygon-pos",
                        10 => "optimism",
                        42161 => "arbitrum-one",
                        43114 => "avalanche-c",
                        8453 => "base",
                        _ => continue,
                    };
                    if !self.supports_network(network) || !is_evm_address(&token.address) {
                        continue;
                    }
                    let symbol = token.symbol.to_ascii_uppercase();
                    if Asset::new(&symbol, Some(network)).is_err() {
                        continue;
                    }
                    tokens.insert(
                        (network.into(), symbol.clone()),
                        DexToken {
                            address: token.address,
                            decimals: token.decimals,
                        },
                    );
                    if self.kind == DexKind::ZeroX
                        && symbol == "WETH"
                        && matches!(network, "ethereum" | "optimism" | "arbitrum-one" | "base")
                    {
                        tokens.insert(
                            (network.into(), "ETH".into()),
                            DexToken {
                                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".into(),
                                decimals: 18,
                            },
                        );
                    }
                }
                self.tokens = Arc::new(tokens);
            }
            Err(error) => {
                tracing::warn!(%error, provider = self.name, "DEX token catalog unavailable; retaining configured metadata")
            }
        }
        self
    }

    fn decimals(&self, asset: &Asset) -> Result<u8> {
        let network = asset
            .location
            .as_deref()
            .context("token precision requires a network")?;
        self.tokens
            .get(&(
                canonical_network_id(network),
                asset.symbol.to_ascii_uppercase(),
            ))
            .map(|token| token.decimals)
            .context("token precision is not configured")
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
            .map(|token| token.address.clone())
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
            DexKind::ZeroX => self.client
                .get("https://api.0x.org/swap/allowance-holder/price")
                .header("0x-api-key", self.api_key.as_deref().context("0x requires ZEROX_API_KEY")?)
                .header("0x-version", "v2")
                .query(&[("chainId", chain_id.to_string()), ("sellToken", from_token),
                    ("buyToken", to_token), ("sellAmount", atomic_amount.to_string())])
                .send().await.context("request 0x price")?,
            DexKind::Barter => self.client
                .post("https://api2.eth.barterswap.xyz/route")
                .bearer_auth(self.api_key.as_deref().context("Barter requires BARTER_API_KEY")?)
                .json(&serde_json::json!({"source": from_token, "target": to_token, "sellAmount": atomic_amount}))
                .send().await.context("request Barter route")?,
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
                    ("srcDecimals", self.decimals(from)?.to_string()),
                    ("destDecimals", self.decimals(to)?.to_string()),
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
            DexKind::ZeroX => {
                if raw.get("liquidityAvailable").and_then(Value::as_bool) != Some(true) {
                    bail!("0x has no liquidity for this pair and amount");
                }
                raw.get("buyAmount")
                    .and_then(Value::as_str)
                    .context("0x price has no buyAmount")
            }
            DexKind::Barter => {
                if raw.get("status").and_then(Value::as_str) != Some("Normal") {
                    bail!("Barter route is unavailable");
                }
                raw.get("targetAmount")
                    .and_then(Value::as_str)
                    .context("Barter route has no targetAmount")
            }
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
            .filter(|(network, _)| self.supports_network(network))
            .filter_map(|(network, symbol)| Asset::new(symbol, Some(network)).ok())
            .collect::<Vec<_>>();
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        from != to
            && from.location == to.location
            && from
                .location
                .as_deref()
                .is_some_and(|network| self.supports_network(network))
            && self.token_address(from).is_ok()
            && self.token_address(to).is_ok()
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if !self.supports_pair(&from, &to) || amount.asset != from {
            bail!(
                "{} quote requires a distinct pair and matching input amount",
                self.name
            );
        }
        let atomic_amount = decimal_to_atomic(&amount.value, Some(self.decimals(&from)?))?;
        let raw = self.request_quote(&from, &to, &atomic_amount).await?;
        let output = Amount::new(
            atomic_to_decimal(self.output_atomic(&raw)?, self.decimals(&to)?)?,
            to.clone(),
        )?;
        let source_url = match self.kind {
            DexKind::Lifi => "https://jumper.exchange/",
            DexKind::KyberSwap => "https://kyberswap.com/",
            DexKind::Nordstern => "https://nordstern.finance/",
            DexKind::Velora => "https://app.velora.xyz/",
            DexKind::ZeroX => "https://matcha.xyz/",
            DexKind::Barter => "https://app.barterswap.xyz/",
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

#[cfg(test)]
mod added_provider_tests {
    use super::*;

    fn provider(name: &str) -> DexAggregatorRouteProvider {
        DexAggregatorRouteProvider::from_config(
            name,
            &[
                "ethereum:USDC=0xA0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
                "ethereum:ETH=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
                "base:USDC=0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".into(),
            ],
            "0x0000000000000000000000000000000000000001",
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn rejects_unavailable_quotes_from_zero_x_and_barter() {
        for (name, valid, invalid) in [
            (
                "0x",
                serde_json::json!({"liquidityAvailable": true, "buyAmount": "98000000"}),
                serde_json::json!({"liquidityAvailable": false, "buyAmount": "98000000"}),
            ),
            (
                "barter",
                serde_json::json!({"status": "Normal", "targetAmount": "98000000"}),
                serde_json::json!({"status": "SwapError", "targetAmount": "98000000"}),
            ),
        ] {
            let provider = provider(name);
            assert_eq!(
                provider.output_atomic(&valid).unwrap(),
                "98000000",
                "{name}"
            );
            assert!(
                provider.output_atomic(&invalid).is_err(),
                "{name} must reject failed routes"
            );
        }
    }

    #[tokio::test]
    async fn new_dex_providers_do_not_claim_cross_chain_or_wrapped_native_swaps() {
        let zero_x = provider("0x");
        let eth = Asset::new("ETH", Some("ethereum")).unwrap();
        let usdc = Asset::new("USDC", Some("ethereum")).unwrap();
        assert_eq!(
            zero_x.token_address(&eth).unwrap(),
            "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"
        );
        assert!(zero_x.supports_pair(&eth, &usdc));
        assert!(!zero_x.supports_pair(&usdc, &Asset::new("USDC", Some("base")).unwrap()));
        let barter = provider("barter");
        let assets = barter.supported_assets().await;
        assert!(assets.contains(&Asset::new("WETH", Some("ethereum")).unwrap()));
        assert!(!assets.contains(&eth));
        assert!(assets
            .iter()
            .all(|asset| asset.location.as_deref() == Some("ethereum")));
    }

    #[test]
    fn stablecoin_precision_depends_on_the_network() {
        for (symbol, network, decimals) in [
            ("USDT", "ethereum", 6),
            ("USDT", "bnb-smart-chain", 18),
            ("USDC", "bnb-smart-chain", 18),
            ("USDE", "ethereum", 18),
        ] {
            assert_eq!(
                DexAggregatorRouteProvider::fallback_decimals(
                    &Asset::new(symbol, Some(network)).unwrap()
                ),
                decimals
            );
        }
    }
}

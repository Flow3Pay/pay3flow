use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};

use crate::route_engine::{
    atomic_to_decimal, canonical_network_id, decimal_to_atomic, ensure_success, first_string,
    truncate_decimal, Amount, Asset, CowExecutionQuote, NearIntentsProvider, PublicRouteProvider,
    PublicRouteQuote, DEFAULT_QUOTE_TTL,
};

const EVM_NATIVE: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

#[derive(Clone)]
struct CowToken {
    address: String,
    decimals: u8,
}

#[derive(Deserialize)]
struct CowTokenList {
    tokens: Vec<ListToken>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListToken {
    chain_id: u64,
    symbol: String,
    address: String,
    decimals: u8,
}

struct CowChain {
    network: &'static str,
    id: u64,
    api: Option<&'static str>,
    native_symbol: &'static str,
    wrapped_symbol: &'static str,
    bridge_destination: bool,
}

impl CowChain {
    fn is_evm(&self) -> bool {
        self.id < 1_000_000_000
    }
    fn native_address(&self) -> &'static str {
        match self.id {
            1_000_000_000 => "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            1_000_000_001 => "11111111111111111111111111111111",
            _ => EVM_NATIVE,
        }
    }

    fn pricing_address<'a>(&self, address: &'a str) -> &'a str {
        // Solana's quote API prices native currency using the wrapped SPL mint.
        if self.id == 1_000_000_001 && address == self.native_address() {
            "So11111111111111111111111111111111111111112"
        } else {
            address
        }
    }
}

// Network identifiers and API paths from CoW SDK's chain/order-book registry;
// receiver-account bridge destinations from its NEAR Intents provider.
const CHAINS: &[CowChain] = &[
    CowChain {
        network: "ethereum",
        id: 1,
        api: Some("mainnet"),
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: true,
    },
    CowChain {
        network: "gnosis",
        id: 100,
        api: Some("xdai"),
        native_symbol: "XDAI",
        wrapped_symbol: "WXDAI",
        bridge_destination: true,
    },
    CowChain {
        network: "arbitrum-one",
        id: 42161,
        api: Some("arbitrum_one"),
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: true,
    },
    CowChain {
        network: "base",
        id: 8453,
        api: Some("base"),
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: true,
    },
    CowChain {
        network: "polygon-pos",
        id: 137,
        api: Some("polygon"),
        native_symbol: "POL",
        wrapped_symbol: "WPOL",
        bridge_destination: true,
    },
    CowChain {
        network: "avalanche-c",
        id: 43114,
        api: Some("avalanche"),
        native_symbol: "AVAX",
        wrapped_symbol: "WAVAX",
        bridge_destination: true,
    },
    CowChain {
        network: "bnb-smart-chain",
        id: 56,
        api: Some("bnb"),
        native_symbol: "BNB",
        wrapped_symbol: "WBNB",
        bridge_destination: true,
    },
    CowChain {
        network: "linea",
        id: 59144,
        api: Some("linea"),
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: false,
    },
    CowChain {
        network: "plasma",
        id: 9745,
        api: Some("plasma"),
        native_symbol: "XPL",
        wrapped_symbol: "WXPL",
        bridge_destination: true,
    },
    CowChain {
        network: "ink",
        id: 57073,
        api: Some("ink"),
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: false,
    },
    CowChain {
        network: "optimism",
        id: 10,
        api: None,
        native_symbol: "ETH",
        wrapped_symbol: "WETH",
        bridge_destination: true,
    },
    CowChain {
        network: "bitcoin",
        id: 1000000000,
        api: None,
        native_symbol: "BTC",
        wrapped_symbol: "WBTC",
        bridge_destination: true,
    },
    CowChain {
        network: "solana",
        id: 1000000001,
        api: Some("solana"),
        native_symbol: "SOL",
        wrapped_symbol: "WSOL",
        bridge_destination: true,
    },
];

#[derive(Clone)]
pub struct CowRouteProvider {
    client: Client,
    endpoints: Arc<HashMap<String, String>>,
    configured_tokens: Arc<HashMap<(String, String), String>>,
    catalog: Arc<RwLock<HashMap<Asset, CowToken>>>,
    catalog_url: Option<String>,
    catalog_refresh: Arc<Mutex<Option<Instant>>>,
    quote_address: String,
    bridge: Option<NearIntentsProvider>,
}

impl CowRouteProvider {
    pub fn from_config(
        api_urls: &[String],
        token_entries: &[String],
        quote_address: Option<&str>,
    ) -> Result<Option<Self>> {
        let Some(quote_address) = quote_address
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let endpoints = api_urls
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .map(|(chain, url)| (canonical_network_id(chain), url.trim().to_string()))
            .filter(|(chain, url)| !chain.is_empty() && !url.is_empty())
            .collect::<HashMap<_, _>>();
        let tokens = token_entries
            .iter()
            .filter_map(|entry| entry.split_once('='))
            .filter_map(|(key, address)| {
                key.split_once(':')
                    .map(|(chain, symbol)| (chain, symbol, address))
            })
            .map(|(chain, symbol, address)| {
                (
                    (
                        canonical_network_id(chain),
                        symbol.trim().to_ascii_uppercase(),
                    ),
                    address.trim().to_string(),
                )
            })
            .filter(|((chain, symbol), address)| {
                !chain.is_empty() && !symbol.is_empty() && !address.is_empty()
            })
            .collect::<HashMap<_, _>>();
        if endpoints.is_empty() {
            return Ok(None);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Pay3Flow-CoW-Protocol/0.1")
            .build()
            .context("failed to build CoW Protocol client")?;
        Ok(Some(Self {
            client,
            endpoints: Arc::new(endpoints),
            configured_tokens: Arc::new(tokens),
            catalog: Arc::new(RwLock::new(HashMap::new())),
            catalog_url: None,
            catalog_refresh: Arc::new(Mutex::new(None)),
            quote_address: quote_address.to_string(),
            bridge: None,
        }))
    }

    fn decimals(symbol: &str) -> u8 {
        match symbol.to_ascii_uppercase().as_str() {
            "USDC" | "USDT" | "FDUSD" => 6,
            _ => 18,
        }
    }

    fn quote_url(base: &str) -> String {
        let base = base.trim_end_matches('/');
        if base.ends_with("/api/v1/quote") {
            base.to_string()
        } else {
            format!("{base}/api/v1/quote")
        }
    }

    fn api_root(base: &str) -> String {
        Self::quote_url(base)
            .trim_end_matches("/quote")
            .trim_end_matches('/')
            .to_string()
    }

    fn chain_id(chain: &str) -> Result<u64> {
        CHAINS
            .iter()
            .find(|entry| entry.network == chain)
            .map(|entry| entry.id)
            .with_context(|| format!("unsupported CoW network {chain}"))
    }

    /// Enable live token discovery; configured addresses remain an offline fallback.
    pub fn with_token_catalog(mut self, url: impl Into<String>) -> Self {
        self.catalog_url = Some(url.into());
        // Custom/test deployments keep their configured endpoints. The public
        // service uses the protocol's network registry, with config overrides.
        if self
            .endpoints
            .values()
            .any(|url| url.starts_with("https://api.cow.fi/"))
        {
            let endpoints = Arc::make_mut(&mut self.endpoints);
            for chain in CHAINS {
                if let Some(api) = chain.api {
                    endpoints
                        .entry(chain.network.to_string())
                        .or_insert_with(|| format!("https://api.cow.fi/{api}"));
                }
            }
        }
        self
    }

    async fn tokens(&self) -> HashMap<Asset, CowToken> {
        if let Some(url) = &self.catalog_url {
            let mut refreshed = self.catalog_refresh.lock().await;
            if refreshed.is_none_or(|at| at.elapsed() >= Duration::from_secs(300)) {
                let result = async {
                    let response = self.client.get(url).send().await?.error_for_status()?;
                    let list = response.json::<CowTokenList>().await?;
                    Ok::<_, anyhow::Error>(
                        list.tokens
                            .into_iter()
                            .filter_map(|token| {
                                let network =
                                    CHAINS.iter().find(|chain| chain.id == token.chain_id)?;
                                let asset = Asset::new(token.symbol, Some(network.network)).ok()?;
                                Some((
                                    asset,
                                    CowToken {
                                        address: token.address,
                                        decimals: token.decimals,
                                    },
                                ))
                            })
                            .collect::<HashMap<_, _>>(),
                    )
                }
                .await;
                match result {
                    Ok(tokens) => *self.catalog.write().await = tokens,
                    Err(error) => {
                        tracing::warn!(%error, "CoW token catalog refresh failed; retaining previous metadata")
                    }
                }
                *refreshed = Some(Instant::now());
            }
        }
        let mut tokens = self
            .configured_tokens
            .iter()
            .filter_map(|((network, symbol), address)| {
                Some((
                    Asset::new(symbol, Some(network)).ok()?,
                    CowToken {
                        address: address.clone(),
                        decimals: Self::decimals(symbol),
                    },
                ))
            })
            .collect::<HashMap<_, _>>();
        tokens.extend(self.catalog.read().await.clone());
        if let Some(bridge) = &self.bridge {
            // The bridge catalog supplies native currencies and additional
            // tokens, including their actual per-network precision.
            bridge.supported_assets().await;
            for token in bridge.supported_tokens().await {
                let Ok(asset) = token.asset() else { continue };
                let Some(chain) = CHAINS
                    .iter()
                    .find(|chain| Some(chain.network) == asset.location.as_deref())
                else {
                    continue;
                };
                let Some(decimals) = token.decimals else {
                    continue;
                };
                let address = match token.contract_address.as_deref() {
                    Some(address) if address != "coin" => address,
                    _ => chain.native_address(),
                };
                tokens
                    .entry(asset)
                    .and_modify(|entry| {
                        if entry.address.eq_ignore_ascii_case(address)
                            || address == chain.native_address()
                        {
                            entry.address = address.to_string();
                            entry.decimals = decimals;
                        }
                    })
                    .or_insert_with(|| CowToken {
                        address: address.to_string(),
                        decimals,
                    });
            }
        }
        tokens
    }

    async fn token(&self, asset: &Asset) -> Result<CowToken> {
        self.tokens()
            .await
            .remove(asset)
            .with_context(|| format!("CoW token metadata is unavailable for {asset}"))
    }

    pub fn supports_wallet_execution(from: &Asset, to: &Asset) -> bool {
        from.location == to.location
            && CHAINS.iter().any(|chain| {
                Some(chain.network) == from.location.as_deref()
                    && chain.is_evm()
                    && from.symbol != chain.native_symbol
            })
    }

    pub async fn execution_quote(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
        owner: &str,
        receiver: &str,
    ) -> Result<CowExecutionQuote> {
        let chain = from
            .location
            .clone()
            .context("CoW origin chain is required")?;
        if to.location.as_deref() != Some(chain.as_str()) {
            bail!("CoW execution must stay on one chain");
        }
        let endpoint = self
            .endpoints
            .get(&chain)
            .with_context(|| format!("CoW endpoint is not configured for {chain}"))?;
        if !Self::supports_wallet_execution(&from, &to) {
            bail!("embedded CoW execution requires an ERC20 origin on one EVM network");
        }
        let sell = self.token(&from).await?;
        let buy = self.token(&to).await?;
        let sell_token = sell.address;
        let buy_token = buy.address;
        let sell_amount = decimal_to_atomic(&amount.value, Some(sell.decimals))?;
        let expires_at = Utc::now() + chrono::Duration::from_std(DEFAULT_QUOTE_TTL)?;
        let body = json!({
            "sellToken": sell_token,
            "buyToken": buy_token,
            "sellAmountBeforeFee": sell_amount,
            "from": owner,
            "receiver": receiver,
            "kind": "sell",
            "partiallyFillable": false,
            "validTo": expires_at.timestamp(),
        });
        let response = self
            .client
            .post(Self::quote_url(endpoint))
            .json(&body)
            .send()
            .await
            .context("request executable CoW quote")?;
        ensure_success(response.status(), "request executable CoW quote")?;
        let quote = response
            .json::<Value>()
            .await
            .context("decode executable CoW quote")?;
        let buy_amount = quote
            .pointer("/quote/buyAmount")
            .or_else(|| quote.get("buyAmount"))
            .and_then(Value::as_str)
            .context("executable CoW quote did not include buyAmount")?;
        let expected_output = atomic_to_decimal(buy_amount, buy.decimals)?;
        let fee_amount = quote
            .pointer("/quote/feeAmount")
            .or_else(|| quote.get("feeAmount"))
            .and_then(Value::as_str)
            .map(|amount| atomic_to_decimal(amount, sell.decimals))
            .transpose()?;
        Ok(CowExecutionQuote {
            chain: chain.clone(),
            chain_id: Self::chain_id(&chain)?,
            api_url: Self::api_root(endpoint),
            sell_token,
            buy_token,
            sell_amount,
            sell_token_decimals: sell.decimals,
            buy_token_decimals: buy.decimals,
            expected_output,
            expected_fee: fee_amount,
            quote,
            expires_at,
        })
    }

    /// Enable read-only quotes through CoW's receiver-account bridge.
    pub fn with_bridge(mut self, bridge: NearIntentsProvider) -> Self {
        self.bridge = Some(bridge);
        self
    }

    async fn quote_cross_chain(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<PublicRouteQuote> {
        if !self.supports_pair(&from, &to) {
            bail!("CoW does not support this origin/destination network direction");
        }
        let chain = from
            .location
            .as_deref()
            .context("CoW origin chain is required")?;
        let target = CHAINS
            .iter()
            .find(|entry| Some(entry.network) == to.location.as_deref())
            .context("unsupported CoW destination network")?;
        let bridge = self
            .bridge
            .as_ref()
            .context("CoW bridge is not configured")?;
        let tokens = self.tokens().await;
        let destination = tokens
            .get(&to)
            .context("unsupported CoW destination token")?;
        let bridge_tokens = bridge.supported_tokens().await;
        if !bridge_tokens.iter().any(|token| token.matches(&to)) {
            bail!("CoW bridge destination is unsupported: {to}");
        }
        let mut intermediaries = bridge_tokens
            .iter()
            .filter_map(|token| {
                let asset = token.asset().ok()?;
                if asset.location.as_deref() != Some(chain) {
                    return None;
                }
                let cow_token = tokens.get(&asset)?;
                let source_chain = CHAINS.iter().find(|entry| entry.network == chain)?;
                let bridge_address = token
                    .contract_address
                    .as_deref()
                    .unwrap_or(source_chain.native_address());
                cow_token
                    .address
                    .eq_ignore_ascii_case(bridge_address)
                    .then_some(asset)
            })
            .collect::<Vec<_>>();
        intermediaries.sort_by_key(|asset| {
            (
                if asset.symbol == to.symbol {
                    0
                } else if asset == &from {
                    1
                } else if matches!(asset.symbol.as_str(), "USDC" | "USDT") {
                    2
                } else {
                    3
                },
                asset.to_string(),
            )
        });
        intermediaries.dedup();
        // EVM preview addresses are reusable across networks. Non-EVM
        // recipients must come from the bridge's network address settings.
        let recipient = bridge
            .quote_recipients
            .get(target.network)
            .cloned()
            .or_else(|| target.is_evm().then(|| self.quote_address.clone()))
            .or_else(|| bridge.quote_recipient.clone())
            .context("CoW bridge destination preview address is required")?;
        let refund = bridge
            .quote_refunds
            .get(chain)
            .cloned()
            .unwrap_or_else(|| self.quote_address.clone());
        let bridge = bridge.clone().with_quote_addresses(recipient, refund)?;
        let mut last_error = None;
        for intermediate in intermediaries.into_iter().take(3) {
            let result = async {
                let swap = self
                    .quote_on_chain(from.clone(), intermediate.clone(), amount.clone())
                    .await?;
                let bridged = bridge
                    .quote(intermediate.clone(), to.clone(), swap.output)
                    .await?;
                let mut fees = swap.fees;
                fees.extend(bridged.fees);
                let expires_at = match (swap.expires_at, bridged.expires_at) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                let mut path = vec![from.clone()];
                if intermediate != from {
                    path.push(intermediate);
                }
                path.push(to.clone());
                let sell = tokens.get(&from).context("unsupported CoW sell token")?;
                Ok(PublicRouteQuote {
                    provider: self.name().to_string(),
                    quote_id: None,
                    description: Some("CoW Swap via NEAR Intents bridge (indicative quote)".into()),
                    source_url: Some(format!(
                        "https://swap.cow.fi/#/{}/swap/{}/{}?targetChainId={}",
                        Self::chain_id(chain)?,
                        sell.address,
                        destination.address,
                        target.id,
                    )),
                    from: from.clone(),
                    to: to.clone(),
                    input: swap.input,
                    output: bridged.output,
                    fees,
                    expires_at,
                    path,
                })
            }
            .await;
            match result {
                Ok(quote) => return Ok(quote),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("CoW has no bridge intermediate token for {from} -> {to}")
        }))
    }

    async fn quote_on_chain(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<PublicRouteQuote> {
        let from_chain = from
            .location
            .clone()
            .context("CoW origin chain is required")?;
        let to_chain = to
            .location
            .clone()
            .context("CoW destination chain is required")?;
        if from_chain != to_chain {
            bail!("CoW Protocol does not provide a cross-chain quote");
        }
        let chain = from_chain.to_ascii_lowercase();
        let endpoint = self
            .endpoints
            .get(&chain)
            .with_context(|| format!("CoW endpoint is not configured for {chain}"))?;
        let sell = self.token(&from).await?;
        let buy = self.token(&to).await?;
        let amount = Amount::new(
            truncate_decimal(&amount.value, Some(sell.decimals))?,
            from.clone(),
        )?;
        let sell_amount = decimal_to_atomic(&amount.value, Some(sell.decimals))?;
        if sell_amount == "0" {
            bail!("CoW input amount is below token precision");
        }
        let network = CHAINS
            .iter()
            .find(|entry| entry.network == chain)
            .context("unsupported CoW origin network")?;
        let native_sell =
            network.is_evm() && sell.address.eq_ignore_ascii_case(network.native_address());
        let sell_token = if native_sell {
            self.tokens()
                .await
                .get(&Asset::new(network.wrapped_symbol, Some(&chain))?)
                .context("CoW wrapped native token metadata is unavailable")?
                .address
                .clone()
        } else {
            network.pricing_address(&sell.address).to_string()
        };
        let buy_token = network.pricing_address(&buy.address);
        let owner = if network.is_evm() {
            self.quote_address.clone()
        } else {
            let bridge = self
                .bridge
                .as_ref()
                .context("CoW non-EVM quote address is required")?;
            NearIntentsProvider::preview_address(
                &bridge.quote_refunds,
                bridge.quote_refund_to.as_deref(),
                &from,
                "owner",
            )?
        };
        let valid_to = (Utc::now() + chrono::Duration::from_std(DEFAULT_QUOTE_TTL)?).timestamp();
        let mut body = json!({
            "sellToken": sell_token,
            "buyToken": buy_token,
            "sellAmountBeforeFee": sell_amount,
            "from": owner,
            "receiver": owner,
            "kind": "sell",
            "partiallyFillable": false,
            "validTo": valid_to,
        });
        if native_sell {
            body["signingScheme"] = json!("eip1271");
            body["onchainOrder"] = json!(true);
            body["verificationGasLimit"] = json!(0);
        } else if !network.is_evm() {
            body["signingScheme"] = json!("eip712");
            body["appData"] = json!("{}");
        }
        let response = self
            .client
            .post(Self::quote_url(endpoint))
            .json(&body)
            .send()
            .await
            .context("request CoW Protocol quote")?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect::<String>();
            bail!("request CoW Protocol quote failed with HTTP {status}: {detail}");
        }
        let raw = response
            .json::<Value>()
            .await
            .context("decode CoW Protocol quote")?;
        let quote = raw.get("quote").unwrap_or(&raw);
        let buy_amount = first_string(quote, &["buyAmount", "buy_amount"])
            .context("CoW quote did not include buyAmount")?;
        let fee_amount = first_string(quote, &["feeAmount", "fee_amount", "surplusFeeAmount"]);
        let output = Amount::new(atomic_to_decimal(&buy_amount, buy.decimals)?, to.clone())?;
        let fee = fee_amount
            .map(|fee| Amount::new(atomic_to_decimal(&fee, sell.decimals)?, from.clone()))
            .transpose()?;
        Ok(PublicRouteQuote {
            provider: self.name().to_string(),
            quote_id: None,
            description: None,
            source_url: Some(format!(
                "https://swap.cow.fi/#/{}/swap/{}/{}",
                Self::chain_id(&chain)?,
                sell.address,
                buy.address,
            )),
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output,
            fees: fee.into_iter().collect(),
            expires_at: raw
                .get("expiration")
                .and_then(Value::as_str)
                .and_then(|expiry| DateTime::parse_from_rfc3339(expiry).ok())
                .map(|expiry| expiry.with_timezone(&Utc))
                .into_iter()
                .chain(DateTime::from_timestamp(valid_to, 0))
                .min(),
            path: vec![from, to],
        })
    }

    pub async fn order_status(&self, chain: &str, order_uid: &str) -> Result<Value> {
        let endpoint = self
            .endpoints
            .get(chain)
            .with_context(|| format!("CoW endpoint is not configured for {chain}"))?;
        let response = self
            .client
            .get(format!("{}/orders/{order_uid}", Self::api_root(endpoint)))
            .send()
            .await
            .context("request CoW order status")?;
        ensure_success(response.status(), "request CoW order status")?;
        response.json().await.context("decode CoW order status")
    }
}

#[async_trait]
impl PublicRouteProvider for CowRouteProvider {
    fn name(&self) -> &str {
        "cow-swap"
    }

    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        let Some(origin) = from.location.as_deref() else {
            return false;
        };
        let Some(destination) = to.location.as_deref() else {
            return false;
        };
        if !self.endpoints.contains_key(origin) {
            return false;
        }
        origin == destination
            || (self.bridge.is_some()
                && CHAINS
                    .iter()
                    .any(|entry| entry.network == origin && entry.is_evm())
                && CHAINS
                    .iter()
                    .any(|entry| entry.network == destination && entry.bridge_destination))
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        let mut assets = self
            .tokens()
            .await
            .into_keys()
            .filter(|asset| {
                asset.location.as_deref().is_some_and(|network| {
                    self.endpoints.contains_key(network)
                        || (self.bridge.is_some()
                            && CHAINS
                                .iter()
                                .any(|entry| entry.network == network && entry.bridge_destination))
                })
            })
            .collect::<Vec<_>>();
        assets.sort_by_key(ToString::to_string);
        assets
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if amount.asset != from {
            bail!("CoW quote input amount must use the origin asset");
        }
        if from.location != to.location {
            return self.quote_cross_chain(from, to, amount).await;
        }
        self.quote_on_chain(from, to, amount).await
    }
}

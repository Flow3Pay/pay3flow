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
    atomic_to_decimal, canonical_network_id, decimal_to_atomic, ensure_success, truncate_decimal,
    Amount, Asset, PublicRouteProvider, PublicRouteQuote, QuoteUnavailable,
    SymbiosisExecutionQuote,
};

const DEFAULT_API_URL: &str = "https://api.symbiosis.finance/crosschain";
const CALLDATA_TTL: chrono::Duration = chrono::Duration::seconds(30);

fn normalized_atomic_input(value: &str, decimals: u8) -> Result<(String, String)> {
    let normalized = truncate_decimal(value, Some(decimals))?;
    let atomic = decimal_to_atomic(&normalized, Some(decimals))?;
    Ok((normalized, atomic))
}

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
    execution_contracts: Arc<HashMap<u64, (String, String)>>,
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
            execution_contracts: Arc::new(HashMap::new()),
        })
    }

    /// Configure audited `chain_id=meta_router,meta_router_gateway` pairs.
    /// Executable calldata is rejected unless both addresses match this list.
    pub fn with_execution_contracts(mut self, entries: &[String]) -> Result<Self> {
        let mut contracts = HashMap::new();
        for entry in entries {
            let (chain_id, addresses) = entry
                .split_once('=')
                .with_context(|| format!("invalid Symbiosis execution contract entry {entry}"))?;
            let (router, gateway) = addresses
                .split_once(',')
                .with_context(|| format!("invalid Symbiosis execution contract entry {entry}"))?;
            let chain_id = chain_id
                .trim()
                .parse::<u64>()
                .with_context(|| format!("invalid Symbiosis chain id in {entry}"))?;
            let router = checked_evm_address(router, "meta router")?;
            let gateway = checked_evm_address(gateway, "meta router gateway")?;
            if contracts.insert(chain_id, (router, gateway)).is_some() {
                bail!("duplicate Symbiosis execution contracts for chain {chain_id}");
            }
        }
        self.execution_contracts = Arc::new(contracts);
        Ok(self)
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

    fn token_for<'a>(tokens: &'a [SymbiosisToken], asset: &Asset) -> Result<&'a SymbiosisToken> {
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

    fn parse_fee(value: &Value, tokens: &[SymbiosisToken]) -> Result<Option<Amount>> {
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
                    .find(|token| {
                        token.chain_id == chain_id && token.symbol.eq_ignore_ascii_case(symbol)
                    })
                    .map(|token| token.decimals)
            });
        let Some(network) = network_for_chain(chain_id) else {
            return Ok(None);
        };
        let asset = Asset::new(symbol, Some(network))?;
        let amount =
            atomic_to_decimal(amount, decimals.context("Symbiosis fee decimals missing")?)?;
        Ok(Some(Amount::new(amount, asset)?))
    }

    fn quote_expiry(raw: &Value) -> Option<DateTime<Utc>> {
        raw.pointer("/tx/validUntil")
            .and_then(Value::as_i64)
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
            .or_else(|| Some(Utc::now() + CALLDATA_TTL))
    }

    pub async fn execution_quote(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
        owner: &str,
        recipient: &str,
    ) -> Result<SymbiosisExecutionQuote> {
        if from == to || amount.asset != from {
            bail!("invalid Symbiosis execution pair or amount");
        }
        let tokens = self.load_tokens().await?;
        let from_token = Self::token_for(&tokens, &from)?.clone();
        let to_token = Self::token_for(&tokens, &to)?.clone();
        let (_, atomic_amount) = normalized_atomic_input(&amount.value, from_token.decimals)?;
        let body = json!({
            "tokenAmountIn": Self::token_payload(&from_token, Some(atomic_amount.clone())),
            "tokenOut": Self::token_payload(&to_token, None),
            "from": owner,
            "to": recipient,
            "slippage": self.slippage_bps,
        });
        let mut request = self.client.post(self.endpoint("v1/swap")).json(&body);
        if let Some(partner_id) = &self.partner_id {
            request = request.header("X-Partner-Id", partner_id);
        }
        let response = request
            .send()
            .await
            .context("request executable Symbiosis quote")?;
        let status = response.status();
        let raw = response
            .json::<Value>()
            .await
            .context("decode executable Symbiosis quote")?;
        if !status.is_success() {
            bail!("executable Symbiosis quote returned HTTP {status}: {raw}");
        }
        let transaction = raw
            .get("tx")
            .cloned()
            .context("Symbiosis executable quote has no transaction")?;
        let output = raw
            .get("tokenAmountOut")
            .context("Symbiosis executable quote has no output amount")?;
        let output_amount = output
            .get("amount")
            .and_then(Value::as_str)
            .context("Symbiosis executable quote has no output amount")?;
        let output_decimals = output
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .unwrap_or(to_token.decimals);
        let expected_output = atomic_to_decimal(output_amount, output_decimals)?;
        let approval_spender = raw
            .get("approveTo")
            .or_else(|| raw.get("approvalAddress"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let (expected_router, expected_gateway) = self
            .execution_contracts
            .get(&from_token.chain_id)
            .with_context(|| {
                format!(
                    "Symbiosis execution contracts are not configured for chain {}",
                    from_token.chain_id
                )
            })?;
        let transaction_to = transaction
            .get("to")
            .and_then(Value::as_str)
            .context("Symbiosis executable transaction has no destination")?;
        if !transaction_to.eq_ignore_ascii_case(expected_router) {
            bail!("Symbiosis executable transaction destination is not the configured MetaRouter");
        }
        if !is_evm_native_token(&from_token.address) {
            let spender = approval_spender
                .as_deref()
                .context("Symbiosis ERC-20 quote has no approval spender")?;
            if !spender.eq_ignore_ascii_case(expected_gateway) {
                bail!("Symbiosis approval spender is not the configured MetaRouterGateway");
            }
        }
        Ok(SymbiosisExecutionQuote {
            source_chain_id: from_token.chain_id,
            source_token: from_token.address,
            input_amount: atomic_amount,
            expected_output,
            approval_spender,
            transaction,
            expires_at: Self::quote_expiry(&raw).unwrap_or_else(|| Utc::now() + CALLDATA_TTL),
            quote: raw,
        })
    }

    pub async fn transaction_status(&self, chain_id: u64, tx_hash: &str) -> Result<Value> {
        let response = self
            .client
            .get(self.endpoint(&format!("v1/tx/{chain_id}/{tx_hash}")))
            .send()
            .await
            .context("request Symbiosis transaction status")?;
        ensure_success(response.status(), "request Symbiosis transaction status")?;
        response
            .json()
            .await
            .context("decode Symbiosis transaction status")
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
        let (input_value, atomic_amount) =
            normalized_atomic_input(&amount.value, from_token.decimals)?;
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
            if status == reqwest::StatusCode::BAD_REQUEST
                && raw.get("message").and_then(Value::as_str) == Some("This swap is not available")
            {
                return Err(QuoteUnavailable {
                    provider: "symbiosis",
                    reason: format!(
                        "No swap available for {from} → {to} at {} {}",
                        amount.value, from.symbol
                    ),
                }
                .into());
            }
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
                    if !fees.iter().any(|existing: &Amount| {
                        existing.asset == fee.asset && existing.value == fee.value
                    }) {
                        fees.push(fee);
                    }
                }
            }
        }
        Ok(PublicRouteQuote {
            provider: self.name().into(),
            quote_id: Some(format!("symbiosis:{}>{}", from, to)),
            description: raw.get("kind").and_then(Value::as_str).map(str::to_string),
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: Amount::new(input_value, amount.asset)?,
            output,
            fees,
            expires_at: Self::quote_expiry(&raw),
            path: vec![from, to],
        })
    }
}

fn checked_evm_address(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.len() != 42
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("invalid Symbiosis {label} address");
    }
    Ok(value.to_string())
}

fn is_evm_native_token(address: &str) -> bool {
    address.eq_ignore_ascii_case("0x0000000000000000000000000000000000000000")
        || address.eq_ignore_ascii_case("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
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

    #[test]
    fn truncates_quote_input_to_the_source_token_precision() {
        let (normalized, atomic) = normalized_atomic_input("12.345678901", 6).unwrap();

        assert_eq!(normalized, "12.345678");
        assert_eq!(atomic, "12345678");
    }

    #[test]
    fn validates_and_indexes_execution_contract_allowlist() {
        let provider = SymbiosisRouteProvider::new(
            DEFAULT_API_URL,
            None,
            "0xf93d011544e89a28b5bdbdd833016cc5f26e82cd",
            300,
        )
        .unwrap()
        .with_execution_contracts(&[
            "1=0xf621Fb08BBE51aF70e7E0F4EA63496894166Ff7F,0xfCEF2Fe72413b65d3F393d278A714caD87512bcd".into(),
        ])
        .unwrap();
        assert_eq!(
            provider.execution_contracts.get(&1).unwrap().0,
            "0xf621Fb08BBE51aF70e7E0F4EA63496894166Ff7F"
        );

        assert!(
            SymbiosisRouteProvider::new(DEFAULT_API_URL, None, "preview", 300)
                .unwrap()
                .with_execution_contracts(&["1=not-an-address,also-invalid".into()])
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejected_swaps_are_path_limits_but_other_api_errors_remain_failures() {
        for (message, status, unavailable) in [
            (
                "This swap is not available",
                reqwest::StatusCode::BAD_REQUEST,
                true,
            ),
            (
                "Invalid token address",
                reqwest::StatusCode::BAD_REQUEST,
                false,
            ),
            (
                "This swap is not available",
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                false,
            ),
        ] {
            let app = axum::Router::new()
                .route(
                    "/v2/tokens",
                    axum::routing::get(|| async {
                        axum::Json(json!([
                            {"symbol": "USDT", "address": "usdt", "chainId": 56, "decimals": 18},
                            {"symbol": "USDC", "address": "usdc", "chainId": 56, "decimals": 18}
                        ]))
                    }),
                )
                .route(
                    "/v1/swap",
                    axum::routing::post(move || async move {
                        (status, axum::Json(json!({"message": message})))
                    }),
                );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let provider = SymbiosisRouteProvider::new(
                format!("http://{address}"),
                None,
                "0x0000000000000000000000000000000000000001",
                300,
            )
            .unwrap();
            let from = Asset::parse("USDT@bnb-smart-chain").unwrap();
            let error = provider
                .quote(
                    from.clone(),
                    Asset::parse("USDC@bnb-smart-chain").unwrap(),
                    Amount::new("100", from).unwrap(),
                )
                .await
                .unwrap_err();
            assert_eq!(
                error.downcast_ref::<QuoteUnavailable>().is_some(),
                unavailable,
                "status: {status}, message: {message}"
            );
            server.abort();
        }
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

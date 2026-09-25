//! Internal route production for Pay3Flow.
//!
//! This module deliberately has no HTTP search handler.  Pay3Flow builds a
//! bounded graph from provider capabilities, publishes stable route
//! capabilities to fmatch, and keeps live prices and quote expiry private.

use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use uuid::Uuid;

const DEFAULT_INTENTS_URL: &str = "https://1click.chaindefuser.com";
const DEFAULT_SLIPPAGE_BPS: u32 = 100;
const DEFAULT_QUOTE_TTL: Duration = Duration::from_secs(180);

/// A currency/token plus its chain or venue.  A missing location is valid for
/// fiat currencies; crypto assets must be qualified before entering a graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asset {
    pub symbol: String,
    pub location: Option<String>,
}

impl Serialize for Asset {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl Asset {
    pub fn new(symbol: impl AsRef<str>, location: Option<&str>) -> Result<Self> {
        let symbol = normalize_code(symbol.as_ref(), "asset symbol")?;
        let location = location.map(normalize_location).transpose()?;
        Ok(Self { symbol, location })
    }

    pub fn parse(value: &str) -> Result<Self> {
        let mut parts = value.split('@');
        let symbol = parts.next().unwrap_or_default();
        let location = parts.next();
        if parts.next().is_some() {
            bail!("asset must have the form SYMBOL or SYMBOL@LOCATION");
        }
        Self::new(symbol, location)
    }

    pub fn qualified(&self) -> bool {
        self.location.is_some()
    }
}

impl Display for Asset {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            Some(location) => write!(formatter, "{}@{location}", self.symbol),
            None => formatter.write_str(&self.symbol),
        }
    }
}

/// An amount carried through a route.  Values are kept as decimal strings at
/// the boundary so provider precision is not lost to binary floating point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Amount {
    pub value: String,
    pub asset: Asset,
}

impl Amount {
    pub fn new(value: impl Into<String>, asset: Asset) -> Result<Self> {
        let value = value.into();
        let value = value.trim();
        let value = value.strip_prefix('+').unwrap_or(value).to_string();
        validate_decimal(&value, false).context("invalid amount")?;
        Ok(Self { value, asset })
    }

    pub fn from_f64(value: f64, asset: Asset) -> Result<Self> {
        if !value.is_finite() || value < 0.0 {
            bail!("amount must be a finite non-negative number");
        }
        Self::new(value.to_string(), asset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    P2pBuy,
    Withdraw,
    Transfer,
    NearIntentSwap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteEdge {
    pub kind: EdgeKind,
    pub from: Asset,
    pub to: Asset,
    pub provider: String,
    /// A configured fee paid in the edge's origin asset.  Fees are private
    /// execution data and are never copied into a published capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<Amount>,
}

impl RouteEdge {
    pub fn new(
        kind: EdgeKind,
        from: Asset,
        to: Asset,
        provider: impl Into<String>,
    ) -> Result<Self> {
        if matches!(kind, EdgeKind::P2pBuy) && to.location.is_none() {
            bail!("P2P buy destination must include a venue or network");
        }
        if from == to {
            bail!("route edge cannot be self-referential");
        }
        let provider = provider.into();
        if provider.trim().is_empty() {
            bail!("route edge provider cannot be empty");
        }
        Ok(Self {
            kind,
            from,
            to,
            provider,
            fee: None,
        })
    }

    pub fn with_fee(mut self, fee: Amount) -> Result<Self> {
        if fee.asset != self.from {
            bail!("edge fee must use the edge origin asset");
        }
        self.fee = Some(fee);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteCapability {
    pub id: String,
    pub from: Asset,
    pub to: Asset,
    pub provider: String,
    pub path: Vec<Asset>,
    pub capabilities: Vec<String>,
    pub enabled: bool,
}

impl RouteCapability {
    fn from_edges(edges: &[RouteEdge]) -> Result<Self> {
        let first = edges.first().context("route has no edges")?;
        let path = std::iter::once(first.from.clone())
            .chain(edges.iter().map(|edge| edge.to.clone()))
            .collect::<Vec<_>>();
        let id = stable_route_id(&path);
        let mut capabilities = edges
            .iter()
            .flat_map(|edge| match edge.kind {
                EdgeKind::P2pBuy => vec!["p2p"],
                EdgeKind::Withdraw | EdgeKind::Transfer | EdgeKind::NearIntentSwap => {
                    vec!["crosschain"]
                }
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        capabilities.sort();
        capabilities.dedup();
        Ok(Self {
            id,
            from: first.from.clone(),
            to: edges.last().context("route has no final edge")?.to.clone(),
            provider: "pay3flow".into(),
            path,
            capabilities,
            enabled: true,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDefinition {
    pub capability: RouteCapability,
    pub edges: Vec<RouteEdge>,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteGraphConfig {
    pub max_depth: usize,
}

impl Default for RouteGraphConfig {
    fn default() -> Self {
        Self { max_depth: 4 }
    }
}

/// Build simple paths from a bounded edge set.  The caller supplies only
/// currently executable edges; unsupported tokens and unavailable withdrawals
/// therefore never become advertised capabilities.
#[derive(Debug, Clone, Copy)]
pub struct RouteGraph {
    config: RouteGraphConfig,
}

impl RouteGraph {
    pub fn new(config: RouteGraphConfig) -> Result<Self> {
        if !(1..=8).contains(&config.max_depth) {
            bail!("route max_depth must be between 1 and 8");
        }
        Ok(Self { config })
    }

    pub fn build(&self, edges: &[RouteEdge]) -> Result<Vec<RouteDefinition>> {
        let mut adjacency: HashMap<&Asset, Vec<&RouteEdge>> = HashMap::new();
        for edge in edges {
            adjacency.entry(&edge.from).or_default().push(edge);
        }

        let mut routes = Vec::new();
        let mut seen = HashSet::new();
        for edge in edges {
            let mut path = vec![edge.clone()];
            let mut visited = HashSet::from([edge.from.clone(), edge.to.clone()]);
            collect_paths(
                &adjacency,
                self.config.max_depth,
                &mut path,
                &mut visited,
                &mut routes,
                &mut seen,
            )?;
        }
        routes.sort_by(|left, right| left.capability.id.cmp(&right.capability.id));
        Ok(routes)
    }
}

fn collect_paths(
    adjacency: &HashMap<&Asset, Vec<&RouteEdge>>,
    max_depth: usize,
    path: &mut Vec<RouteEdge>,
    visited: &mut HashSet<Asset>,
    routes: &mut Vec<RouteDefinition>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let capability = RouteCapability::from_edges(path)?;
    if seen.insert(capability.id.clone()) {
        routes.push(RouteDefinition {
            capability,
            edges: path.clone(),
        });
    }
    if path.len() >= max_depth {
        return Ok(());
    }
    let destination = path.last().context("path is empty")?.to.clone();
    let Some(next_edges) = adjacency.get(&destination) else {
        return Ok(());
    };
    for edge in next_edges {
        if visited.contains(&edge.to) {
            continue;
        }
        visited.insert(edge.to.clone());
        path.push((*edge).clone());
        collect_paths(adjacency, max_depth, path, visited, routes, seen)?;
        path.pop();
        visited.remove(&edge.to);
    }
    Ok(())
}

fn stable_route_id(path: &[Asset]) -> String {
    let slug = path
        .iter()
        .map(|asset| asset.to_string().to_ascii_lowercase().replace('@', "-"))
        .collect::<Vec<_>>()
        .join("-");
    format!("pay3flow:{slug}")
}

#[derive(Debug, Clone, Default)]
pub struct RouteRegistry {
    routes: Arc<RwLock<HashMap<String, RouteCapability>>>,
    definitions: Arc<RwLock<HashMap<String, RouteDefinition>>>,
}

#[derive(Debug, Clone, Default)]
pub struct RouteReconciliation {
    pub upsert: Vec<RouteCapability>,
    pub disable: Vec<RouteCapability>,
}

impl RouteRegistry {
    pub async fn reconcile(&self, current: Vec<RouteCapability>) -> RouteReconciliation {
        let definitions = current
            .into_iter()
            .map(|capability| RouteDefinition {
                edges: route_edges(&capability),
                capability,
            })
            .collect();
        self.reconcile_definitions(definitions).await
    }

    pub async fn reconcile_definitions(
        &self,
        current: Vec<RouteDefinition>,
    ) -> RouteReconciliation {
        let mut guard = self.routes.write().await;
        let incoming_definitions = current
            .into_iter()
            .map(|route| (route.capability.id.clone(), route))
            .collect::<HashMap<_, _>>();
        let incoming = incoming_definitions
            .iter()
            .map(|(id, route)| (id.clone(), route.capability.clone()))
            .collect::<HashMap<_, _>>();
        let mut upsert = incoming.values().cloned().collect::<Vec<_>>();
        upsert.sort_by(|left, right| left.id.cmp(&right.id));
        let mut disable = guard
            .iter()
            .filter(|(id, _)| !incoming.contains_key(*id))
            .map(|(_, route)| {
                let mut disabled = route.clone();
                disabled.enabled = false;
                disabled
            })
            .collect::<Vec<_>>();
        disable.sort_by(|left, right| left.id.cmp(&right.id));
        *guard = incoming;
        drop(guard);
        *self.definitions.write().await = incoming_definitions;
        RouteReconciliation { upsert, disable }
    }

    pub async fn get(&self, route_id: &str) -> Option<RouteCapability> {
        self.routes.read().await.get(route_id).cloned()
    }

    pub async fn snapshot(&self) -> Vec<RouteCapability> {
        self.routes.read().await.values().cloned().collect()
    }

    async fn definition(&self, route_id: &str) -> Option<RouteDefinition> {
        self.definitions.read().await.get(route_id).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearToken {
    #[serde(rename = "assetId", alias = "asset_id")]
    pub asset_id: String,
    pub blockchain: String,
    pub symbol: String,
    #[serde(default)]
    pub decimals: Option<u8>,
    #[serde(
        default,
        rename = "contractAddress",
        alias = "contract_address",
        alias = "address"
    )]
    pub contract_address: Option<String>,
}

impl NearToken {
    pub fn matches(&self, asset: &Asset) -> bool {
        asset.location.as_deref().is_some_and(|location| {
            self.symbol.eq_ignore_ascii_case(&asset.symbol)
                && same_chain(location, &self.blockchain)
        })
    }
}

fn same_chain(asset_location: &str, provider_chain: &str) -> bool {
    fn canonical(value: &str) -> String {
        match value.trim().to_ascii_lowercase().as_str() {
            "xrp" => "xrpl".into(),
            "eth" => "ethereum".into(),
            "arb" => "arbitrum-one".into(),
            "op" => "optimism".into(),
            "sol" => "solana".into(),
            "bsc" => "bnb-smart-chain".into(),
            "pol" => "polygon-pos".into(),
            "avax" => "avalanche-c".into(),
            "doge" => "dogecoin".into(),
            "btc" => "bitcoin".into(),
            "ltc" => "litecoin".into(),
            "near" => "near".into(),
            other => other.to_string(),
        }
    }

    canonical(asset_location) == canonical(provider_chain)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearQuote {
    pub quote_id: String,
    pub from: Asset,
    pub to: Asset,
    pub input: Amount,
    pub output: Amount,
    pub fee: Option<Amount>,
    pub expires_at: Option<DateTime<Utc>>,
    pub available: bool,
    #[serde(default)]
    pub deposit_address: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearSwapStatus {
    pub status: String,
    #[serde(default)]
    pub output_amount: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone)]
pub struct NearQuoteRequest {
    pub from: Asset,
    pub to: Asset,
    pub amount: Amount,
    pub recipient: String,
    pub refund_to: String,
    pub slippage_bps: u32,
    pub dry: bool,
}

#[async_trait]
pub trait QuoteProvider: Send + Sync {
    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<NearQuote>;
}

/// NEAR Intents 1-Click provider.  The provider is read-only by default:
/// `quote` requests a dry quote, while `executable_quote` must be called
/// explicitly by the later execution flow.
#[derive(Clone)]
pub struct NearIntentsProvider {
    client: Client,
    base_url: String,
    jwt: Option<String>,
    slippage_bps: u32,
    quote_ttl: Duration,
    tokens: Arc<RwLock<Vec<NearToken>>>,
    quote_recipient: Option<String>,
    quote_refund_to: Option<String>,
}

impl NearIntentsProvider {
    pub fn new(base_url: impl Into<String>, jwt: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Pay3Flow-NEAR-Intents/0.1")
            .build()
            .context("failed to build NEAR Intents client")?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            jwt,
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            quote_ttl: DEFAULT_QUOTE_TTL,
            tokens: Arc::new(RwLock::new(Vec::new())),
            quote_recipient: None,
            quote_refund_to: None,
        })
    }

    pub fn with_default_url() -> Result<Self> {
        Self::new(DEFAULT_INTENTS_URL, None)
    }

    pub fn with_slippage_bps(mut self, slippage_bps: u32) -> Result<Self> {
        if slippage_bps > 10_000 {
            bail!("slippage must be at most 10000 basis points");
        }
        self.slippage_bps = slippage_bps;
        Ok(self)
    }

    /// Configure valid destination and refund addresses for dry quotes made
    /// through the `QuoteProvider` trait.  The 1Click API validates these
    /// addresses even when `dry=true`; executable quotes receive their
    /// addresses directly through `NearQuoteRequest`.
    pub fn with_quote_addresses(
        mut self,
        recipient: impl Into<String>,
        refund_to: impl Into<String>,
    ) -> Result<Self> {
        let recipient = recipient.into();
        let refund_to = refund_to.into();
        if recipient.trim().is_empty() || refund_to.trim().is_empty() {
            bail!("NEAR Intents quote addresses cannot be empty");
        }
        self.quote_recipient = Some(recipient);
        self.quote_refund_to = Some(refund_to);
        Ok(self)
    }

    pub async fn load_supported_tokens(&self) -> Result<Vec<NearToken>> {
        let mut request = self.client.get(self.endpoint("tokens"));
        if let Some(jwt) = &self.jwt {
            request = request.bearer_auth(jwt);
        }
        let response = request.send().await.context("fetch NEAR Intents tokens")?;
        ensure_success(response.status(), "fetch NEAR Intents tokens")?;
        let tokens = response
            .json::<Vec<NearToken>>()
            .await
            .context("decode NEAR Intents token list")?;
        *self.tokens.write().await = tokens.clone();
        Ok(tokens)
    }

    pub async fn supported_tokens(&self) -> Vec<NearToken> {
        self.tokens.read().await.clone()
    }

    #[cfg(test)]
    fn set_supported_tokens(&self, tokens: Vec<NearToken>) {
        if let Ok(mut guard) = self.tokens.try_write() {
            *guard = tokens;
        }
    }

    pub async fn route_available(&self, from: &Asset, to: &Asset) -> bool {
        let tokens = self.tokens.read().await;
        tokens.iter().any(|token| token.matches(from))
            && tokens.iter().any(|token| token.matches(to))
    }

    pub async fn executable_quote(&self, request: NearQuoteRequest) -> Result<NearQuote> {
        self.request_quote(NearQuoteRequest {
            dry: false,
            ..request
        })
        .await
    }

    pub async fn status(&self, deposit_address: &str) -> Result<NearSwapStatus> {
        self.status_with_memo(deposit_address, None).await
    }

    pub async fn status_with_memo(
        &self,
        deposit_address: &str,
        deposit_memo: Option<&str>,
    ) -> Result<NearSwapStatus> {
        let mut request = self
            .client
            .get(self.endpoint("status"))
            .query(&[("depositAddress", deposit_address)]);
        if let Some(deposit_memo) = deposit_memo {
            request = request.query(&[("depositMemo", deposit_memo)]);
        }
        if let Some(jwt) = &self.jwt {
            request = request.bearer_auth(jwt);
        }
        let response = request.send().await.context("fetch NEAR Intents status")?;
        ensure_success(response.status(), "fetch NEAR Intents status")?;
        let raw = response
            .json::<Value>()
            .await
            .context("decode NEAR Intents status")?;
        Ok(NearSwapStatus {
            status: raw
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN")
                .to_string(),
            output_amount: first_string(&raw, &["amountOut", "amount_out"]),
            raw,
        })
    }

    async fn request_quote(&self, request: NearQuoteRequest) -> Result<NearQuote> {
        if request.amount.asset != request.from {
            bail!("quote amount asset must equal the origin asset");
        }
        if !request.from.qualified() || !request.to.qualified() {
            bail!("NEAR Intents assets must include a blockchain location");
        }
        let (origin_asset, origin_decimals, destination_asset, destination_decimals) = {
            let tokens = self.tokens.read().await;
            let origin = tokens
                .iter()
                .find(|token| token.matches(&request.from))
                .with_context(|| format!("unsupported NEAR Intents origin {}", request.from))?;
            let destination = tokens
                .iter()
                .find(|token| token.matches(&request.to))
                .with_context(|| format!("unsupported NEAR Intents destination {}", request.to))?;
            (
                origin.asset_id.clone(),
                origin.decimals,
                destination.asset_id.clone(),
                destination.decimals,
            )
        };
        let atomic_amount = decimal_to_atomic(&request.amount.value, origin_decimals)?;
        let body = json!({
            "dry": request.dry,
            "swapType": "EXACT_INPUT",
            "slippageTolerance": request.slippage_bps,
            "originAsset": origin_asset,
            "destinationAsset": destination_asset,
            "amount": atomic_amount,
            "depositType": "ORIGIN_CHAIN",
            "refundTo": request.refund_to,
            "refundType": "ORIGIN_CHAIN",
            "recipient": request.recipient,
            "recipientType": "DESTINATION_CHAIN",
            "deadline": (Utc::now() + chrono::Duration::from_std(self.quote_ttl)?).to_rfc3339(),
            "quoteWaitingTimeMs": 3000,
        });
        let mut builder = self.client.post(self.endpoint("quote"));
        if let Some(jwt) = &self.jwt {
            builder = builder.bearer_auth(jwt);
        }
        let response = builder
            .json(&body)
            .send()
            .await
            .context("request NEAR Intents quote")?;
        ensure_success(response.status(), "request NEAR Intents quote")?;
        let raw = response
            .json::<Value>()
            .await
            .context("decode NEAR Intents quote")?;
        let quote_payload = raw.get("quote").cloned().unwrap_or_else(|| raw.clone());
        let mut quote = parse_near_quote_with_decimals(
            quote_payload,
            request.from,
            request.to,
            request.amount,
            request.dry,
            destination_decimals,
            origin_decimals,
        )?;
        quote.raw = raw;
        Ok(quote)
    }

    fn endpoint(&self, path: &str) -> String {
        let base = self
            .base_url
            .strip_suffix("/v0")
            .unwrap_or(&self.base_url)
            .trim_end_matches('/');
        format!("{base}/v0/{path}")
    }
}

/// Inputs used by the periodic capability refresh.  The list of fiat
/// currencies and intermediates is deployment configuration, not user input.
#[derive(Debug, Clone)]
pub struct RouteRefreshConfig {
    pub source_fiats: Vec<String>,
    pub p2p_assets: Vec<String>,
    pub intent_assets: Vec<Asset>,
    pub withdrawal_edges: Vec<RouteEdge>,
}

impl Default for RouteRefreshConfig {
    fn default() -> Self {
        Self {
            source_fiats: vec!["AMD".into()],
            p2p_assets: vec!["USDT".into(), "USDC".into(), "XRP".into()],
            intent_assets: vec![
                asset_for_default("USDT", "tron"),
                asset_for_default("USDC", "solana"),
                asset_for_default("XRP", "xrpl"),
            ],
            withdrawal_edges: Vec::new(),
        }
    }
}

fn asset_for_default(symbol: &str, location: &str) -> Asset {
    Asset {
        symbol: symbol.into(),
        location: Some(location.into()),
    }
}

/// Owns the private graph and its stable capability registry.  Refreshing it
/// queries configured P2P boards and the NEAR token catalog, then reconciles
/// fmatch-visible capabilities without exposing the graph over HTTP.
#[derive(Clone)]
pub struct RouteEngine {
    graph: RouteGraph,
    pub registry: RouteRegistry,
}

impl RouteEngine {
    pub fn new(config: RouteGraphConfig) -> Result<Self> {
        Ok(Self {
            graph: RouteGraph::new(config)?,
            registry: RouteRegistry::default(),
        })
    }

    pub async fn refresh(
        &self,
        p2p: &crate::p2p::P2pSearchService,
        near: &NearIntentsProvider,
        config: &RouteRefreshConfig,
    ) -> Result<RouteReconciliation> {
        let mut edges = config.withdrawal_edges.clone();
        for fiat in &config.source_fiats {
            let fiat = Asset::new(fiat, None)?;
            for symbol in &config.p2p_assets {
                let response = p2p
                    .search(crate::p2p::P2pSearchQuery {
                        fiat: fiat.symbol.clone(),
                        asset: symbol.clone(),
                        side: crate::p2p::P2pSide::BuyCrypto,
                        amount: None,
                        payment_method: None,
                        merchant_only: None,
                        min_orders: None,
                        min_completion_rate: None,
                        limit: Some(20),
                        sources: None,
                    })
                    .await
                    .with_context(|| format!("refresh P2P capabilities for {fiat} → {symbol}"))?;
                for offer in response.offers {
                    let to = Asset::new(&offer.asset, Some(offer.source.as_str()))?;
                    edges.push(RouteEdge::new(
                        EdgeKind::P2pBuy,
                        fiat.clone(),
                        to,
                        offer.source,
                    )?);
                }
            }
        }

        let supported = near.supported_tokens().await;
        let intent_assets = config
            .intent_assets
            .iter()
            .filter(|asset| supported.iter().any(|token| token.matches(asset)))
            .cloned()
            .collect::<Vec<_>>();
        for from in &intent_assets {
            for to in &intent_assets {
                if from != to {
                    edges.push(RouteEdge::new(
                        EdgeKind::NearIntentSwap,
                        from.clone(),
                        to.clone(),
                        "near-intents",
                    )?);
                }
            }
        }
        let routes = self
            .graph
            .build(&deduplicate_edges(edges))?
            .into_iter()
            .filter(|route| {
                config.source_fiats.iter().any(|source| {
                    route.capability.from.symbol.eq_ignore_ascii_case(source)
                        && route.capability.from.location.is_none()
                })
            })
            .collect();
        Ok(self.registry.reconcile_definitions(routes).await)
    }
}

fn deduplicate_edges(edges: Vec<RouteEdge>) -> Vec<RouteEdge> {
    let mut seen = HashSet::new();
    edges
        .into_iter()
        .filter(|edge| {
            seen.insert((
                edge.kind.clone(),
                edge.from.clone(),
                edge.to.clone(),
                edge.provider.clone(),
                edge.fee.clone(),
            ))
        })
        .collect()
}

/// Parse deployment-owned withdrawal/transfer edges.  Each item uses
/// `FROM>TO|provider[|fee]`, for example
/// `USDT@binance>USDT@tron|binance|1.5`.
///
/// A fee is denominated in the origin asset.  It is optional because some
/// providers quote a fee dynamically; those providers can supply it through
/// the edge quote source instead.
pub fn parse_route_edges(values: &[String]) -> Result<Vec<RouteEdge>> {
    values
        .iter()
        .map(|value| {
            let mut fields = value.split('|');
            let pair = fields
                .next()
                .with_context(|| format!("route edge `{value}` must contain `from>to`"))?;
            let provider = fields
                .next()
                .with_context(|| format!("route edge `{value}` must contain `|provider`"))?;
            let fee = fields.next();
            if fields.next().is_some() {
                bail!("route edge `{value}` has too many `|` fields");
            }
            let (from, to) = pair
                .split_once('>')
                .with_context(|| format!("route edge `{value}` must contain `from>to`"))?;
            let from = Asset::parse(from)?;
            let to = Asset::parse(to)?;
            let kind = if from.symbol == to.symbol {
                EdgeKind::Withdraw
            } else {
                EdgeKind::Transfer
            };
            let edge = RouteEdge::new(kind, from.clone(), to, provider.trim())?;
            fee.filter(|fee| !fee.trim().is_empty())
                .map(|fee| Amount::new(fee.trim(), from.clone()))
                .transpose()?
                .map_or(Ok(edge.clone()), |fee| edge.with_fee(fee))
        })
        .collect()
}

#[async_trait]
impl QuoteProvider for NearIntentsProvider {
    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<NearQuote> {
        let request = NearQuoteRequest {
            from: from.clone(),
            to,
            amount,
            recipient: self
                .quote_recipient
                .clone()
                .context("NEAR Intents quote recipient is not configured")?,
            refund_to: self
                .quote_refund_to
                .clone()
                .context("NEAR Intents quote refund address is not configured")?,
            slippage_bps: self.slippage_bps,
            dry: true,
        };
        self.request_quote(request).await
    }
}

fn parse_near_quote_with_decimals(
    raw: Value,
    from: Asset,
    to: Asset,
    input: Amount,
    dry: bool,
    destination_decimals: Option<u8>,
    fee_decimals: Option<u8>,
) -> Result<NearQuote> {
    let output_value = first_string(&raw, &["amountOut", "amount_out", "destinationAmount"])
        .context("NEAR Intents quote has no output amount")?;
    let output_value = destination_decimals
        .map(|decimals| atomic_to_decimal(&output_value, decimals))
        .transpose()?
        .unwrap_or(output_value);
    let output = Amount::new(output_value, to.clone())?;
    let fee = first_string(&raw, &["appFee", "fee", "networkFee"])
        .map(|value| {
            let value = fee_decimals
                .map(|decimals| atomic_to_decimal(&value, decimals))
                .transpose()?
                .unwrap_or(value);
            Amount::new(value, from.clone())
        })
        .transpose()?;
    let expires_at = first_string(&raw, &["deadline", "expiresAt", "expires_at"])
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc));
    Ok(NearQuote {
        quote_id: first_string(&raw, &["quoteId", "quote_id", "id"])
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        from,
        to,
        input,
        output,
        fee,
        expires_at,
        available: raw
            .get("status")
            .and_then(Value::as_str)
            .is_none_or(|status| {
                status.eq_ignore_ascii_case("success") || status.eq_ignore_ascii_case("ok")
            })
            && (dry || raw.get("depositAddress").is_some()),
        deposit_address: raw
            .get("depositAddress")
            .or_else(|| raw.get("deposit_address"))
            .and_then(Value::as_str)
            .map(str::to_string),
        raw,
    })
}

fn decimal_to_atomic(value: &str, decimals: Option<u8>) -> Result<String> {
    let Some(decimals) = decimals else {
        return Ok(value.trim().to_string());
    };
    let value = value.trim();
    validate_decimal(value, false)?;
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > usize::from(decimals) {
        bail!("amount has more precision than the token supports");
    }
    let mut atomic = format!(
        "{}{}{}",
        whole,
        fraction,
        "0".repeat(usize::from(decimals).saturating_sub(fraction.len()))
    );
    let first_nonzero = atomic
        .bytes()
        .position(|byte| byte != b'0')
        .unwrap_or(atomic.len().saturating_sub(1));
    atomic.drain(..first_nonzero);
    Ok(atomic)
}

fn atomic_to_decimal(value: &str, decimals: u8) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("atomic amount must be an unsigned integer");
    }
    let decimals = usize::from(decimals);
    let mut digits = value.trim_start_matches('0').to_string();
    if digits.is_empty() {
        return Ok("0".into());
    }
    if decimals == 0 {
        return Ok(digits);
    }
    if digits.len() <= decimals {
        digits = format!("{}{}", "0".repeat(decimals + 1 - digits.len()), digits);
    }
    let split = digits.len() - decimals;
    let (whole, fraction) = digits.split_at(split);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        Ok(whole.to_string())
    } else {
        Ok(format!("{whole}.{fraction}"))
    }
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn ensure_success(status: StatusCode, operation: &str) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        bail!("{operation} failed with HTTP {status}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteQuote {
    pub route_id: String,
    pub quote_id: String,
    pub input: Amount,
    pub output: Amount,
    pub fees: Vec<Amount>,
    pub available: bool,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EdgePrice {
    pub output: Amount,
    pub fee: Option<Amount>,
    pub available: bool,
}

#[async_trait]
pub trait EdgeQuoteSource: Send + Sync {
    async fn quote_edge(&self, edge: &RouteEdge, input: Amount) -> Result<EdgePrice>;
}

/// Live edge pricing backed by the existing P2P search adapters and the NEAR
/// Intents provider. Withdrawal/transfer fees are injected by deployment code;
/// the edge remains unavailable if a configured fee uses the wrong asset.
pub struct LiveEdgeQuoteSource {
    p2p: crate::p2p::P2pSearchService,
    near: NearIntentsProvider,
    edge_fees: HashMap<(Asset, Asset), Amount>,
}

impl LiveEdgeQuoteSource {
    pub fn new(p2p: crate::p2p::P2pSearchService, near: NearIntentsProvider) -> Self {
        Self {
            p2p,
            near,
            edge_fees: HashMap::new(),
        }
    }

    pub fn with_edge_fee(mut self, from: Asset, to: Asset, fee: Amount) -> Result<Self> {
        if fee.asset != from {
            bail!("edge fee must use the edge origin asset");
        }
        self.edge_fees.insert((from, to), fee);
        Ok(self)
    }
}

#[async_trait]
impl EdgeQuoteSource for LiveEdgeQuoteSource {
    async fn quote_edge(&self, edge: &RouteEdge, input: Amount) -> Result<EdgePrice> {
        match edge.kind {
            EdgeKind::P2pBuy => {
                let fiat_amount = input
                    .value
                    .parse::<f64>()
                    .context("P2P amount is not numeric")?;
                let response = self
                    .p2p
                    .search(crate::p2p::P2pSearchQuery {
                        fiat: edge.from.symbol.clone(),
                        asset: edge.to.symbol.clone(),
                        side: crate::p2p::P2pSide::BuyCrypto,
                        amount: Some(fiat_amount),
                        payment_method: None,
                        merchant_only: None,
                        min_orders: None,
                        min_completion_rate: None,
                        limit: Some(1),
                        sources: Some(edge.provider.clone()),
                    })
                    .await?;
                let offer = response
                    .offers
                    .into_iter()
                    .find(|offer| offer.source.eq_ignore_ascii_case(&edge.provider));
                let Some(offer) = offer else {
                    return Ok(EdgePrice {
                        output: Amount::new("0", edge.to.clone())?,
                        fee: None,
                        available: false,
                    });
                };
                let price = offer
                    .price
                    .parse::<f64>()
                    .context("P2P price is not numeric")?;
                if price <= 0.0 {
                    bail!("P2P price must be positive");
                }
                Ok(EdgePrice {
                    output: Amount::from_f64(fiat_amount / price, edge.to.clone())?,
                    fee: None,
                    available: true,
                })
            }
            EdgeKind::Withdraw | EdgeKind::Transfer => {
                let fee = edge
                    .fee
                    .as_ref()
                    .or_else(|| self.edge_fees.get(&(edge.from.clone(), edge.to.clone())));
                if let Some(fee) = fee {
                    let input_value = input.value.parse::<f64>()?;
                    let fee_value = fee.value.parse::<f64>()?;
                    if fee_value > input_value {
                        return Ok(EdgePrice {
                            output: Amount::new("0", edge.to.clone())?,
                            fee: Some(fee.clone()),
                            available: false,
                        });
                    }
                    return Ok(EdgePrice {
                        output: Amount::from_f64(input_value - fee_value, edge.to.clone())?,
                        fee: Some(fee.clone()),
                        available: true,
                    });
                }
                Ok(EdgePrice {
                    output: Amount::new(input.value, edge.to.clone())?,
                    fee: None,
                    available: true,
                })
            }
            EdgeKind::NearIntentSwap => {
                let quote = match self
                    .near
                    .quote(edge.from.clone(), edge.to.clone(), input)
                    .await
                {
                    Ok(quote) => quote,
                    Err(error) => {
                        tracing::debug!(%error, from = %edge.from, to = %edge.to, "NEAR Intents edge unavailable");
                        return Ok(EdgePrice {
                            output: Amount::new("0", edge.to.clone())?,
                            fee: None,
                            available: false,
                        });
                    }
                };
                Ok(EdgePrice {
                    output: quote.output,
                    fee: quote.fee,
                    available: quote.available,
                })
            }
        }
    }
}

/// Quote a selected internal route.  Route IDs are accepted only from the
/// private registry, so callers cannot turn this into public route discovery.
pub struct RouteQuoteService<S> {
    registry: RouteRegistry,
    source: Arc<S>,
    quote_ttl: Duration,
}

impl<S> RouteQuoteService<S>
where
    S: EdgeQuoteSource + 'static,
{
    pub fn new(registry: RouteRegistry, source: Arc<S>) -> Self {
        Self {
            registry,
            source,
            quote_ttl: DEFAULT_QUOTE_TTL,
        }
    }

    pub fn with_quote_ttl(mut self, quote_ttl: Duration) -> Self {
        self.quote_ttl = quote_ttl;
        self
    }

    pub async fn quote(&self, route_id: &str, input: Amount) -> Result<RouteQuote> {
        let route = self
            .registry
            .definition(route_id)
            .await
            .with_context(|| format!("route {route_id} is not available"))?;
        if !route.capability.enabled || route.capability.from != input.asset {
            bail!("route is unavailable for this input asset");
        }
        let mut current = input.clone();
        let mut fees = Vec::new();
        let mut available = true;
        for edge in route.edges {
            let price = self.source.quote_edge(&edge, current).await?;
            available &= price.available;
            if let Some(fee) = price.fee {
                fees.push(fee);
            }
            current = price.output;
            if !available {
                break;
            }
        }
        Ok(RouteQuote {
            route_id: route.capability.id,
            quote_id: Uuid::new_v4().to_string(),
            input,
            output: current,
            fees,
            available,
            expires_at: Utc::now() + chrono::Duration::from_std(self.quote_ttl)?,
        })
    }
}

// Route capabilities deliberately do not carry edges.  Reconstructing a
// stable path is enough for publication; the executable edge plan belongs to
// the private graph owned by the caller.
fn route_edges(route: &RouteCapability) -> Vec<RouteEdge> {
    route
        .path
        .windows(2)
        .map(|window| {
            let kind = if window[0].location.is_none() {
                EdgeKind::P2pBuy
            } else if window[0].symbol == window[1].symbol {
                EdgeKind::Withdraw
            } else {
                EdgeKind::NearIntentSwap
            };
            RouteEdge {
                kind,
                from: window[0].clone(),
                to: window[1].clone(),
                provider: "pay3flow".into(),
                fee: None,
            }
        })
        .collect()
}

fn normalize_code(value: &str, field: &str) -> Result<String> {
    let value = value.trim().to_ascii_uppercase();
    if !(2..=32).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        bail!("{field} must be 2-32 ASCII letters, digits, '-' or '_'");
    }
    Ok(value)
}

fn normalize_location(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        bail!("asset location must be a non-empty network or venue slug");
    }
    Ok(value)
}

fn validate_decimal(value: &str, allow_negative: bool) -> Result<()> {
    let value = value.trim();
    let value = value.strip_prefix('+').unwrap_or(value);
    let value = if allow_negative {
        value.strip_prefix('-').unwrap_or(value)
    } else {
        value
    };
    let mut dots = 0;
    if value.is_empty() || value.starts_with('.') || value.ends_with('.') {
        bail!("decimal value is malformed");
    }
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => {}
            b'.' => dots += 1,
            _ => bail!("decimal value is malformed"),
        }
    }
    if dots > 1 {
        bail!("decimal value is malformed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(value: &str) -> Asset {
        Asset::parse(value).unwrap()
    }

    #[test]
    fn assets_keep_network_identity() {
        assert_eq!(asset("usdt@TRON").to_string(), "USDT@tron");
        assert_ne!(asset("USDT@tron"), asset("USDT@binance"));
        assert_eq!(asset("AMD").to_string(), "AMD");
        assert_eq!(
            serde_json::to_string(&asset("USDT@tron")).unwrap(),
            "\"USDT@tron\""
        );
    }

    #[test]
    fn graph_builds_direct_and_cross_chain_routes_with_stable_ids() {
        let edges = vec![
            RouteEdge::new(
                EdgeKind::P2pBuy,
                asset("AMD"),
                asset("XRP@binance"),
                "binance",
            )
            .unwrap(),
            RouteEdge::new(
                EdgeKind::P2pBuy,
                asset("AMD"),
                asset("USDT@binance"),
                "binance",
            )
            .unwrap(),
            RouteEdge::new(
                EdgeKind::Withdraw,
                asset("USDT@binance"),
                asset("USDT@tron"),
                "binance",
            )
            .unwrap(),
            RouteEdge::new(
                EdgeKind::NearIntentSwap,
                asset("USDT@tron"),
                asset("XRP@xrpl"),
                "near-intents",
            )
            .unwrap(),
        ];
        let routes = RouteGraph::new(RouteGraphConfig { max_depth: 4 })
            .unwrap()
            .build(&edges)
            .unwrap();
        let ids = routes
            .iter()
            .map(|route| route.capability.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"pay3flow:amd-xrp-binance"));
        assert!(ids.contains(&"pay3flow:amd-usdt-binance-usdt-tron-xrp-xrpl"));
        assert_eq!(ids, {
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            sorted
        });
    }

    #[test]
    fn graph_depth_prevents_useless_combinations() {
        let edges = [
            ("AMD", "USDT@binance", EdgeKind::P2pBuy),
            ("USDT@binance", "USDT@tron", EdgeKind::Withdraw),
            ("USDT@tron", "USDC@tron", EdgeKind::NearIntentSwap),
            ("USDC@tron", "USDC@solana", EdgeKind::Withdraw),
            ("USDC@solana", "XRP@xrpl", EdgeKind::NearIntentSwap),
        ]
        .into_iter()
        .map(|(from, to, kind)| RouteEdge::new(kind, asset(from), asset(to), "test").unwrap())
        .collect::<Vec<_>>();
        let routes = RouteGraph::new(RouteGraphConfig { max_depth: 3 })
            .unwrap()
            .build(&edges)
            .unwrap();
        assert!(routes.iter().all(|route| route.edges.len() <= 3));
        assert!(!routes.iter().any(|route| route.edges.len() == 5));
    }

    #[test]
    fn route_id_changes_when_network_changes() {
        let left = stable_route_id(&[asset("AMD"), asset("USDT@tron")]);
        let right = stable_route_id(&[asset("AMD"), asset("USDT@ethereum")]);
        assert_ne!(left, right);
    }

    #[test]
    fn amount_rejects_lossy_or_malformed_values() {
        assert!(Amount::new("10.25", asset("USDT@tron")).is_ok());
        assert!(Amount::new("1e3", asset("USDT@tron")).is_err());
        assert!(Amount::new("-1", asset("USDT@tron")).is_err());
        assert_eq!(
            Amount::new(" 10.25 ", asset("USDT@tron")).unwrap().value,
            "10.25"
        );
        assert_eq!(Amount::new("+10", asset("USDT@tron")).unwrap().value, "10");
    }

    #[test]
    fn converts_between_human_and_token_base_units_without_floating_point() {
        assert_eq!(decimal_to_atomic("881.4", Some(6)).unwrap(), "881400000");
        assert_eq!(decimal_to_atomic("0.000001", Some(6)).unwrap(), "1");
        assert_eq!(atomic_to_decimal("881400000", 6).unwrap(), "881.4");
        assert_eq!(atomic_to_decimal("1", 6).unwrap(), "0.000001");
        assert!(decimal_to_atomic("1.000001", Some(6)).is_ok());
        assert!(decimal_to_atomic("1.0000001", Some(6)).is_err());
    }

    #[test]
    fn near_token_matching_requires_symbol_and_chain() {
        let token = NearToken {
            asset_id: "nep141:usdt.tron".into(),
            blockchain: "tron".into(),
            symbol: "USDT".into(),
            decimals: Some(6),
            contract_address: None,
        };
        assert!(token.matches(&asset("USDT@tron")));
        assert!(!token.matches(&asset("USDT@ethereum")));

        let xrp_token = NearToken {
            asset_id: "nep141:xrp".into(),
            blockchain: "xrp".into(),
            symbol: "XRP".into(),
            decimals: Some(6),
            contract_address: None,
        };
        assert!(xrp_token.matches(&asset("XRP@xrpl")));
    }

    #[test]
    fn near_token_accepts_the_official_address_field() {
        let token: NearToken = serde_json::from_value(json!({
            "assetId": "nep141:usdt.tron",
            "blockchain": "tron",
            "symbol": "USDT",
            "decimals": 6,
            "address": "usdt.tron"
        }))
        .unwrap();
        assert_eq!(token.contract_address.as_deref(), Some("usdt.tron"));
    }

    #[test]
    fn near_provider_uses_versioned_api_without_duplicate_version_segments() {
        let provider = NearIntentsProvider::new("https://1click.chaindefuser.com", None).unwrap();
        assert_eq!(
            provider.endpoint("tokens"),
            "https://1click.chaindefuser.com/v0/tokens"
        );
        let provider = NearIntentsProvider::new("https://example.test/v0/", None).unwrap();
        assert_eq!(provider.endpoint("quote"), "https://example.test/v0/quote");
    }

    #[test]
    fn quote_parser_keeps_output_and_expiry() {
        let parsed = parse_near_quote_with_decimals(
            json!({
                "quoteId": "q-1",
                "amountOut": "881.4",
                "deadline": "2030-01-01T00:00:00Z",
                "depositAddress": "0xabc"
            }),
            asset("USDT@tron"),
            asset("XRP@xrpl"),
            Amount::new("1000", asset("USDT@tron")).unwrap(),
            false,
            None,
            None,
        )
        .unwrap();
        assert_eq!(parsed.quote_id, "q-1");
        assert_eq!(parsed.output.value, "881.4");
        assert!(parsed.available);
        assert_eq!(parsed.deposit_address.as_deref(), Some("0xabc"));
    }

    #[test]
    fn quote_parser_supports_current_nested_base_unit_response() {
        let parsed = parse_near_quote_with_decimals(
            json!({
                "quoteId": "q-2",
                "amountOut": "881400000",
                "deadline": "2030-01-01T00:00:00Z",
                "depositAddress": "0xabc"
            }),
            asset("USDT@tron"),
            asset("XRP@xrpl"),
            Amount::new("1000", asset("USDT@tron")).unwrap(),
            false,
            Some(6),
            Some(6),
        )
        .unwrap();
        assert_eq!(parsed.quote_id, "q-2");
        assert_eq!(parsed.output.value, "881.4");
    }

    #[tokio::test]
    async fn registry_emits_disabled_tombstones_for_removed_routes() {
        let registry = RouteRegistry::default();
        let edge = RouteEdge::new(
            EdgeKind::P2pBuy,
            asset("AMD"),
            asset("USDT@binance"),
            "binance",
        )
        .unwrap();
        let definition = RouteDefinition {
            capability: RouteCapability::from_edges(std::slice::from_ref(&edge)).unwrap(),
            edges: vec![edge],
        };
        let first = registry.reconcile_definitions(vec![definition]).await;
        assert_eq!(first.upsert.len(), 1);
        let second = registry.reconcile_definitions(Vec::new()).await;
        assert_eq!(second.disable.len(), 1);
        assert!(!second.disable[0].enabled);
    }

    #[tokio::test]
    async fn route_quote_keeps_fees_private_and_calls_each_edge() {
        struct FixedSource;

        #[async_trait]
        impl EdgeQuoteSource for FixedSource {
            async fn quote_edge(&self, edge: &RouteEdge, input: Amount) -> Result<EdgePrice> {
                let output = if edge.kind == EdgeKind::P2pBuy {
                    Amount::new("100", edge.to.clone())?
                } else {
                    Amount::new("95", edge.to.clone())?
                };
                Ok(EdgePrice {
                    output,
                    fee: Some(Amount::new("1", input.asset)?),
                    available: true,
                })
            }
        }

        let edges = vec![
            RouteEdge::new(
                EdgeKind::P2pBuy,
                asset("AMD"),
                asset("USDT@binance"),
                "binance",
            )
            .unwrap(),
            RouteEdge::new(
                EdgeKind::Withdraw,
                asset("USDT@binance"),
                asset("USDT@tron"),
                "binance",
            )
            .unwrap(),
        ];
        let capability = RouteCapability::from_edges(&edges).unwrap();
        let registry = RouteRegistry::default();
        registry
            .reconcile_definitions(vec![RouteDefinition {
                capability: capability.clone(),
                edges,
            }])
            .await;
        let service = RouteQuoteService::new(registry, Arc::new(FixedSource));
        let quote = service
            .quote(&capability.id, Amount::new("500000", asset("AMD")).unwrap())
            .await
            .unwrap();
        assert_eq!(quote.output.value, "95");
        assert_eq!(quote.fees.len(), 2);
        assert!(quote.available);
        assert!(quote.expires_at > Utc::now());
    }

    #[tokio::test]
    async fn unavailable_edge_returns_unavailable_quote_without_pricing_later_edges() {
        struct UnavailableSource;

        #[async_trait]
        impl EdgeQuoteSource for UnavailableSource {
            async fn quote_edge(&self, edge: &RouteEdge, _input: Amount) -> Result<EdgePrice> {
                assert_eq!(edge.kind, EdgeKind::P2pBuy);
                Ok(EdgePrice {
                    output: Amount::new("0", edge.to.clone())?,
                    fee: None,
                    available: false,
                })
            }
        }

        let edges = vec![RouteEdge::new(
            EdgeKind::P2pBuy,
            asset("AMD"),
            asset("USDT@binance"),
            "binance",
        )
        .unwrap()];
        let capability = RouteCapability::from_edges(&edges).unwrap();
        let registry = RouteRegistry::default();
        registry
            .reconcile_definitions(vec![RouteDefinition {
                capability: capability.clone(),
                edges,
            }])
            .await;
        let service = RouteQuoteService::new(registry, Arc::new(UnavailableSource));
        let quote = service
            .quote(&capability.id, Amount::new("500000", asset("AMD")).unwrap())
            .await
            .unwrap();
        assert!(!quote.available);
        assert_eq!(quote.output.value, "0");
    }

    #[tokio::test]
    async fn near_provider_checks_token_location_before_quoting() {
        let provider = NearIntentsProvider::new("http://127.0.0.1:1", None).unwrap();
        provider.set_supported_tokens(vec![NearToken {
            asset_id: "nep141:usdt.tron".into(),
            blockchain: "tron".into(),
            symbol: "USDT".into(),
            decimals: Some(6),
            contract_address: None,
        }]);
        assert!(
            provider
                .route_available(&asset("USDT@tron"), &asset("USDT@tron"))
                .await
        );
        assert!(
            !provider
                .route_available(&asset("USDT@tron"), &asset("XRP@xrpl"))
                .await
        );
    }

    #[test]
    fn deployment_edges_require_explicit_provider_and_preserve_kind() {
        let edges = parse_route_edges(&["USDT@binance>USDT@tron|binance|1.5".into()]).unwrap();
        assert_eq!(edges[0].kind, EdgeKind::Withdraw);
        assert_eq!(edges[0].provider, "binance");
        assert_eq!(edges[0].fee.as_ref().unwrap().value, "1.5");
        assert_eq!(edges[0].fee.as_ref().unwrap().asset, asset("USDT@binance"));
        assert!(parse_route_edges(&["USDT@binance>USDT@tron".into()]).is_err());
        assert!(parse_route_edges(&["USDT@binance>USDT@tron|binance|1|extra".into()]).is_err());
    }
}

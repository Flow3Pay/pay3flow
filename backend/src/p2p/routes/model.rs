use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::p2p::service::{P2pOffer, SourceStatus};
use crate::service_reputation::{
    CombinedReputation, RouteFeedback, RouteServiceStats, ServiceLink,
};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeMode {
    #[default]
    All,
    P2p,
    Exchanger,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pRouteSearchQuery {
    pub source_fiat: String,
    pub target_fiat: String,
    pub source_amount: f64,
    pub source_network: Option<String>,
    pub target_network: Option<String>,
    /// Legacy field. Crypto-to-crypto routes no longer use a fiat pivot.
    pub bridge_fiat: Option<String>,
    /// Backward-compatible alias for `intermediary_assets`.
    pub assets: Option<String>,
    /// Optional comma-separated crypto intermediaries for fiat routes or crypto cycles.
    /// Fiat routes default to `p2p_search_assets`; crypto cycles use provider catalogs.
    pub intermediary_assets: Option<String>,
    pub source_payment_method: Option<String>,
    pub target_payment_method: Option<String>,
    /// Payment-method fee from the backend-owned payment-method catalog.
    /// Missing values keep circular-route profitability unconfirmed.
    pub source_payment_fee_percent: Option<f64>,
    /// Payment-method fee from the backend-owned payment-method catalog.
    /// Missing values keep circular-route profitability unconfirmed.
    pub target_payment_fee_percent: Option<f64>,
    pub merchant_only: Option<bool>,
    pub min_orders: Option<u64>,
    pub min_completion_rate: Option<f64>,
    /// Default false: same-venue routes do not require an on-chain transfer.
    pub allow_cross_venue: Option<bool>,
    /// Reject prices too far from the median for that leg. Default 1000 (10%).
    pub max_price_deviation_bps: Option<u32>,
    pub limit: Option<usize>,
    /// Optional comma-separated list of P2P sources to query.
    pub sources: Option<String>,
    /// Limit results to P2P offers, exchanger routes, or search both (default).
    #[serde(default)]
    pub exchange_mode: ExchangeMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pRoute {
    pub route_id: String,
    pub rank: usize,
    pub asset: String,
    /// Canonical network id for the crypto asset selected by the user.
    pub entry_network: Option<String>,
    pub source_network: Option<String>,
    pub target_network: Option<String>,
    pub source_fiat: String,
    pub source_amount: String,
    pub acquired_asset_amount: String,
    /// Amount entering the route-provider leg. It is kept out of the public
    /// response except through a signed execution descriptor.
    #[serde(skip)]
    pub(crate) provider_input_amount: Option<String>,
    pub target_fiat: String,
    pub target_amount: String,
    /// Target-fiat units per one source-fiat unit.
    pub effective_rate: String,
    pub same_venue: bool,
    pub requires_asset_transfer: bool,
    pub transfer_fee_included: bool,
    pub route_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profitability: Option<RouteProfitability>,
    /// Ordered live swap quotes making up a wallet-to-wallet cycle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cycle_legs: Vec<CryptoCycleLeg>,
    /// Decimal scale of the integer amounts in `profitability`; fiat defaults to 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profitability_decimals: Option<u8>,
    pub bridge_currency: Option<String>,
    pub market_path: Option<CryptoMarketPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_provider_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_quote_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_fees: Vec<RouteFee>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_expires_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<RouteExecutionDescriptor>,
    /// True when both selected bank names were present in venue responses.
    /// False means at least one venue returned only opaque payment IDs.
    pub payment_methods_verified: bool,
    pub entry_offer: Option<P2pOffer>,
    pub exit_offer: Option<P2pOffer>,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<RouteServiceStats>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reputation: Option<CombinedReputation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<RouteFeedback>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_links: Vec<ServiceLink>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteExecutionDescriptor {
    pub provider: String,
    pub from_asset: String,
    pub to_asset: String,
    pub input_amount: String,
    pub expires_at: DateTime<Utc>,
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RouteProfitability {
    Confirmed {
        net_profit_minor: i64,
        profit_bps: i32,
    },
    Unconfirmed {
        gross_profit_minor: i64,
        gross_profit_bps: i32,
        missing_costs: Vec<RouteCostKind>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RouteCostKind {
    SourcePaymentFee,
    TargetPaymentFee,
    NetworkFee,
    ProviderFee,
    LiveQuote,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteFee {
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CryptoMarketPath {
    pub venue: String,
    pub source_pair: String,
    pub target_pair: String,
    pub source_rate: String,
    pub target_rate: String,
    pub intermediary_amount: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteAssetStatus {
    pub asset: String,
    pub entry_offers: usize,
    pub exit_offers: usize,
    pub routes_built: usize,
    /// False when route construction stopped after collecting a bounded
    /// top-ranked candidate set instead of enumerating every combination.
    pub routes_exhaustive: bool,
    pub can_exchange_to_target: bool,
    pub entry_sources: Vec<SourceStatus>,
    pub exit_sources: Vec<SourceStatus>,
    /// Internal discovery transport. This is deliberately not serialized as
    /// a venue: Fmatch distributes venue offers but is not itself a venue.
    #[serde(skip)]
    pub(in crate::p2p) entry_discovery_source: Option<String>,
    #[serde(skip)]
    pub(in crate::p2p) exit_discovery_source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pRouteSearchResponse {
    pub search_id: Uuid,
    pub routes_found: usize,
    /// Whether `routes_found` is an exact count of every valid combination.
    pub routes_exhaustive: bool,
    pub searched_at: DateTime<Utc>,
    pub source_fiat: String,
    pub target_fiat: String,
    pub source_amount: String,
    pub assets_searched: Vec<String>,
    pub can_exchange_to_target: bool,
    pub routes: Vec<P2pRoute>,
    pub asset_statuses: Vec<RouteAssetStatus>,
    /// Completion states for route providers that do not produce P2P offers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_statuses: Vec<SourceStatus>,
    /// Discovery source for the route snapshot: Fmatch, database cache, or
    /// the legacy provider path used by local-only callers.
    pub source: String,
    pub stale: bool,
}

/// One sequential swap in a circular crypto route.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CryptoCycleLeg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_pair: Option<String>,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub from_asset: String,
    pub to_asset: String,
    pub input_amount: String,
    pub output_amount: String,
    pub source_url: Option<String>,
    pub quote_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

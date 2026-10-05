use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum P2pSide {
    #[serde(rename = "buy", alias = "buy_crypto")]
    BuyCrypto,
    #[serde(rename = "sell", alias = "sell_crypto")]
    SellCrypto,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2pSearchQuery {
    pub fiat: String,
    pub asset: String,
    pub side: P2pSide,
    /// Fiat amount, for example `100000` AMD. Omit to search every limit range.
    pub amount: Option<f64>,
    /// Crypto amount to sell. Filters by available asset and the resulting fiat limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_amount: Option<f64>,
    pub payment_method: Option<String>,
    pub merchant_only: Option<bool>,
    pub min_orders: Option<u64>,
    /// Fraction from 0 to 1. `0.95` means a 95% completion rate.
    pub min_completion_rate: Option<f64>,
    pub limit: Option<usize>,
    /// Optional comma-separated list of P2P sources to query.
    pub sources: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Advertiser {
    pub id: Option<String>,
    pub nickname: String,
    pub user_type: Option<String>,
    pub is_merchant: bool,
    pub is_verified: bool,
    pub completed_orders_30d: Option<u64>,
    pub completion_rate_30d: Option<f64>,
    pub positive_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct P2pOffer {
    #[serde(default = "default_p2p_offer_market")]
    pub(crate) market: P2pOfferMarket,
    pub source: String,
    pub ad_id: String,
    pub side: P2pSide,
    pub fiat: String,
    pub asset: String,
    /// Canonical transfer network when the venue fixes the asset rail.
    pub network: Option<String>,
    /// Fiat units paid or received for one unit of `asset`.
    pub price: String,
    pub available_asset: String,
    pub min_fiat: String,
    pub max_fiat: String,
    pub payment_methods: Vec<String>,
    pub pay_time_limit_minutes: Option<u32>,
    pub advertiser: Advertiser,
    pub advertiser_profile_url: Option<String>,
    pub source_url: String,
    /// True only when the venue URL addresses this exact advertisement.
    /// Public market URLs must not be presented as exact offer links.
    pub source_url_is_exact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum P2pOfferMarket {
    P2p,
    DirectExchange,
}

fn default_p2p_offer_market() -> P2pOfferMarket {
    P2pOfferMarket::P2p
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStatus {
    pub source: String,
    pub ok: bool,
    /// A cached observation is useful for availability, but is not a new
    /// network-response sample and must not affect latency averages.
    #[serde(default)]
    pub cached: bool,
    pub latency_ms: u128,
    pub offers_found: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2pSearchResponse {
    pub query: P2pSearchQuery,
    pub searched_at: DateTime<Utc>,
    pub cached: bool,
    pub offers: Vec<P2pOffer>,
    pub sources: Vec<SourceStatus>,
    pub source: String,
    pub stale: bool,
    pub observed_at: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FiatRouteQuote {
    pub provider: String,
    pub source_url: String,
    pub source_currency: String,
    pub target_currency: String,
    pub source_amount: f64,
    pub target_amount: f64,
}

//! Request and response shapes for the B2B crypto routing API.
//!
//! Amounts are exact decimal strings at the boundary. Nothing here parses
//! money into `f64`; the only floating-point value in the API is the analytics
//! conversion rate, which is a ratio and not an amount.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// One side of a quote: the token, its network and (for the exact side) the
/// amount.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteSide {
    pub asset: String,
    pub network: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteRequest {
    pub input: QuoteSide,
    pub output: QuoteSide,
    pub recipient: String,
    pub refund_address: String,
    /// `test` or `live`. V1 defaults to `live`.
    #[serde(default)]
    pub env: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTransferRequest {
    pub quote_id: Uuid,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCheckoutRequest {
    #[serde(flatten)]
    pub quote: QuoteRequest,
    #[serde(default)]
    pub success_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteView {
    pub quote_id: Uuid,
    pub order_id: Uuid,
    pub provider: String,
    pub status: String,
    pub exact: String,
    pub input_asset: String,
    pub input_network: String,
    pub output_asset: String,
    pub output_network: String,
    pub input_amount: String,
    pub output_amount: String,
    pub best_output: String,
    pub captured_bps: i32,
    pub app_fee_bps: i32,
    pub recipient: String,
    pub refund_address: String,
    pub deposit_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_memo: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferEventView {
    pub id: i64,
    pub kind: String,
    pub body: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferView {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub env: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub status: String,
    pub input_asset: String,
    pub input_network: String,
    pub output_asset: String,
    pub output_network: String,
    pub recipient: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<QuoteView>,
    pub events: Vec<TransferEventView>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckoutView {
    pub order_id: Uuid,
    pub token: String,
    pub url: String,
    pub status: String,
    pub success_url: Option<String>,
    pub quote: QuoteView,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckoutStatusView {
    pub order_id: Uuid,
    pub status: String,
    pub success_url: Option<String>,
    pub quote: QuoteView,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    #[serde(default)]
    pub events: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookEndpointView {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    /// Returned once at creation; only a ciphertext is stored.
    pub secret: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetAmount {
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureReason {
    pub reason: String,
    pub orders: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyticsView {
    pub orders: i64,
    pub paid_orders: i64,
    pub conversion: f64,
    pub average_ticket: Vec<AssetAmount>,
    pub volume: Vec<AssetAmount>,
    pub captured: Vec<AssetAmount>,
    /// Volume-weighted average captured margin across delivered orders.
    pub margin_bps: i32,
    pub failure_reasons: Vec<FailureReason>,
}

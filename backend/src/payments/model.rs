use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::payments::status::{RouteStatus, TransactionStatus};

/// All amounts are stored in the smallest currency unit (cents for fiat).
pub type Minor = i64;

/// Money math helper: convert a decimal amount (e.g. `100.50`) to minor units.
pub fn to_minor(amount: f64) -> Minor {
    (amount * 100.0).round() as Minor
}

/// Convert minor units to a decimal amount.
pub fn from_minor(minor: Minor) -> f64 {
    minor as f64 / 100.0
}

/// A payment the user wants to make (creation request body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPayment {
    pub amount: f64,
    pub currency: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub to_geo: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    /// Destination currency. `None` = same as `currency` (no conversion).
    #[serde(default)]
    pub to_currency: Option<String>,
}

/// Domain model of a transaction persisted in `transactions`.
#[derive(Debug, Clone, Serialize)]
pub struct Transaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: TransactionStatus,
    pub from_amount: Minor,
    pub from_currency: String,
    pub to_amount: Option<Minor>,
    pub to_currency: Option<String>,
    pub from_account: String,
    pub to_account: String,
    pub method: Option<String>,
    pub fees: Minor,
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New transaction to be inserted.
#[derive(Debug, Clone)]
pub struct NewTransaction {
    pub user_id: Uuid,
    pub from_amount: Minor,
    pub from_currency: String,
    pub to_currency: Option<String>,
    pub from_account: String,
    pub to_account: String,
    pub method: Option<String>,
    pub fees: Minor,
    pub idempotency_key: Option<String>,
}

/// Route: the chosen acquirer leg for a transaction.
#[derive(Debug, Clone, Serialize)]
pub struct Route {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub acquirer_id: Option<Uuid>,
    pub acquirer_slug: String,
    pub fee_percent: f64,
    pub exchange_rate: Option<f64>,
    pub status: RouteStatus,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewRoute {
    pub transaction_id: Uuid,
    pub acquirer_id: Option<Uuid>,
    pub acquirer_slug: String,
    pub fee_percent: f64,
    pub exchange_rate: Option<f64>,
    pub status: RouteStatus,
    pub source: String,
}
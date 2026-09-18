use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::exchange::status::{
    FundingInstructionStatus, LegStatus, OrderStatus, ProofVerificationStatus, QuoteStatus,
    SettlementStatus,
};

pub type Minor = i64;

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeCorridor {
    pub id: Uuid,
    pub source_country: String,
    pub source_currency: String,
    pub target_country: String,
    pub target_currency: String,
    pub status: String,
    pub min_amount_minor: Option<Minor>,
    pub max_amount_minor: Option<Minor>,
    pub daily_limit_minor: Option<Minor>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewExchangeOrder {
    pub user_id: Uuid,
    pub idempotency_key: String,
    pub source_country: String,
    pub source_currency: String,
    pub source_amount_minor: Minor,
    pub source_method_type: String,
    pub source_method_ref: Option<String>,
    pub target_country: String,
    pub target_currency: String,
    pub target_amount_min_minor: Option<Minor>,
    pub target_method_type: String,
    pub target_method_ref: Option<String>,
    pub deadline_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub idempotency_key: String,
    pub source_country: String,
    pub source_currency: String,
    pub source_amount_minor: Minor,
    pub source_method_type: String,
    pub source_method_ref: Option<String>,
    pub target_country: String,
    pub target_currency: String,
    pub target_amount_min_minor: Option<Minor>,
    pub target_method_type: String,
    pub target_method_ref: Option<String>,
    pub funding_instruction_id: Option<Uuid>,
    pub funding_status: FundingInstructionStatus,
    pub status: OrderStatus,
    pub deadline_at: Option<DateTime<Utc>>,
    pub selected_quote_id: Option<Uuid>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeQuote {
    pub id: Uuid,
    pub order_id: Uuid,
    pub solver_id: Uuid,
    pub source_amount_minor: Minor,
    pub target_amount_minor: Minor,
    pub source_currency: String,
    pub target_currency: String,
    pub funding_method_type: String,
    pub requires_user_funding: bool,
    pub rate: String,
    pub fee_minor: Minor,
    pub eta_minutes: i32,
    pub expires_at: DateTime<Utc>,
    pub status: QuoteStatus,
    pub settlement_plan: Value,
    pub risk_score: i32,
    pub score: Option<i64>,
    pub raw_response: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeSettlement {
    pub id: Uuid,
    pub order_id: Uuid,
    pub quote_id: Uuid,
    pub solver_id: Uuid,
    pub status: SettlementStatus,
    pub token_leg_status: LegStatus,
    pub money_leg_status: LegStatus,
    pub funding_status: FundingInstructionStatus,
    pub pay3flow_wallet_ref: Option<String>,
    pub token_ledger_ref: Option<String>,
    pub money_reference: Option<String>,
    pub proof_id: Option<Uuid>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FundingInstruction {
    pub id: Uuid,
    pub order_id: Uuid,
    pub quote_id: Uuid,
    pub solver_id: Uuid,
    pub status: FundingInstructionStatus,
    pub method_type: String,
    pub amount_minor: Minor,
    pub currency: String,
    pub destination_ref: String,
    pub expires_at: DateTime<Utc>,
    pub user_confirmed_at: Option<DateTime<Utc>>,
    pub raw_payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeProof {
    pub id: Uuid,
    pub settlement_id: Uuid,
    pub solver_id: Uuid,
    pub proof_type: String,
    pub proof_payload: Value,
    pub verification_status: ProofVerificationStatus,
    pub verified_by: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub payload: Value,
}

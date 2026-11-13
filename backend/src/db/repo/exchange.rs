//! Exchange statements that run on a caller-provided connection.

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::db::repo::reputation::Conn;

const MARK_QUOTE_SCORED: &str =
    "UPDATE exchange_quotes SET score = $2, status = $3, updated_at = now() WHERE id = $1";
const MARK_QUOTE_SELECTED: &str =
    "UPDATE exchange_quotes SET status = $2, updated_at = now() WHERE id = $1";
const SELECT_QUOTE_ORDER: &str = r#"
UPDATE exchange_orders
SET selected_quote_id = $2,
    status = $3,
    updated_at = now()
WHERE id = $1 AND status = $4
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;
const INSERT_FUNDING_INSTRUCTION_FOR_QUOTE: &str = r#"
INSERT INTO funding_instructions
    (order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
     destination_ref, expires_at, raw_payload)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#;
const SHOW_FUNDING_INSTRUCTION: &str = r#"
UPDATE funding_instructions
SET status = $2, updated_at = now()
WHERE id = $1 AND status = $3
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#;
const LOCK_ORDER_WITH_INSTRUCTION: &str = r#"
UPDATE exchange_orders
SET funding_instruction_id = $2,
    funding_status = $3,
    selected_quote_id = $4,
    status = $5,
    updated_at = now()
WHERE id = $1 AND status = $6
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;
const CONFIRM_FUNDING_INSTRUCTION: &str = r#"
UPDATE funding_instructions
SET status = $2, user_confirmed_at = now(), updated_at = now()
WHERE id = $1 AND status = $3
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#;
const SET_ORDER_FUNDING_STATUS: &str = r#"
UPDATE exchange_orders
SET funding_status = $2, updated_at = now()
WHERE id = $1
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;
const UPSERT_SETTLEMENT: &str = r#"
INSERT INTO exchange_settlements
    (order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
     funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference)
VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, NULL, NULL)
ON CONFLICT (order_id) DO UPDATE SET
    updated_at = now()
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#;
const ACKNOWLEDGE_FUNDING_INSTRUCTION: &str = r#"
UPDATE funding_instructions
SET status = $2, updated_at = now()
WHERE id = $1
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#;
const MARK_SETTLEMENT_EXECUTED: &str = r#"
UPDATE exchange_settlements
SET status = $2,
    token_leg_status = $3,
    money_leg_status = $4,
    funding_status = $5,
    pay3flow_wallet_ref = $6,
    token_ledger_ref = $7,
    money_reference = $8,
    updated_at = now()
WHERE id = $1
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#;
const MARK_ORDER_EXECUTED: &str = r#"
UPDATE exchange_orders
SET status = $2,
    funding_status = $3,
    updated_at = now()
WHERE id = $1
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;
const REFERENCE_ALREADY_VERIFIED: &str = r#"
SELECT 1 FROM exchange_proofs
WHERE proof_payload->>'reference' = $1 AND verification_status = 'verified'
LIMIT 1
"#;
const INSERT_VERIFIED_PROOF: &str = r#"
INSERT INTO exchange_proofs
    (settlement_id, solver_id, proof_type, proof_payload, verification_status, verified_by, verified_at)
VALUES ($1, $2, $3, $4, $5, $6, now())
RETURNING id, settlement_id, solver_id, proof_type, proof_payload, verification_status,
          verified_by, verified_at, created_at
"#;
const SETTLE_PROOF: &str = r#"
UPDATE exchange_settlements
SET status = $2,
    proof_id = $3,
    failure_code = $4,
    failure_message = $5,
    updated_at = now()
WHERE id = $1
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#;
const SETTLE_ORDER_PROOF: &str = r#"
UPDATE exchange_orders
SET status = $2,
    failure_code = $3,
    failure_message = $4,
    updated_at = now()
WHERE id = $1
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;
const UPDATE_SETTLEMENT_REFS: &str = r#"
UPDATE exchange_settlements
SET pay3flow_wallet_ref = COALESCE($2, pay3flow_wallet_ref),
    token_ledger_ref = COALESCE($3, token_ledger_ref),
    money_reference = COALESCE($4, money_reference),
    updated_at = now()
WHERE id = $1
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#;
const FAIL_SETTLEMENT: &str = r#"
UPDATE exchange_settlements
SET status = $2,
    money_leg_status = $3,
    failure_code = $4,
    failure_message = $5,
    updated_at = now()
WHERE id = $1
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#;
const FAIL_ORDER: &str = r#"
UPDATE exchange_orders
SET status = $2,
    failure_code = $3,
    failure_message = $4,
    updated_at = now()
WHERE id = $1
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          pay3flow_fee_minor, pay3flow_fee_currency, source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, correlation_id, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#;

pub async fn mark_quote_scored<C>(
    client: &C,
    quote_id: &Uuid,
    score: i64,
    status: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(MARK_QUOTE_SCORED, &[quote_id, &score, &status])
        .await?;
    Ok(())
}

pub async fn mark_quote_selected<C>(client: &C, quote_id: &Uuid, status: &str) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(MARK_QUOTE_SELECTED, &[quote_id, &status])
        .await?;
    Ok(())
}

/// `Err` when the order was not in the expected status.
pub async fn select_quote_order<C>(
    client: &C,
    order_id: &Uuid,
    quote_id: &Uuid,
    quoted_status: &str,
    quoting_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    client
        .query_opt(
            SELECT_QUOTE_ORDER,
            &[order_id, quote_id, &quoted_status, &quoting_status],
        )
        .await?
        .context("exchange order was not quoting at winner selection time")
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_funding_instruction_for_quote<C>(
    client: &C,
    order_id: &Uuid,
    quote_id: &Uuid,
    solver_id: &Uuid,
    status: &str,
    method_type: &str,
    amount_minor: i64,
    currency: &str,
    destination_ref: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
    raw_payload: &serde_json::Value,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            INSERT_FUNDING_INSTRUCTION_FOR_QUOTE,
            &[
                order_id,
                quote_id,
                solver_id,
                &status,
                &method_type,
                &amount_minor,
                &currency,
                &destination_ref,
                &expires_at,
                raw_payload,
            ],
        )
        .await?)
}

pub async fn show_funding_instruction<C>(
    client: &C,
    instruction_id: &Uuid,
    shown_status: &str,
    created_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            SHOW_FUNDING_INSTRUCTION,
            &[instruction_id, &shown_status, &created_status],
        )
        .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn lock_order_with_instruction<C>(
    client: &C,
    order_id: &Uuid,
    instruction_id: &Uuid,
    shown_status: &str,
    quote_id: &Uuid,
    locked_status: &str,
    quoted_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    client
        .query_opt(
            LOCK_ORDER_WITH_INSTRUCTION,
            &[
                order_id,
                instruction_id,
                &shown_status,
                quote_id,
                &locked_status,
                &quoted_status,
            ],
        )
        .await?
        .context("exchange order was not quoted at confirm time")
}

pub async fn confirm_funding_instruction<C>(
    client: &C,
    instruction_id: &Uuid,
    confirmed_status: &str,
    shown_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            CONFIRM_FUNDING_INSTRUCTION,
            &[instruction_id, &confirmed_status, &shown_status],
        )
        .await?)
}

pub async fn set_order_funding_status<C>(
    client: &C,
    order_id: &Uuid,
    funding_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(SET_ORDER_FUNDING_STATUS, &[order_id, &funding_status])
        .await?)
}

pub async fn upsert_settlement<C>(
    client: &C,
    order_id: &Uuid,
    quote_id: &Uuid,
    solver_id: &Uuid,
    status: &str,
    token_leg_status: &str,
    money_leg_status: &str,
    funding_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            UPSERT_SETTLEMENT,
            &[
                order_id,
                quote_id,
                solver_id,
                &status,
                &token_leg_status,
                &money_leg_status,
                &funding_status,
            ],
        )
        .await?)
}

pub async fn acknowledge_funding_instruction<C>(
    client: &C,
    instruction_id: &Uuid,
    status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(ACKNOWLEDGE_FUNDING_INSTRUCTION, &[instruction_id, &status])
        .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn mark_settlement_executed<C>(
    client: &C,
    settlement_id: &Uuid,
    status: &str,
    token_leg_status: &str,
    money_leg_status: &str,
    funding_status: &str,
    wallet_ref: &str,
    token_ledger_ref: &str,
    money_reference: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            MARK_SETTLEMENT_EXECUTED,
            &[
                settlement_id,
                &status,
                &token_leg_status,
                &money_leg_status,
                &funding_status,
                &wallet_ref,
                &token_ledger_ref,
                &money_reference,
            ],
        )
        .await?)
}

pub async fn mark_order_executed<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    funding_status: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(MARK_ORDER_EXECUTED, &[order_id, &status, &funding_status])
        .await?)
}

pub async fn reference_already_verified<C>(client: &C, reference: &str) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(REFERENCE_ALREADY_VERIFIED, &[&reference])
        .await?
        .is_some())
}

pub async fn insert_verified_proof<C>(
    client: &C,
    settlement_id: &Uuid,
    solver_id: &Uuid,
    proof_type: &str,
    proof_payload: &serde_json::Value,
    verification_status: &str,
    verified_by: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            INSERT_VERIFIED_PROOF,
            &[
                settlement_id,
                solver_id,
                &proof_type,
                proof_payload,
                &verification_status,
                &verified_by,
            ],
        )
        .await?)
}

pub async fn settle_proof<C>(
    client: &C,
    settlement_id: &Uuid,
    status: &str,
    proof_id: &Uuid,
    failure_code: &Option<&str>,
    failure_message: &Option<&str>,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            SETTLE_PROOF,
            &[
                settlement_id,
                &status,
                proof_id,
                failure_code,
                failure_message,
            ],
        )
        .await?)
}

pub async fn settle_order_proof<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    failure_code: &Option<&str>,
    failure_message: &Option<&str>,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            SETTLE_ORDER_PROOF,
            &[order_id, &status, failure_code, failure_message],
        )
        .await?)
}

pub async fn update_settlement_refs<C>(
    client: &C,
    settlement_id: &Uuid,
    pay3flow_wallet_ref: &Option<String>,
    token_ledger_ref: &Option<String>,
    money_reference: &Option<String>,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            UPDATE_SETTLEMENT_REFS,
            &[
                settlement_id,
                pay3flow_wallet_ref,
                token_ledger_ref,
                money_reference,
            ],
        )
        .await?)
}

pub async fn fail_settlement<C>(
    client: &C,
    settlement_id: &Uuid,
    status: &str,
    money_leg_status: &str,
    code: &str,
    message: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            FAIL_SETTLEMENT,
            &[settlement_id, &status, &money_leg_status, &code, &message],
        )
        .await?)
}

pub async fn fail_order<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    code: &str,
    message: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(FAIL_ORDER, &[order_id, &status, &code, &message])
        .await?)
}

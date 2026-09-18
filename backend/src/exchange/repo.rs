use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::model::{
    AuditEvent, ExchangeCorridor, ExchangeOrder, ExchangeProof, ExchangeQuote, ExchangeSettlement,
    ExchangeSolver, FundingInstruction, Minor, NewAuditEvent, NewExchangeOrder, NewExchangeProof,
    NewExchangeQuote, NewExchangeSettlement, NewExchangeSolver, NewFundingInstruction,
};
use crate::exchange::status::{
    FundingInstructionStatus, LegStatus, OrderStatus, ProofVerificationStatus, QuoteStatus,
    SettlementStatus, SolverStatus,
};

const SELECT_ORDER: &str = r#"
SELECT id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
       source_method_type, source_method_ref, target_country, target_currency,
       target_amount_min_minor, target_method_type, target_method_ref,
       funding_instruction_id, funding_status, status, deadline_at, selected_quote_id,
       failure_code, failure_message, created_at, updated_at
FROM exchange_orders
"#;

const SELECT_CORRIDOR: &str = r#"
SELECT id, source_country, source_currency, target_country, target_currency, status,
       min_amount_minor, max_amount_minor, daily_limit_minor, metadata, created_at, updated_at
FROM exchange_corridors
"#;

const SELECT_SOLVER: &str = r#"
SELECT id, slug, actor_id, handle, display_name, status, countries, currencies, rails,
       min_amount_minor, max_amount_minor, fee_model, risk_score, last_seen_at, created_at, updated_at
FROM exchange_solvers
"#;

const SELECT_QUOTE: &str = r#"
SELECT id, order_id, solver_id, source_amount_minor, target_amount_minor, source_currency,
       target_currency, funding_method_type, requires_user_funding, rate::TEXT, fee_minor,
       eta_minutes, expires_at, status, settlement_plan, risk_score, score, raw_response,
       created_at, updated_at
FROM exchange_quotes
"#;

const SELECT_FUNDING_INSTRUCTION: &str = r#"
SELECT id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
       destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
FROM funding_instructions
"#;

const SELECT_SETTLEMENT: &str = r#"
SELECT id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
       funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
       failure_code, failure_message, created_at, updated_at
FROM exchange_settlements
"#;

const SELECT_PROOF: &str = r#"
SELECT id, settlement_id, solver_id, proof_type, proof_payload, verification_status,
       verified_by, verified_at, created_at
FROM exchange_proofs
"#;

pub async fn enabled_corridor(
    pool: &DbPool,
    source_country: &str,
    source_currency: &str,
    target_country: &str,
    target_currency: &str,
) -> Result<Option<ExchangeCorridor>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_CORRIDOR}
WHERE source_country = $1
  AND source_currency = $2
  AND target_country = $3
  AND target_currency = $4
  AND status = 'enabled'"
        ))
        .await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &source_country,
                &source_currency,
                &target_country,
                &target_currency,
            ],
        )
        .await?;
    Ok(row.map(row_to_corridor))
}

pub async fn corridor_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeCorridor>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_CORRIDOR} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_corridor))
}

pub async fn enabled_corridors(pool: &DbPool) -> Result<Vec<ExchangeCorridor>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_CORRIDOR} WHERE status = 'enabled' ORDER BY source_country, target_country"
        ))
        .await?;
    let rows = client.query(&stmt, &[]).await?;
    Ok(rows.into_iter().map(row_to_corridor).collect())
}

pub async fn create_order_idempotent(
    pool: &DbPool,
    order: &NewExchangeOrder,
) -> Result<ExchangeOrder> {
    if let Some(existing) =
        order_by_idempotency_key(pool, &order.user_id, &order.idempotency_key).await?
    {
        return Ok(existing);
    }

    ensure_corridor_accepts_order(pool, order).await?;

    let inserted = insert_order(pool, order).await?;
    match inserted {
        Some(order) => Ok(order),
        None => order_by_idempotency_key(pool, &order.user_id, &order.idempotency_key)
            .await?
            .context("idempotency conflict found no winning exchange order"),
    }
}

pub async fn ensure_corridor_accepts_order(pool: &DbPool, order: &NewExchangeOrder) -> Result<()> {
    let corridor = enabled_corridor(
        pool,
        &order.source_country,
        &order.source_currency,
        &order.target_country,
        &order.target_currency,
    )
    .await?
    .with_context(|| {
        format!(
            "exchange corridor is not enabled: {}/{} -> {}/{}",
            order.source_country,
            order.source_currency,
            order.target_country,
            order.target_currency
        )
    })?;

    if let Some(min) = corridor.min_amount_minor {
        if order.source_amount_minor < min {
            bail!(
                "exchange amount is below corridor minimum: {} < {}",
                order.source_amount_minor,
                min
            );
        }
    }

    if let Some(max) = corridor.max_amount_minor {
        if order.source_amount_minor > max {
            bail!(
                "exchange amount is above corridor maximum: {} > {}",
                order.source_amount_minor,
                max
            );
        }
    }

    Ok(())
}

async fn insert_order(pool: &DbPool, order: &NewExchangeOrder) -> Result<Option<ExchangeOrder>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO exchange_orders
    (user_id, idempotency_key, source_country, source_currency, source_amount_minor,
     source_method_type, source_method_ref, target_country, target_currency,
     target_amount_min_minor, target_method_type, target_method_ref, deadline_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (user_id, idempotency_key) DO NOTHING
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#,
        )
        .await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &order.user_id,
                &order.idempotency_key,
                &order.source_country,
                &order.source_currency,
                &order.source_amount_minor,
                &order.source_method_type,
                &order.source_method_ref,
                &order.target_country,
                &order.target_currency,
                &order.target_amount_min_minor,
                &order.target_method_type,
                &order.target_method_ref,
                &order.deadline_at,
            ],
        )
        .await?;
    Ok(row.map(row_to_order))
}

pub async fn order_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeOrder>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_ORDER} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_order))
}

pub async fn orders_for_user(
    pool: &DbPool,
    user_id: &Uuid,
    limit: i64,
) -> Result<Vec<ExchangeOrder>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_ORDER} WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"
        ))
        .await?;
    let rows = client.query(&stmt, &[user_id, &limit]).await?;
    Ok(rows.into_iter().map(row_to_order).collect())
}

pub async fn order_by_idempotency_key(
    pool: &DbPool,
    user_id: &Uuid,
    idempotency_key: &str,
) -> Result<Option<ExchangeOrder>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_ORDER} WHERE user_id = $1 AND idempotency_key = $2"
        ))
        .await?;
    let row = client
        .query_opt(&stmt, &[user_id, &idempotency_key])
        .await?;
    Ok(row.map(row_to_order))
}

pub async fn transition_order_status(
    pool: &DbPool,
    id: &Uuid,
    from: OrderStatus,
    to: OrderStatus,
) -> Result<bool> {
    if !from.can_transition(to) {
        bail!(
            "illegal exchange order transition: {} -> {}",
            from.as_str(),
            to.as_str()
        );
    }

    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            "UPDATE exchange_orders SET status = $2, updated_at = now() WHERE id = $1 AND status = $3",
        )
        .await?;
    let changed = client
        .execute(&stmt, &[id, &to.as_str(), &from.as_str()])
        .await?;
    Ok(changed == 1)
}

pub async fn cancel_order(pool: &DbPool, id: &Uuid, current: OrderStatus) -> Result<bool> {
    transition_order_status(pool, id, current, OrderStatus::Cancelled).await
}

pub async fn upsert_solver(pool: &DbPool, solver: &NewExchangeSolver) -> Result<ExchangeSolver> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO exchange_solvers
    (slug, actor_id, handle, display_name, status, countries, currencies, rails,
     min_amount_minor, max_amount_minor, fee_model, risk_score, last_seen_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now())
ON CONFLICT (slug) DO UPDATE SET
    actor_id = EXCLUDED.actor_id,
    handle = EXCLUDED.handle,
    display_name = EXCLUDED.display_name,
    status = EXCLUDED.status,
    countries = EXCLUDED.countries,
    currencies = EXCLUDED.currencies,
    rails = EXCLUDED.rails,
    min_amount_minor = EXCLUDED.min_amount_minor,
    max_amount_minor = EXCLUDED.max_amount_minor,
    fee_model = EXCLUDED.fee_model,
    risk_score = EXCLUDED.risk_score,
    last_seen_at = now(),
    updated_at = now()
RETURNING id, slug, actor_id, handle, display_name, status, countries, currencies, rails,
          min_amount_minor, max_amount_minor, fee_model, risk_score, last_seen_at, created_at, updated_at
"#,
        )
        .await?;
    let row = client
        .query_one(
            &stmt,
            &[
                &solver.slug,
                &solver.actor_id,
                &solver.handle,
                &solver.display_name,
                &solver.status.as_str(),
                &solver.countries,
                &solver.currencies,
                &solver.rails,
                &solver.min_amount_minor,
                &solver.max_amount_minor,
                &solver.fee_model,
                &solver.risk_score,
            ],
        )
        .await?;
    Ok(row_to_solver(row))
}

pub async fn solver_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeSolver>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_SOLVER} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_solver))
}

pub async fn solver_by_slug(pool: &DbPool, slug: &str) -> Result<Option<ExchangeSolver>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_SOLVER} WHERE slug = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[&slug]).await?;
    Ok(row.map(row_to_solver))
}

pub async fn insert_quote(pool: &DbPool, quote: &NewExchangeQuote) -> Result<ExchangeQuote> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO exchange_quotes
    (order_id, solver_id, source_amount_minor, target_amount_minor, source_currency,
     target_currency, funding_method_type, requires_user_funding, rate, fee_minor,
     eta_minutes, expires_at, status, settlement_plan, risk_score, score, raw_response)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, ($9::TEXT)::NUMERIC, $10, $11, $12, $13, $14, $15, $16, $17)
RETURNING id, order_id, solver_id, source_amount_minor, target_amount_minor, source_currency,
          target_currency, funding_method_type, requires_user_funding, rate::TEXT, fee_minor,
          eta_minutes, expires_at, status, settlement_plan, risk_score, score, raw_response,
          created_at, updated_at
"#,
        )
        .await?;
    let row = client
        .query_one(
            &stmt,
            &[
                &quote.order_id,
                &quote.solver_id,
                &quote.source_amount_minor,
                &quote.target_amount_minor,
                &quote.source_currency,
                &quote.target_currency,
                &quote.funding_method_type,
                &quote.requires_user_funding,
                &quote.rate,
                &quote.fee_minor,
                &quote.eta_minutes,
                &quote.expires_at,
                &quote.status.as_str(),
                &quote.settlement_plan,
                &quote.risk_score,
                &quote.score,
                &quote.raw_response,
            ],
        )
        .await?;
    Ok(row_to_quote(row))
}

pub async fn quote_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeQuote>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_QUOTE} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_quote))
}

pub async fn quotes_for_order(pool: &DbPool, order_id: &Uuid) -> Result<Vec<ExchangeQuote>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_QUOTE} WHERE order_id = $1 ORDER BY created_at"
        ))
        .await?;
    let rows = client.query(&stmt, &[order_id]).await?;
    Ok(rows.into_iter().map(row_to_quote).collect())
}

pub async fn insert_funding_instruction(
    pool: &DbPool,
    instruction: &NewFundingInstruction,
) -> Result<FundingInstruction> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO funding_instructions
    (order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
     destination_ref, expires_at, raw_payload)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#,
        )
        .await?;
    let row = client
        .query_one(
            &stmt,
            &[
                &instruction.order_id,
                &instruction.quote_id,
                &instruction.solver_id,
                &instruction.status.as_str(),
                &instruction.method_type,
                &instruction.amount_minor,
                &instruction.currency,
                &instruction.destination_ref,
                &instruction.expires_at,
                &instruction.raw_payload,
            ],
        )
        .await?;
    Ok(row_to_funding_instruction(row))
}

pub async fn funding_instruction_by_id(
    pool: &DbPool,
    id: &Uuid,
) -> Result<Option<FundingInstruction>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_FUNDING_INSTRUCTION} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_funding_instruction))
}

pub async fn funding_instructions_for_order(
    pool: &DbPool,
    order_id: &Uuid,
) -> Result<Vec<FundingInstruction>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_FUNDING_INSTRUCTION} WHERE order_id = $1 ORDER BY created_at"
        ))
        .await?;
    let rows = client.query(&stmt, &[order_id]).await?;
    Ok(rows.into_iter().map(row_to_funding_instruction).collect())
}

pub async fn create_funding_instruction_for_quote(
    pool: &DbPool,
    order: &ExchangeOrder,
    quote: &ExchangeQuote,
) -> Result<(ExchangeOrder, FundingInstruction)> {
    if order.id != quote.order_id {
        bail!("quote does not belong to exchange order");
    }
    if order.status != OrderStatus::Quoted {
        bail!("exchange order must be quoted before confirm");
    }
    if let Some(selected_quote_id) = order.selected_quote_id {
        if selected_quote_id != quote.id {
            bail!("exchange order already selected a different quote");
        }
    }
    if quote.expires_at <= Utc::now() {
        bail!("exchange quote expired");
    }

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let instruction_row = tx
        .query_one(
            r#"
INSERT INTO funding_instructions
    (order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
     destination_ref, expires_at, raw_payload)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#,
            &[
                &order.id,
                &quote.id,
                &quote.solver_id,
                &FundingInstructionStatus::ShownToUser.as_str(),
                &quote.funding_method_type,
                &quote.source_amount_minor,
                &quote.source_currency,
                &format!("solver:{}:{}", quote.solver_id, quote.id),
                &quote.expires_at,
                &serde_json::json!({
                    "order_id": order.id,
                    "quote_id": quote.id,
                    "solver_id": quote.solver_id,
                    "display_text": "Confirm funding for the selected exchange route",
                    "requires_user_funding": quote.requires_user_funding,
                }),
            ],
        )
        .await?;
    let instruction = row_to_funding_instruction(instruction_row);

    tx.execute(
        "UPDATE exchange_quotes SET status = $2, updated_at = now() WHERE id = $1",
        &[&quote.id, &QuoteStatus::Selected.as_str()],
    )
    .await?;

    let order_row = tx
        .query_opt(
            r#"
UPDATE exchange_orders
SET funding_instruction_id = $2,
    funding_status = $3,
    selected_quote_id = $4,
    status = $5,
    updated_at = now()
WHERE id = $1 AND status = $6
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#,
            &[
                &order.id,
                &instruction.id,
                &FundingInstructionStatus::ShownToUser.as_str(),
                &quote.id,
                &OrderStatus::Locked.as_str(),
                &OrderStatus::Quoted.as_str(),
            ],
        )
        .await?
        .context("exchange order was not quoted at confirm time")?;
    let updated_order = row_to_order(order_row);

    tx.commit().await?;
    Ok((updated_order, instruction))
}

pub async fn confirm_funding_instruction(
    pool: &DbPool,
    order: &ExchangeOrder,
    instruction: &FundingInstruction,
) -> Result<(ExchangeOrder, FundingInstruction)> {
    if order.id != instruction.order_id {
        bail!("funding instruction does not belong to exchange order");
    }
    if instruction.status == FundingInstructionStatus::UserConfirmed {
        return Ok((order.clone(), instruction.clone()));
    }
    if instruction.status != FundingInstructionStatus::ShownToUser {
        bail!("funding instruction must be shown to user before confirmation");
    }
    if instruction.expires_at <= Utc::now() {
        bail!("funding instruction expired");
    }

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let instruction_row = tx
        .query_one(
            r#"
UPDATE funding_instructions
SET status = $2, user_confirmed_at = now(), updated_at = now()
WHERE id = $1 AND status = $3
RETURNING id, order_id, quote_id, solver_id, status, method_type, amount_minor, currency,
          destination_ref, expires_at, user_confirmed_at, raw_payload, created_at, updated_at
"#,
            &[
                &instruction.id,
                &FundingInstructionStatus::UserConfirmed.as_str(),
                &FundingInstructionStatus::ShownToUser.as_str(),
            ],
        )
        .await?;
    let updated_instruction = row_to_funding_instruction(instruction_row);

    let order_row = tx
        .query_one(
            r#"
UPDATE exchange_orders
SET funding_status = $2, updated_at = now()
WHERE id = $1
RETURNING id, user_id, idempotency_key, source_country, source_currency, source_amount_minor,
          source_method_type, source_method_ref, target_country, target_currency,
          target_amount_min_minor, target_method_type, target_method_ref,
          funding_instruction_id, funding_status, status, deadline_at, selected_quote_id,
          failure_code, failure_message, created_at, updated_at
"#,
            &[&order.id, &FundingInstructionStatus::UserConfirmed.as_str()],
        )
        .await?;
    let updated_order = row_to_order(order_row);

    tx.commit().await?;
    Ok((updated_order, updated_instruction))
}

pub async fn insert_settlement(
    pool: &DbPool,
    settlement: &NewExchangeSettlement,
) -> Result<ExchangeSettlement> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO exchange_settlements
    (order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
     funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
RETURNING id, order_id, quote_id, solver_id, status, token_leg_status, money_leg_status,
          funding_status, pay3flow_wallet_ref, token_ledger_ref, money_reference, proof_id,
          failure_code, failure_message, created_at, updated_at
"#,
        )
        .await?;
    let row = client
        .query_one(
            &stmt,
            &[
                &settlement.order_id,
                &settlement.quote_id,
                &settlement.solver_id,
                &settlement.status.as_str(),
                &settlement.token_leg_status.as_str(),
                &settlement.money_leg_status.as_str(),
                &settlement.funding_status.as_str(),
                &settlement.pay3flow_wallet_ref,
                &settlement.token_ledger_ref,
                &settlement.money_reference,
            ],
        )
        .await?;
    Ok(row_to_settlement(row))
}

pub async fn settlement_by_order_id(
    pool: &DbPool,
    order_id: &Uuid,
) -> Result<Option<ExchangeSettlement>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_SETTLEMENT} WHERE order_id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[order_id]).await?;
    Ok(row.map(row_to_settlement))
}

pub async fn settlement_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeSettlement>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_SETTLEMENT} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_settlement))
}

pub async fn insert_proof(pool: &DbPool, proof: &NewExchangeProof) -> Result<ExchangeProof> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO exchange_proofs
    (settlement_id, solver_id, proof_type, proof_payload, verification_status, verified_by)
VALUES ($1, $2, $3, $4, $5, $6)
RETURNING id, settlement_id, solver_id, proof_type, proof_payload, verification_status,
          verified_by, verified_at, created_at
"#,
        )
        .await?;
    let row = client
        .query_one(
            &stmt,
            &[
                &proof.settlement_id,
                &proof.solver_id,
                &proof.proof_type,
                &proof.proof_payload,
                &proof.verification_status.as_str(),
                &proof.verified_by,
            ],
        )
        .await?;
    Ok(row_to_proof(row))
}

pub async fn proof_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<ExchangeProof>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_PROOF} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_proof))
}

pub async fn proofs_for_settlement(
    pool: &DbPool,
    settlement_id: &Uuid,
) -> Result<Vec<ExchangeProof>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_PROOF} WHERE settlement_id = $1 ORDER BY created_at"
        ))
        .await?;
    let rows = client.query(&stmt, &[settlement_id]).await?;
    Ok(rows.into_iter().map(row_to_proof).collect())
}

pub async fn insert_audit_event(pool: &DbPool, event: &NewAuditEvent) -> Result<AuditEvent> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
INSERT INTO audit_events
    (entity_type, entity_id, event_type, actor_type, actor_id, payload)
VALUES ($1, $2, $3, $4, $5, $6)
RETURNING id, entity_type, entity_id, event_type, actor_type, actor_id, payload, created_at
"#,
        )
        .await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &event.entity_type,
                &event.entity_id,
                &event.event_type,
                &event.actor_type,
                &event.actor_id,
                &event.payload,
            ],
        )
        .await?
        .context("audit event insert returned no row")?;
    Ok(row_to_audit_event(row))
}

pub async fn audit_events_for_entity(
    pool: &DbPool,
    entity_type: &str,
    entity_id: &Uuid,
) -> Result<Vec<AuditEvent>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            r#"
SELECT id, entity_type, entity_id, event_type, actor_type, actor_id, payload, created_at
FROM audit_events
WHERE entity_type = $1 AND entity_id = $2
ORDER BY created_at
"#,
        )
        .await?;
    let rows = client.query(&stmt, &[&entity_type, entity_id]).await?;
    Ok(rows.into_iter().map(row_to_audit_event).collect())
}

fn row_to_corridor(row: tokio_postgres::Row) -> ExchangeCorridor {
    ExchangeCorridor {
        id: row.get(0),
        source_country: row.get(1),
        source_currency: row.get(2),
        target_country: row.get(3),
        target_currency: row.get(4),
        status: row.get(5),
        min_amount_minor: row.try_get::<_, Option<Minor>>(6).ok().flatten(),
        max_amount_minor: row.try_get::<_, Option<Minor>>(7).ok().flatten(),
        daily_limit_minor: row.try_get::<_, Option<Minor>>(8).ok().flatten(),
        metadata: row.get(9),
        created_at: row.get::<_, DateTime<Utc>>(10),
        updated_at: row.get::<_, DateTime<Utc>>(11),
    }
}

fn row_to_order(row: tokio_postgres::Row) -> ExchangeOrder {
    ExchangeOrder {
        id: row.get(0),
        user_id: row.get(1),
        idempotency_key: row.get(2),
        source_country: row.get(3),
        source_currency: row.get(4),
        source_amount_minor: row.get::<_, Minor>(5),
        source_method_type: row.get(6),
        source_method_ref: row.try_get::<_, Option<String>>(7).ok().flatten(),
        target_country: row.get(8),
        target_currency: row.get(9),
        target_amount_min_minor: row.try_get::<_, Option<Minor>>(10).ok().flatten(),
        target_method_type: row.get(11),
        target_method_ref: row.try_get::<_, Option<String>>(12).ok().flatten(),
        funding_instruction_id: row.try_get::<_, Option<Uuid>>(13).ok().flatten(),
        funding_status: FundingInstructionStatus::parse(&row.get::<_, String>(14))
            .unwrap_or(FundingInstructionStatus::NotStarted),
        status: OrderStatus::parse(&row.get::<_, String>(15)).unwrap_or(OrderStatus::Created),
        deadline_at: row.try_get::<_, Option<DateTime<Utc>>>(16).ok().flatten(),
        selected_quote_id: row.try_get::<_, Option<Uuid>>(17).ok().flatten(),
        failure_code: row.try_get::<_, Option<String>>(18).ok().flatten(),
        failure_message: row.try_get::<_, Option<String>>(19).ok().flatten(),
        created_at: row.get::<_, DateTime<Utc>>(20),
        updated_at: row.get::<_, DateTime<Utc>>(21),
    }
}

fn row_to_solver(row: tokio_postgres::Row) -> ExchangeSolver {
    ExchangeSolver {
        id: row.get(0),
        slug: row.get(1),
        actor_id: row.try_get::<_, Option<String>>(2).ok().flatten(),
        handle: row.try_get::<_, Option<String>>(3).ok().flatten(),
        display_name: row.get(4),
        status: SolverStatus::parse(&row.get::<_, String>(5)).unwrap_or(SolverStatus::Discovered),
        countries: row.get(6),
        currencies: row.get(7),
        rails: row.get(8),
        min_amount_minor: row.try_get::<_, Option<Minor>>(9).ok().flatten(),
        max_amount_minor: row.try_get::<_, Option<Minor>>(10).ok().flatten(),
        fee_model: row.get(11),
        risk_score: row.get(12),
        last_seen_at: row.try_get::<_, Option<DateTime<Utc>>>(13).ok().flatten(),
        created_at: row.get::<_, DateTime<Utc>>(14),
        updated_at: row.get::<_, DateTime<Utc>>(15),
    }
}

fn row_to_quote(row: tokio_postgres::Row) -> ExchangeQuote {
    ExchangeQuote {
        id: row.get(0),
        order_id: row.get(1),
        solver_id: row.get(2),
        source_amount_minor: row.get::<_, Minor>(3),
        target_amount_minor: row.get::<_, Minor>(4),
        source_currency: row.get(5),
        target_currency: row.get(6),
        funding_method_type: row.get(7),
        requires_user_funding: row.get(8),
        rate: row.get(9),
        fee_minor: row.get::<_, Minor>(10),
        eta_minutes: row.get(11),
        expires_at: row.get::<_, DateTime<Utc>>(12),
        status: QuoteStatus::parse(&row.get::<_, String>(13)).unwrap_or(QuoteStatus::Received),
        settlement_plan: row.get(14),
        risk_score: row.get(15),
        score: row.try_get::<_, Option<i64>>(16).ok().flatten(),
        raw_response: row
            .try_get::<_, Option<serde_json::Value>>(17)
            .ok()
            .flatten(),
        created_at: row.get::<_, DateTime<Utc>>(18),
        updated_at: row.get::<_, DateTime<Utc>>(19),
    }
}

fn row_to_funding_instruction(row: tokio_postgres::Row) -> FundingInstruction {
    FundingInstruction {
        id: row.get(0),
        order_id: row.get(1),
        quote_id: row.get(2),
        solver_id: row.get(3),
        status: FundingInstructionStatus::parse(&row.get::<_, String>(4))
            .unwrap_or(FundingInstructionStatus::Created),
        method_type: row.get(5),
        amount_minor: row.get::<_, Minor>(6),
        currency: row.get(7),
        destination_ref: row.get(8),
        expires_at: row.get::<_, DateTime<Utc>>(9),
        user_confirmed_at: row.try_get::<_, Option<DateTime<Utc>>>(10).ok().flatten(),
        raw_payload: row.get(11),
        created_at: row.get::<_, DateTime<Utc>>(12),
        updated_at: row.get::<_, DateTime<Utc>>(13),
    }
}

fn row_to_settlement(row: tokio_postgres::Row) -> ExchangeSettlement {
    ExchangeSettlement {
        id: row.get(0),
        order_id: row.get(1),
        quote_id: row.get(2),
        solver_id: row.get(3),
        status: SettlementStatus::parse(&row.get::<_, String>(4))
            .unwrap_or(SettlementStatus::Created),
        token_leg_status: LegStatus::parse(&row.get::<_, String>(5))
            .unwrap_or(LegStatus::NotStarted),
        money_leg_status: LegStatus::parse(&row.get::<_, String>(6))
            .unwrap_or(LegStatus::NotStarted),
        funding_status: FundingInstructionStatus::parse(&row.get::<_, String>(7))
            .unwrap_or(FundingInstructionStatus::NotStarted),
        pay3flow_wallet_ref: row.try_get::<_, Option<String>>(8).ok().flatten(),
        token_ledger_ref: row.try_get::<_, Option<String>>(9).ok().flatten(),
        money_reference: row.try_get::<_, Option<String>>(10).ok().flatten(),
        proof_id: row.try_get::<_, Option<Uuid>>(11).ok().flatten(),
        failure_code: row.try_get::<_, Option<String>>(12).ok().flatten(),
        failure_message: row.try_get::<_, Option<String>>(13).ok().flatten(),
        created_at: row.get::<_, DateTime<Utc>>(14),
        updated_at: row.get::<_, DateTime<Utc>>(15),
    }
}

fn row_to_proof(row: tokio_postgres::Row) -> ExchangeProof {
    ExchangeProof {
        id: row.get(0),
        settlement_id: row.get(1),
        solver_id: row.get(2),
        proof_type: row.get(3),
        proof_payload: row.get(4),
        verification_status: ProofVerificationStatus::parse(&row.get::<_, String>(5))
            .unwrap_or(ProofVerificationStatus::Pending),
        verified_by: row.try_get::<_, Option<String>>(6).ok().flatten(),
        verified_at: row.try_get::<_, Option<DateTime<Utc>>>(7).ok().flatten(),
        created_at: row.get::<_, DateTime<Utc>>(8),
    }
}

fn row_to_audit_event(row: tokio_postgres::Row) -> AuditEvent {
    AuditEvent {
        id: row.get(0),
        entity_type: row.get(1),
        entity_id: row.get(2),
        event_type: row.get(3),
        actor_type: row.get(4),
        actor_id: row.try_get::<_, Option<String>>(5).ok().flatten(),
        payload: row.get(6),
        created_at: row.get::<_, DateTime<Utc>>(7),
    }
}

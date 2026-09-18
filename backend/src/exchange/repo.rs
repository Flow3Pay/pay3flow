use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::model::{
    AuditEvent, ExchangeCorridor, ExchangeOrder, Minor, NewAuditEvent, NewExchangeOrder,
};
use crate::exchange::status::{FundingInstructionStatus, OrderStatus};

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

pub async fn create_order_idempotent(
    pool: &DbPool,
    order: &NewExchangeOrder,
) -> Result<ExchangeOrder> {
    let inserted = insert_order(pool, order).await?;
    match inserted {
        Some(order) => Ok(order),
        None => order_by_idempotency_key(pool, &order.user_id, &order.idempotency_key)
            .await?
            .context("idempotency conflict found no winning exchange order"),
    }
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
    let row = client.query_opt(&stmt, &[user_id, &idempotency_key]).await?;
    Ok(row.map(row_to_order))
}

pub async fn transition_order_status(
    pool: &DbPool,
    id: &Uuid,
    from: OrderStatus,
    to: OrderStatus,
) -> Result<bool> {
    if !from.can_transition(to) {
        bail!("illegal exchange order transition: {} -> {}", from.as_str(), to.as_str());
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

//! Exchange kill-switch, daily-limit, manual-review and consent statements.

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::db::repo::reputation::Conn;

const SELECT_CONTROLS: &str = r#"
SELECT exchange_enabled, user_daily_limit_minor, solver_daily_limit_minor,
       manual_review_threshold_minor, terms_version, updated_by, updated_at
FROM exchange_controls WHERE singleton = TRUE
"#;
const UPDATE_CONTROLS: &str = r#"
UPDATE exchange_controls
SET exchange_enabled = $1,
    user_daily_limit_minor = $2,
    solver_daily_limit_minor = $3,
    manual_review_threshold_minor = $4,
    terms_version = $5,
    updated_by = $6,
    updated_at = now()
WHERE singleton = TRUE
RETURNING exchange_enabled, user_daily_limit_minor, solver_daily_limit_minor,
          manual_review_threshold_minor, terms_version, updated_by, updated_at
"#;
const ENABLED_CORRIDOR_EXISTS: &str = r#"
SELECT 1 FROM exchange_corridors
WHERE source_country = $1 AND source_currency = $2
  AND target_country = $3 AND target_currency = $4
  AND status = 'enabled'
"#;
const USER_DAILY_USAGE: &str = r#"
SELECT COALESCE(SUM(source_amount_minor), 0)::BIGINT
FROM exchange_orders
WHERE user_id = $1
  AND created_at >= date_trunc('day', now())
  AND status NOT IN ('cancelled', 'failed', 'expired')
"#;
const SOLVER_STATUS: &str = "SELECT status FROM exchange_solvers WHERE id = $1";
const SOLVER_DAILY_USAGE: &str = r#"
SELECT COALESCE(SUM(o.source_amount_minor), 0)::BIGINT
FROM exchange_orders o
JOIN exchange_quotes q ON q.id = o.selected_quote_id
WHERE q.solver_id = $1
  AND o.created_at >= date_trunc('day', now())
  AND o.status NOT IN ('created', 'discovering', 'quoting', 'quoted', 'cancelled', 'failed', 'expired')
"#;
const INSERT_MANUAL_REVIEW: &str = r#"
INSERT INTO exchange_manual_reviews (order_id, status, reason)
VALUES ($1, 'pending', $2)
ON CONFLICT (order_id) DO UPDATE SET updated_at = exchange_manual_reviews.updated_at
RETURNING order_id, status, reason, resolved_by, resolution_note, created_at, resolved_at, updated_at
"#;
const MANUAL_REVIEW_FOR_ORDER: &str = r#"
SELECT order_id, status, reason, resolved_by, resolution_note, created_at, resolved_at, updated_at
FROM exchange_manual_reviews WHERE order_id = $1
"#;
const RESOLVE_MANUAL_REVIEW: &str = r#"
UPDATE exchange_manual_reviews
SET status = $2, resolved_by = $3, resolution_note = $4,
    resolved_at = now(), updated_at = now()
WHERE order_id = $1 AND status = 'pending'
RETURNING order_id, status, reason, resolved_by, resolution_note, created_at, resolved_at, updated_at
"#;
const SET_CORRIDOR_STATUS: &str =
    "UPDATE exchange_corridors SET status = $2, updated_at = now() WHERE id = $1";
const SET_SOLVER_STATUS: &str =
    "UPDATE exchange_solvers SET status = $2, updated_at = now() WHERE id = $1";
const RESOLVE_SETTLEMENT_DISPUTE: &str = r#"
UPDATE exchange_settlements
SET status = $2,
    failure_code = CASE WHEN $2 = 'done' THEN NULL ELSE 'manual_resolution_failed' END,
    failure_message = CASE WHEN $2 = 'done' THEN NULL ELSE $3 END,
    updated_at = now()
WHERE order_id = $1 AND status = 'disputed'
"#;
const RESOLVE_ORDER_DISPUTE: &str = r#"
UPDATE exchange_orders
SET status = $2,
    failure_code = CASE WHEN $2 = 'done' THEN NULL ELSE 'manual_resolution_failed' END,
    failure_message = CASE WHEN $2 = 'done' THEN NULL ELSE $3 END,
    updated_at = now()
WHERE id = $1 AND status = 'disputed'
"#;
const INSERT_CONSENT: &str = r#"
INSERT INTO exchange_consents
    (order_id, user_id, terms_version, settlement_asset_disclosure)
VALUES ($1, $2, $3, TRUE)
ON CONFLICT (order_id, terms_version) DO NOTHING
"#;

pub async fn controls<C>(client: &C) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client.query_one(SELECT_CONTROLS, &[]).await?)
}

pub async fn update_controls<C>(
    client: &C,
    exchange_enabled: bool,
    user_daily_limit_minor: i64,
    solver_daily_limit_minor: i64,
    manual_review_threshold_minor: &Option<i64>,
    terms_version: &str,
    actor: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            UPDATE_CONTROLS,
            &[
                &exchange_enabled,
                &user_daily_limit_minor,
                &solver_daily_limit_minor,
                manual_review_threshold_minor,
                &terms_version,
                &actor,
            ],
        )
        .await?)
}

pub async fn enabled_corridor_exists<C>(
    client: &C,
    source_country: &str,
    source_currency: &str,
    target_country: &str,
    target_currency: &str,
) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(
            ENABLED_CORRIDOR_EXISTS,
            &[
                &source_country,
                &source_currency,
                &target_country,
                &target_currency,
            ],
        )
        .await?
        .is_some())
}

pub async fn user_daily_usage_minor<C>(client: &C, user_id: &Uuid) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client.query_one(USER_DAILY_USAGE, &[user_id]).await?.get(0))
}

/// `Err` when the solver row does not exist.
pub async fn solver_status<C>(client: &C, solver_id: &Uuid) -> Result<String>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(SOLVER_STATUS, &[solver_id])
        .await?
        .context("exchange solver not found")?
        .get(0))
}

pub async fn solver_daily_usage_minor<C>(client: &C, solver_id: &Uuid) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(SOLVER_DAILY_USAGE, &[solver_id])
        .await?
        .get(0))
}

pub async fn insert_manual_review<C>(
    client: &C,
    order_id: &Uuid,
    reason: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(INSERT_MANUAL_REVIEW, &[order_id, &reason])
        .await?)
}

pub async fn manual_review_for_order<C>(
    client: &C,
    order_id: &Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(MANUAL_REVIEW_FOR_ORDER, &[order_id])
        .await?)
}

/// `Err` when no pending review matched.
pub async fn resolve_manual_review<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    actor: &str,
    note: &Option<&str>,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(RESOLVE_MANUAL_REVIEW, &[order_id, &status, &actor, note])
        .await?
        .context("pending manual review not found")?)
}

pub async fn set_corridor_enabled<C>(client: &C, corridor_id: &Uuid, status: &str) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(SET_CORRIDOR_STATUS, &[corridor_id, &status])
        .await?)
}

pub async fn set_solver_status<C>(client: &C, solver_id: &Uuid, status: &str) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(SET_SOLVER_STATUS, &[solver_id, &status])
        .await?)
}

pub async fn resolve_settlement_dispute<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    note: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(RESOLVE_SETTLEMENT_DISPUTE, &[order_id, &status, &note])
        .await?;
    Ok(())
}

pub async fn resolve_order_dispute<C>(
    client: &C,
    order_id: &Uuid,
    status: &str,
    note: &str,
) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(RESOLVE_ORDER_DISPUTE, &[order_id, &status, &note])
        .await?)
}

pub async fn insert_consent<C>(
    client: &C,
    order_id: &Uuid,
    user_id: &Uuid,
    terms_version: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(INSERT_CONSENT, &[order_id, user_id, &terms_version])
        .await?;
    Ok(())
}

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::repo::exchange_control;
use crate::db::DbPool;
use crate::exchange::model::{ExchangeOrder, ExchangeQuote, Minor};
use crate::exchange::repo;
use crate::exchange::status::{OrderStatus, SettlementStatus, SolverStatus};

#[derive(Debug, Clone, Serialize)]
pub struct ExchangeControls {
    pub exchange_enabled: bool,
    pub user_daily_limit_minor: Minor,
    pub solver_daily_limit_minor: Minor,
    pub manual_review_threshold_minor: Option<Minor>,
    pub terms_version: String,
    pub updated_by: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExchangeControlsPatch {
    pub exchange_enabled: Option<bool>,
    pub user_daily_limit_minor: Option<Minor>,
    pub solver_daily_limit_minor: Option<Minor>,
    pub manual_review_threshold_minor: Option<Minor>,
    pub clear_manual_review_threshold: Option<bool>,
    pub terms_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManualReview {
    pub order_id: Uuid,
    pub status: String,
    pub reason: String,
    pub resolved_by: Option<String>,
    pub resolution_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

pub async fn controls(pool: &DbPool) -> Result<ExchangeControls> {
    let client = pool.get().await?;
    let row = exchange_control::controls(&client).await?;
    Ok(row_to_controls(row))
}

pub async fn update_controls(
    pool: &DbPool,
    patch: &ExchangeControlsPatch,
    actor: &str,
) -> Result<ExchangeControls> {
    if matches!(patch.user_daily_limit_minor, Some(value) if value <= 0)
        || matches!(patch.solver_daily_limit_minor, Some(value) if value <= 0)
        || matches!(patch.manual_review_threshold_minor, Some(value) if value <= 0)
    {
        bail!("exchange limits must be greater than zero");
    }
    if matches!(patch.terms_version.as_deref(), Some(value) if value.trim().is_empty()) {
        bail!("terms_version must not be empty");
    }

    let current = controls(pool).await?;
    let manual_review_threshold = if patch.clear_manual_review_threshold.unwrap_or(false) {
        None
    } else {
        patch
            .manual_review_threshold_minor
            .or(current.manual_review_threshold_minor)
    };
    let terms_version = patch
        .terms_version
        .as_deref()
        .map(str::trim)
        .unwrap_or(&current.terms_version);
    let client = pool.get().await?;
    let row = exchange_control::update_controls(
        &client,
        patch.exchange_enabled.unwrap_or(current.exchange_enabled),
        patch
            .user_daily_limit_minor
            .unwrap_or(current.user_daily_limit_minor),
        patch
            .solver_daily_limit_minor
            .unwrap_or(current.solver_daily_limit_minor),
        &manual_review_threshold,
        terms_version,
        actor,
    )
    .await?;
    Ok(row_to_controls(row))
}

pub async fn ensure_global_enabled(pool: &DbPool) -> Result<ExchangeControls> {
    let settings = controls(pool).await?;
    if !settings.exchange_enabled {
        bail!("exchange flow is disabled by the global kill switch");
    }
    Ok(settings)
}

pub async fn ensure_order_allowed(
    pool: &DbPool,
    order: &ExchangeOrder,
) -> Result<ExchangeControls> {
    let settings = ensure_global_enabled(pool).await?;
    let client = pool.get().await?;
    let enabled = exchange_control::enabled_corridor_exists(
        &client,
        &order.source_country,
        &order.source_currency,
        &order.target_country,
        &order.target_currency,
    )
    .await?;
    if !enabled {
        bail!("exchange corridor is disabled by its kill switch");
    }
    Ok(settings)
}

pub async fn ensure_user_daily_limit(
    pool: &DbPool,
    user_id: &Uuid,
    amount_minor: Minor,
    corridor_limit_minor: Option<Minor>,
) -> Result<()> {
    let settings = ensure_global_enabled(pool).await?;
    let client = pool.get().await?;
    let used: Minor = exchange_control::user_daily_usage_minor(&client, user_id).await?;
    let limit = corridor_limit_minor
        .map(|corridor_limit| corridor_limit.min(settings.user_daily_limit_minor))
        .unwrap_or(settings.user_daily_limit_minor);
    if used.saturating_add(amount_minor) > limit {
        bail!("exchange user daily limit exceeded");
    }
    Ok(())
}

pub async fn ensure_solver_allowed(
    pool: &DbPool,
    order: &ExchangeOrder,
    quote: &ExchangeQuote,
) -> Result<()> {
    let settings = ensure_order_allowed(pool, order).await?;
    let client = pool.get().await?;
    let status = exchange_control::solver_status(&client, &quote.solver_id).await?;
    if status != "active" {
        bail!("exchange solver is disabled or unavailable");
    }
    let used: Minor = exchange_control::solver_daily_usage_minor(&client, &quote.solver_id).await?;
    if used.saturating_add(order.source_amount_minor) > settings.solver_daily_limit_minor {
        bail!("exchange solver daily limit exceeded");
    }
    Ok(())
}

pub async fn ensure_or_create_manual_review(
    pool: &DbPool,
    order: &ExchangeOrder,
) -> Result<Option<ManualReview>> {
    let settings = ensure_order_allowed(pool, order).await?;
    let Some(threshold) = settings.manual_review_threshold_minor else {
        return Ok(None);
    };
    if order.source_amount_minor < threshold {
        return Ok(None);
    }
    let client = pool.get().await?;
    let row = exchange_control::insert_manual_review(
        &client,
        &order.id,
        &format!("amount reaches manual review threshold {threshold}"),
    )
    .await?;
    Ok(Some(row_to_manual_review(row)))
}

pub async fn manual_review_for_order(
    pool: &DbPool,
    order_id: &Uuid,
) -> Result<Option<ManualReview>> {
    let client = pool.get().await?;
    let row = exchange_control::manual_review_for_order(&client, order_id).await?;
    Ok(row.map(row_to_manual_review))
}

pub async fn require_manual_review_approval(pool: &DbPool, order_id: &Uuid) -> Result<()> {
    if let Some(review) = manual_review_for_order(pool, order_id).await? {
        match review.status.as_str() {
            "approved" => {}
            "pending" => bail!("exchange order is pending manual review"),
            "rejected" => bail!("exchange order was rejected by manual review"),
            _ => bail!("exchange order has an invalid manual review status"),
        }
    }
    Ok(())
}

pub async fn resolve_manual_review(
    pool: &DbPool,
    order_id: &Uuid,
    approved: bool,
    actor: &str,
    note: Option<&str>,
) -> Result<ManualReview> {
    let status = if approved { "approved" } else { "rejected" };
    let client = pool.get().await?;
    let row =
        exchange_control::resolve_manual_review(&client, order_id, status, actor, &note).await?;
    Ok(row_to_manual_review(row))
}

pub async fn set_corridor_enabled(pool: &DbPool, corridor_id: &Uuid, enabled: bool) -> Result<()> {
    let client = pool.get().await?;
    let status = if enabled { "enabled" } else { "disabled" };
    let changed = exchange_control::set_corridor_enabled(&client, corridor_id, status).await?;
    if changed != 1 {
        bail!("exchange corridor not found");
    }
    Ok(())
}

pub async fn set_solver_status(
    pool: &DbPool,
    solver_id: &Uuid,
    status: SolverStatus,
) -> Result<()> {
    let client = pool.get().await?;
    let changed = exchange_control::set_solver_status(&client, solver_id, status.as_str()).await?;
    if changed != 1 {
        bail!("exchange solver not found");
    }
    Ok(())
}

pub async fn resolve_dispute(
    pool: &DbPool,
    order: &ExchangeOrder,
    outcome: OrderStatus,
    actor: &str,
    note: &str,
) -> Result<ExchangeOrder> {
    if order.status != OrderStatus::Disputed {
        bail!("exchange order must be disputed before manual resolution");
    }
    if !matches!(outcome, OrderStatus::Done | OrderStatus::Failed) {
        bail!("manual dispute outcome must be done or failed");
    }
    let settlement_status = if outcome == OrderStatus::Done {
        SettlementStatus::Done
    } else {
        SettlementStatus::Failed
    };
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    exchange_control::resolve_settlement_dispute(&tx, &order.id, settlement_status.as_str(), note)
        .await?;
    let changed =
        exchange_control::resolve_order_dispute(&tx, &order.id, outcome.as_str(), note).await?;
    if changed != 1 {
        bail!("disputed exchange order changed during manual resolution");
    }
    if outcome == OrderStatus::Done {
        crate::referrals::credit_completed_exchange(&tx, order).await?;
    }
    tx.commit().await?;
    let updated = repo::order_by_id(pool, &order.id)
        .await?
        .context("exchange order not found after manual resolution")?;
    repo::audit_order_event(
        pool,
        &updated,
        "exchange.dispute.manually_resolved",
        serde_json::json!({
            "outcome": outcome.as_str(),
            "actor": actor,
            "note": note,
        }),
    )
    .await?;
    Ok(updated)
}

pub async fn record_consent(
    pool: &DbPool,
    order: &ExchangeOrder,
    terms_version: &str,
) -> Result<()> {
    let settings = ensure_order_allowed(pool, order).await?;
    if terms_version != settings.terms_version {
        bail!("exchange terms version is stale; reload the funding instruction");
    }
    let client = pool.get().await?;
    exchange_control::insert_consent(&client, &order.id, &order.user_id, terms_version).await?;
    Ok(())
}

fn row_to_controls(row: tokio_postgres::Row) -> ExchangeControls {
    ExchangeControls {
        exchange_enabled: row.get(0),
        user_daily_limit_minor: row.get(1),
        solver_daily_limit_minor: row.get(2),
        manual_review_threshold_minor: row.get(3),
        terms_version: row.get(4),
        updated_by: row.get(5),
        updated_at: row.get(6),
    }
}

fn row_to_manual_review(row: tokio_postgres::Row) -> ManualReview {
    ManualReview {
        order_id: row.get(0),
        status: row.get(1),
        reason: row.get(2),
        resolved_by: row.get(3),
        resolution_note: row.get(4),
        created_at: row.get(5),
        resolved_at: row.get(6),
        updated_at: row.get(7),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_patch_defaults_to_no_changes() {
        let patch = ExchangeControlsPatch::default();
        assert!(patch.exchange_enabled.is_none());
        assert!(patch.user_daily_limit_minor.is_none());
        assert!(!patch.clear_manual_review_threshold.unwrap_or(false));
    }
}

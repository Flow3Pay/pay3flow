use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const OPEN_TRADES: &str = "SELECT count(*) FROM otc_trades WHERE state NOT IN ('completed','refunded','rejected','expired')";
const INITIALIZE_MODE: &str = "UPDATE otc_controls SET demo=$1 WHERE singleton AND demo IS NULL";
const MODE: &str = "SELECT demo FROM otc_controls WHERE singleton";
const RECORD_INVENTORY: &str = "INSERT INTO otc_inventory(chain,balance,gas_reserve,observed_at,snapshot) VALUES($1,$2::text::numeric,$3::text::numeric,now(),$4) ON CONFLICT(chain) DO UPDATE SET balance=$2::text::numeric,gas_reserve=$3::text::numeric,observed_at=now(),snapshot=$4";
const RELEASE_EVIDENCE_DEBITS: &str = "UPDATE otc_reservations r SET status='released' WHERE chain=$1 AND status='debit' AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=r.trade_id AND e.kind IN ('payout','refund') AND e.valid AND e.included_at<=$2)";
const RELEASE_ESCROW_DEBITS: &str = "UPDATE otc_reservations r SET status='released' WHERE chain='ethereum' AND $1='ethereum' AND status='debit' AND EXISTS(SELECT 1 FROM otc_escrow_evidence e WHERE e.trade_id=r.trade_id AND e.kind='funding' AND e.valid AND e.included_at<=$2)";
const RECORD_SCAN: &str = "UPDATE otc_observers SET checkpoint=$2,last_scan=now(),failures=0,lag=$3,paused=FALSE,last_error=NULL WHERE chain=$1";
const RECORD_SCAN_FAILURE: &str =
    "UPDATE otc_observers SET failures=failures+1,paused=TRUE,last_error='scan failed' WHERE chain=$1";
const REVALIDATE: &str = "SELECT trade_id,kind,evidence FROM otc_evidence WHERE valid ORDER BY last_checked_at ASC NULLS FIRST LIMIT 100";
const CHECKED_EVIDENCE: &str =
    "UPDATE otc_evidence SET last_checked_at=now() WHERE trade_id=$1 AND kind=$2";
const INVALIDATE_EVIDENCE: &str =
    "UPDATE otc_evidence SET valid=FALSE WHERE trade_id=$1 AND kind=$2";
const REVIEW_TRADE: &str = "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1";
const UNKNOWN_ATTEMPTS: &str = "UPDATE otc_attempts SET state='unknown' WHERE trade_id=$1 AND kind=$2 AND state IN ('verified','review')";
const OVERDUE_PAYOUTS: &str = "SELECT id FROM otc_trades WHERE state NOT IN ('completed','refunded','rejected','expired') AND terms->'escrow' IS NULL AND (terms->>'payout_by')::timestamptz<now() AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=otc_trades.id AND e.kind IN ('payment','review_payment'))";
const UNKNOWN_OUTGOING: &str =
    "SELECT trade_id,id FROM otc_attempts WHERE kind IN ('payout','refund') AND state='unknown'";
const UNHEALTHY_OBSERVERS: &str = "SELECT chain FROM otc_observers WHERE paused OR failures>=3 OR last_scan IS NULL OR last_scan<now()-interval '60 seconds'";
const FUNDED_OPEN_TRADES: &str = "SELECT count(*) FROM otc_trades t WHERE state NOT IN ('completed','refunded','rejected','expired') AND (EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind IN ('payment','review_payment')) OR EXISTS(SELECT 1 FROM otc_escrow_evidence e WHERE e.trade_id=t.id AND e.kind='funding' AND valid))";
const UNDELIVERED_OUTBOX: &str =
    "SELECT activity_id FROM otc_outbox WHERE delivered_at IS NULL AND created_at<now()-interval '60 seconds'";
const STALE_BOOKINGS: &str = "SELECT id FROM otc_trades WHERE state='booking_pending' AND updated_at<now()-interval '60 seconds'";
const INVENTORY_SHORTFALL: &str = "SELECT r.trade_id FROM otc_reservations r JOIN otc_inventory i ON i.chain=r.chain WHERE r.status!='released' AND i.balance-i.gas_reserve<(SELECT COALESCE(sum(amount),0) FROM otc_reservations WHERE chain=i.chain AND status!='released')";
const CASE_FUNDED: &str = "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind IN ('payment','review_payment') AND valid UNION ALL SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='funding' AND valid LIMIT 1";
const STALE_UPDATES: &str = "SELECT trade_id,incident_key FROM otc_incidents WHERE closed_at IS NULL AND trade_id IS NOT NULL AND next_update_at<now()";
const FEE_MISMATCHES: &str = "SELECT t.id FROM otc_trades t LEFT JOIN otc_ledger l ON l.trade_id=t.id AND l.kind='fee_accrued' WHERE (t.state='completed' AND (l.id IS NULL OR l.amount!=(t.terms->'fee_policy'->>'fee_units')::numeric)) OR (t.state IN ('refunded','rejected','expired') AND l.id IS NOT NULL)";
const EXPIRE_QUOTES: &str =
    "UPDATE otc_trades SET state='expired',updated_at=now() WHERE state='quoted' AND (terms->>'quote_by')::timestamptz<now()";

pub(crate) async fn open_trades(client: &(impl GenericClient + Sync)) -> Result<i64> {
    Ok(client.query_one(OPEN_TRADES, &[]).await?.get(0))
}

pub(crate) async fn initialize_mode(
    client: &(impl GenericClient + Sync),
    demo: bool,
) -> Result<()> {
    client.execute(INITIALIZE_MODE, &[&demo]).await?;
    Ok(())
}

pub(crate) async fn mode(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client.query_one(MODE, &[]).await?.get(0))
}

pub(crate) async fn record_inventory(
    client: &(impl GenericClient + Sync),
    chain: &str,
    balance: &str,
    gas: &str,
    checkpoint: &Value,
) -> Result<()> {
    client
        .execute(RECORD_INVENTORY, &[&chain, &balance, &gas, checkpoint])
        .await?;
    Ok(())
}

pub(crate) async fn release_settled_debits(
    client: &(impl GenericClient + Sync),
    chain: &str,
    at: &DateTime<Utc>,
) -> Result<()> {
    client
        .execute(RELEASE_EVIDENCE_DEBITS, &[&chain, at])
        .await?;
    Ok(())
}

pub(crate) async fn release_escrow_debits(
    client: &(impl GenericClient + Sync),
    chain: &str,
    at: &DateTime<Utc>,
) -> Result<()> {
    client.execute(RELEASE_ESCROW_DEBITS, &[&chain, at]).await?;
    Ok(())
}

pub(crate) async fn record_scan(
    client: &(impl GenericClient + Sync),
    chain: &str,
    checkpoint: &Value,
    lag: i64,
) -> Result<()> {
    client
        .execute(RECORD_SCAN, &[&chain, checkpoint, &lag])
        .await?;
    Ok(())
}

pub(crate) async fn record_scan_failure(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<()> {
    client.execute(RECORD_SCAN_FAILURE, &[&chain]).await?;
    Ok(())
}

/// Returns `(trade_id, kind, evidence)` per valid payment that needs a re-check.
pub(crate) async fn revalidate(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, String, Value)>> {
    let rows = client.query(REVALIDATE, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

pub(crate) async fn mark_evidence_checked(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(CHECKED_EVIDENCE, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn invalidate_evidence(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(INVALIDATE_EVIDENCE, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn review_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(REVIEW_TRADE, &[id]).await?;
    Ok(())
}

pub(crate) async fn mark_attempts_unknown(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(UNKNOWN_ATTEMPTS, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn overdue_payouts(client: &(impl GenericClient + Sync)) -> Result<Vec<Uuid>> {
    let rows = client.query(OVERDUE_PAYOUTS, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

/// Returns `(trade_id, attempt_id)` per outgoing attempt of unknown state.
pub(crate) async fn unknown_outgoing(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, Uuid)>> {
    let rows = client.query(UNKNOWN_OUTGOING, &[]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub(crate) async fn unhealthy_observers(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<String>> {
    let rows = client.query(UNHEALTHY_OBSERVERS, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn funded_open_trades(client: &(impl GenericClient + Sync)) -> Result<i64> {
    Ok(client.query_one(FUNDED_OPEN_TRADES, &[]).await?.get(0))
}

pub(crate) async fn undelivered_outbox(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<String>> {
    let rows = client.query(UNDELIVERED_OUTBOX, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn stale_bookings(client: &(impl GenericClient + Sync)) -> Result<Vec<Uuid>> {
    let rows = client.query(STALE_BOOKINGS, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn inventory_shortfalls(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<Uuid>> {
    let rows = client.query(INVENTORY_SHORTFALL, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn case_funded(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<bool> {
    Ok(client.query_opt(CASE_FUNDED, &[id]).await?.is_some())
}

/// Returns `(trade_id, incident_key)` per incident whose customer update is overdue.
pub(crate) async fn stale_updates(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, String)>> {
    let rows = client.query(STALE_UPDATES, &[]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub(crate) async fn fee_mismatches(client: &(impl GenericClient + Sync)) -> Result<Vec<Uuid>> {
    let rows = client.query(FEE_MISMATCHES, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn expire_quotes(client: &(impl GenericClient + Sync)) -> Result<()> {
    client.execute(EXPIRE_QUOTES, &[]).await?;
    Ok(())
}

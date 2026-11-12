use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const OBSERVED_ESCROWS: &str =
    "SELECT trade_id FROM otc_escrows ORDER BY observed_at ASC NULLS FIRST LIMIT 100";
const ESCROW_OBSERVATION: &str = "SELECT deployment,policy,safe,vault,scan_from,scanned_through FROM otc_escrows WHERE trade_id=$1";
const SCANNED_THROUGH: &str = "SELECT scanned_through FROM otc_escrows WHERE trade_id=$1";
const RECORD_DEPLOYMENT: &str = "UPDATE otc_escrows SET safe=$2,vault=$3,deployed_at=$4,expires_at=$5,deployment_block=$6,state=CASE WHEN state='awaiting_consent' THEN 'created' ELSE state END WHERE trade_id=$1 AND (vault IS NULL OR vault=$3)";
const DEPLOYED_ADDRESSES: &str = "SELECT safe,vault,deployed_at FROM otc_escrows WHERE trade_id=$1";
const EXPIRE_UNSIGNED: &str =
    "UPDATE otc_escrows SET state='expired' WHERE trade_id=$1 AND safe IS NULL";
const EXPIRE_TRADE: &str =
    "UPDATE otc_trades SET state='expired',updated_at=now() WHERE id=$1 AND state='accepted'";
const RELEASE_RESERVATION: &str = "UPDATE otc_reservations SET status='released' WHERE trade_id=$1";
const OBSERVATION_PROGRESS: &str = "UPDATE otc_escrows SET observed_at=CASE WHEN $3 THEN now() ELSE NULL END,scanned_through=$2 WHERE trade_id=$1";
const MARK_FUNDED: &str = "UPDATE otc_escrows SET state=CASE WHEN state IN ('created','review') THEN 'funded' ELSE state END WHERE trade_id=$1";
const DEBIT_SELL_RESERVATION: &str = "UPDATE otc_reservations r SET status='debit' FROM otc_trades t WHERE r.trade_id=$1 AND t.id=r.trade_id AND t.terms->>'direction'='sell'";
const MARK_RELEASED: &str = "UPDATE otc_escrows SET state='released' WHERE trade_id=$1";
const MARK_REFUNDED: &str = "UPDATE otc_escrows SET state='refunded' WHERE trade_id=$1";
const MARK_EXPIRED: &str = "UPDATE otc_escrows SET state='expired' WHERE trade_id=$1";
const LOCK_TRADE: &str = "SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE";
const LOCK_TRADE_ROW: &str = "SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE";
const EVIDENCE_IDENTITY: &str =
    "SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind=$2";
const REFRESH_EVIDENCE: &str = "UPDATE otc_escrow_evidence SET valid=TRUE,evidence=$3,last_checked_at=now() WHERE trade_id=$1 AND kind=$2";
const INSERT_EVIDENCE: &str = "INSERT INTO otc_escrow_evidence(trade_id,kind,identity,evidence,included_at) VALUES($1,$2,$3,$4,$5)";
const VALID_EVIDENCE: &str =
    "SELECT kind,evidence FROM otc_escrow_evidence WHERE trade_id=$1 AND valid";
const INVALIDATE_EVIDENCE: &str =
    "UPDATE otc_escrow_evidence SET valid=FALSE WHERE trade_id=$1 AND kind=$2";
const REVIEW_ESCROW: &str =
    "UPDATE otc_escrows SET state='review',scanned_through=NULL WHERE trade_id=$1";
const REVIEW_TRADE: &str = "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1";
const OPEN_CALLS: &str = "SELECT id,instructions,reference,scan_cursor FROM otc_escrow_calls WHERE trade_id=$1 AND state IN ('unknown','submitted')";
const ADVANCE_CALL_CURSOR: &str = "UPDATE otc_escrow_calls SET scan_cursor=$2 WHERE id=$1";
const RESOLVE_CALL: &str =
    "UPDATE otc_escrow_calls SET reference=$2,state=$3,evidence=$4 WHERE id=$1";
const LOCKED_ESCROW_STATE: &str =
    "SELECT state,expires_at FROM otc_escrows WHERE trade_id=$1 FOR UPDATE";
const VALID_EVIDENCE_AT: &str =
    "SELECT included_at FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid";
const FUNDING_AT: &str =
    "SELECT included_at FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='funding' AND valid";
const RELEASE_EVIDENCE: &str =
    "SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='release' AND valid";
const REFUND_EVIDENCE: &str =
    "SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='refund' AND valid";
const UNRESOLVED_ATTEMPT: &str = "SELECT id FROM otc_attempts WHERE trade_id=$1 AND state IN ('prepared','unknown','submitted','confirming','review')";
const INVALIDATED_EVIDENCE: &str =
    "SELECT kind FROM otc_escrow_evidence WHERE trade_id=$1 AND NOT valid LIMIT 1";
const INSERT_FEE: &str = "INSERT INTO otc_ledger(id,trade_id,kind,currency,amount,policy) VALUES($1,$2,'fee_accrued','USDT',$3::text::numeric,$4) ON CONFLICT DO NOTHING";
const SET_TRADE_STATE: &str = "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1";
const RELEASE_ACTIVE_RESERVATION: &str =
    "UPDATE otc_reservations SET status='released' WHERE trade_id=$1 AND status='active'";

pub(crate) async fn observed_escrows(client: &(impl GenericClient + Sync)) -> Result<Vec<Uuid>> {
    Ok(client
        .query(OBSERVED_ESCROWS, &[])
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

/// Returns `(deployment, policy, safe, vault, scan_from, scanned_through)`.
pub(crate) async fn observation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(
    Value,
    Value,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
)> {
    let row = client.query_one(ESCROW_OBSERVATION, &[trade_id]).await?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
    ))
}

pub(crate) async fn cursor(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Option<String>> {
    Ok(client.query_one(SCANNED_THROUGH, &[trade_id]).await?.get(0))
}

pub(crate) async fn record_deployment(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    safe: &str,
    vault: &str,
    at: DateTime<Utc>,
    expires: DateTime<Utc>,
    block_hash: &str,
) -> Result<()> {
    client
        .execute(
            RECORD_DEPLOYMENT,
            &[trade_id, &safe, &vault, &at, &expires, &block_hash],
        )
        .await?;
    Ok(())
}

pub(crate) async fn deployed_addresses(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(Option<String>, Option<String>)> {
    let row = client.query_one(DEPLOYED_ADDRESSES, &[trade_id]).await?;
    Ok((row.get(0), row.get(1)))
}

pub(crate) async fn expire_unsigned(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(EXPIRE_UNSIGNED, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn expire_trade(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(EXPIRE_TRADE, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn release_reservation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(RELEASE_RESERVATION, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn record_progress(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    scanned_through: &str,
    caught_up: bool,
) -> Result<()> {
    client
        .execute(
            OBSERVATION_PROGRESS,
            &[trade_id, &scanned_through, &caught_up],
        )
        .await?;
    Ok(())
}

pub(crate) async fn mark_funded(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(MARK_FUNDED, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn debit_sell_reservation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(DEBIT_SELL_RESERVATION, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn mark_released(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(MARK_RELEASED, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn mark_refunded(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(MARK_REFUNDED, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn mark_expired(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(MARK_EXPIRED, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn lock_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.query_one(LOCK_TRADE, &[id]).await?;
    Ok(())
}

pub(crate) async fn lock_escrow_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> std::result::Result<tokio_postgres::Row, tokio_postgres::Error> {
    client.query_one(LOCK_TRADE_ROW, &[id]).await
}

pub(crate) async fn evidence_identity(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(EVIDENCE_IDENTITY, &[trade_id, &kind])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn refresh_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(REFRESH_EVIDENCE, &[trade_id, &kind, evidence])
        .await?;
    Ok(())
}

pub(crate) async fn insert_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
    identity: &str,
    evidence: &Value,
    at: DateTime<Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_EVIDENCE,
            &[trade_id, &kind, &identity, evidence, &at],
        )
        .await?;
    Ok(())
}

pub(crate) async fn valid_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<(String, Value)>> {
    Ok(client
        .query(VALID_EVIDENCE, &[trade_id])
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect())
}

pub(crate) async fn invalidate_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<()> {
    client
        .execute(INVALIDATE_EVIDENCE, &[trade_id, &kind])
        .await?;
    Ok(())
}

pub(crate) async fn review_escrow(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(REVIEW_ESCROW, &[trade_id]).await?;
    Ok(())
}

pub(crate) async fn review_trade(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(REVIEW_TRADE, &[trade_id]).await?;
    Ok(())
}

/// Returns `(id, instructions, reference, scan_cursor)` per open handoff.
pub(crate) async fn open_calls(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<(Uuid, Value, Option<String>, Option<String>)>> {
    Ok(client
        .query(OPEN_CALLS, &[trade_id])
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect())
}

pub(crate) async fn advance_call_cursor(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    cursor: &str,
) -> Result<()> {
    client.execute(ADVANCE_CALL_CURSOR, &[id, &cursor]).await?;
    Ok(())
}

pub(crate) async fn resolve_call(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    reference: &str,
    state: &str,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(RESOLVE_CALL, &[id, &reference, &state, evidence])
        .await?;
    Ok(())
}

pub(crate) async fn lock_escrow_state(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(String, Option<DateTime<Utc>>)> {
    let row = client.query_one(LOCKED_ESCROW_STATE, &[trade_id]).await?;
    Ok((row.get(0), row.get(1)))
}

pub(crate) async fn valid_evidence_at(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<Option<DateTime<Utc>>> {
    Ok(client
        .query_opt(VALID_EVIDENCE_AT, &[trade_id, &kind])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn funding_at(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Option<DateTime<Utc>>> {
    Ok(client
        .query_opt(FUNDING_AT, &[trade_id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn released(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(RELEASE_EVIDENCE, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn refunded(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(REFUND_EVIDENCE, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn has_unresolved_attempt(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_ATTEMPT, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn has_invalidated_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(INVALIDATED_EVIDENCE, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn insert_fee(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    fee_units: &str,
    policy: &Value,
) -> Result<()> {
    client
        .execute(INSERT_FEE, &[id, trade_id, &fee_units, policy])
        .await?;
    Ok(())
}

pub(crate) async fn set_trade_state(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    state: &str,
) -> Result<()> {
    client.execute(SET_TRADE_STATE, &[id, &state]).await?;
    Ok(())
}

pub(crate) async fn release_active_reservation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client
        .execute(RELEASE_ACTIVE_RESERVATION, &[trade_id])
        .await?;
    Ok(())
}

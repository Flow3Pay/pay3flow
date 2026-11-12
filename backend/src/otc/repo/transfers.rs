use crate::otc::error::{Error, Result};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use tokio_postgres::Row;
use uuid::Uuid;

// Queries shared with `repo::booking` and `repo::escrow_observer`.
const LOCK_TRADE: &str = "SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE";
const LOCK_TRADE_ID: &str = "SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE";
const SET_TRADE_STATE: &str = "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1";

const ATTEMPT_BY_KEY: &str = "SELECT id,instructions,state FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3";
const SIGNING_PAUSED: &str = "SELECT signing_paused FROM otc_controls WHERE singleton";
const LIVE_CHECKPOINT: &str = "SELECT checkpoint FROM otc_observers WHERE chain=$1 AND NOT paused AND last_scan>now()-interval '60 seconds'";
const FAILED_ATTEMPT: &str = "SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND state='failed' AND NOT retry_authorized";
const UNRESOLVED_OUTGOING: &str = "SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind IN ('payout','refund') AND state!='failed'";
const VALID_RECEIPT: &str = "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND valid AND (kind='payment' OR ($2::text='refund' AND kind='review_payment'))";
const OPEN_ATTEMPT_FOR_WALLET: &str = "SELECT id FROM otc_attempts WHERE state IN ('prepared','unknown','submitted','confirming') AND instructions->'leg'->>'chain'=$1 AND instructions->'leg'->>'sender'=$2";
const UNRESOLVED_ESCROW_CALL: &str =
    "SELECT id FROM otc_escrow_calls WHERE sender=$1 AND state IN ('unknown','submitted') LIMIT 1";
const LOCK_INVENTORY: &str = "SELECT balance::text,gas_reserve::text FROM otc_inventory WHERE chain=$1 AND observed_at>now()-interval '60 seconds' FOR UPDATE";
const RESERVED_ELSEWHERE: &str = "SELECT COALESCE(sum(amount),0)::text FROM otc_reservations WHERE chain=$1 AND status!='released' AND trade_id!=$2";
const REASSIGN_RESERVATION: &str = "UPDATE otc_reservations SET chain=$2,amount=$3::text::numeric,status='active' WHERE trade_id=$1";
const INSERT_ATTEMPT: &str = "INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,$4,'unknown',$5,$6)";
const SUBMIT_CONTEXT: &str = "SELECT a.trade_id,a.kind,a.reference,a.chain,t.customer_actor,t.desk_actor FROM otc_attempts a JOIN otc_trades t ON t.id=a.trade_id WHERE a.id=$1 FOR UPDATE OF a";
const SET_ATTEMPT_REFERENCE: &str = "UPDATE otc_attempts SET reference=$2,state=CASE WHEN state IN ('unknown','prepared') THEN 'submitted' ELSE state END,updated_at=now() WHERE id=$1";
const TRADE_TERMS: &str = "SELECT terms FROM otc_trades WHERE id=$1";
const MARK_CONFIRMING: &str = "UPDATE otc_attempts SET state='confirming',updated_at=now() WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted')";
const EVIDENCE_BY_IDENTITY: &str =
    "SELECT trade_id,kind,valid,evidence FROM otc_evidence WHERE chain=$1 AND identity=$2";
const EVIDENCE_IDENTITY: &str = "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2";
const REFRESH_EVIDENCE: &str = "UPDATE otc_evidence SET valid=TRUE,evidence=$3,included_at=$4,last_checked_at=now() WHERE trade_id=$1 AND kind=$2";
const INSERT_EVIDENCE: &str = "INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES($1,$2,$3,$4,$5,$6)";
const VERIFY_ATTEMPT: &str = "UPDATE otc_attempts SET state='verified',updated_at=now() WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted','confirming')";
const PAYMENT_INCLUDED_AT: &str =
    "SELECT included_at FROM otc_evidence WHERE trade_id=$1 AND kind='payment' AND valid";
const REVIEW_PAYMENT_RECEIPT: &str =
    "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='review_payment' AND valid";
const PAYOUT_IDENTITY: &str =
    "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='payout' AND valid";
const REFUND_IDENTITY: &str =
    "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='refund' AND valid";
const INSERT_FEE_ACCRUED: &str = "INSERT INTO otc_ledger(id,trade_id,kind,currency,amount,policy) VALUES($1,$2,'fee_accrued','USDT',$3::text::numeric,$4) ON CONFLICT DO NOTHING";
const DEBIT_RESERVATION: &str = "UPDATE otc_reservations SET status='debit' WHERE trade_id=$1";
const OPEN_ATTEMPTS: &str = "SELECT id,trade_id,kind,chain,instructions,reference FROM otc_attempts WHERE state IN ('unknown','submitted','confirming') ORDER BY created_at LIMIT 100";
const LOCATE_REFERENCE: &str =
    "UPDATE otc_attempts SET reference=$2,state='submitted' WHERE id=$1 AND reference IS NULL";
const VALID_EVIDENCE_JSON: &str =
    "SELECT evidence FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid";
const INVALIDATE_EVIDENCE: &str =
    "UPDATE otc_evidence SET valid=FALSE WHERE trade_id=$1 AND kind=$2";
const RESET_ATTEMPTS: &str = "UPDATE otc_attempts SET state='unknown' WHERE trade_id=$1 AND kind=$2 AND state IN ('verified','review')";
const MAP_TRADE_TO_REVIEW: &str =
    "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1";
const GUARDED_MAP_TRADE_TO_REVIEW: &str = "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1 AND state NOT IN ('completed','refunded')";
const FAIL_ATTEMPT: &str = "UPDATE otc_attempts SET state='failed',failure_evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('unknown','submitted','confirming')";
const RETRY_CONTEXT: &str = "SELECT a.trade_id,a.kind,a.state,a.failure_evidence,t.customer_actor,t.desk_actor FROM otc_attempts a JOIN otc_trades t ON t.id=a.trade_id WHERE a.id=$1 FOR UPDATE OF a,t";
const AUTHORIZE_RETRY: &str = "UPDATE otc_attempts SET retry_authorized=TRUE WHERE id=$1";
const REVIEWED_TRADE: &str =
    "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1 AND state='review'";

pub(crate) async fn lock_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<Row> {
    Ok(client.query_one(LOCK_TRADE, &[id]).await?)
}

/// Row-locks the trade without reading its columns.
pub(crate) async fn lock_trade_id(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.query_one(LOCK_TRADE_ID, &[id]).await?;
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

/// Returns `(attempt, instructions, state)` for an idempotent handoff replay.
pub(crate) async fn attempt_by_key(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
    key: &str,
) -> Result<Option<(Uuid, Value, String)>> {
    let row = client.query_opt(ATTEMPT_BY_KEY, &[id, &kind, &key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn signing_paused(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client
        .query_one(SIGNING_PAUSED, &[])
        .await?
        .get::<_, bool>(0))
}

pub(crate) async fn live_checkpoint(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<Value> {
    let row = client.query_one(LIVE_CHECKPOINT, &[&chain]).await?;
    Ok(row.get::<_, Value>(0))
}

pub(crate) async fn failed_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(FAILED_ATTEMPT, &[id, &kind])
        .await?
        .is_some())
}

pub(crate) async fn unresolved_outgoing(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_OUTGOING, &[id])
        .await?
        .is_some())
}

pub(crate) async fn has_valid_receipt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(VALID_RECEIPT, &[id, &kind])
        .await?
        .is_some())
}

pub(crate) async fn open_attempt_for_wallet(
    client: &(impl GenericClient + Sync),
    chain: &str,
    sender: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(OPEN_ATTEMPT_FOR_WALLET, &[&chain, &sender])
        .await?
        .is_some())
}

pub(crate) async fn unresolved_escrow_call(
    client: &(impl GenericClient + Sync),
    sender: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_ESCROW_CALL, &[&sender])
        .await?
        .is_some())
}

/// Returns `(balance, gas_reserve)` as decimal text.
pub(crate) async fn lock_inventory(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<(String, String)> {
    let row = client
        .query_opt(LOCK_INVENTORY, &[&chain])
        .await?
        .ok_or(Error::Disabled)?;
    Ok((row.get(0), row.get(1)))
}

pub(crate) async fn reserved_elsewhere(
    client: &(impl GenericClient + Sync),
    chain: &str,
    id: &Uuid,
) -> Result<String> {
    let row = client.query_one(RESERVED_ELSEWHERE, &[&chain, id]).await?;
    Ok(row.get(0))
}

pub(crate) async fn reassign_reservation(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    chain: &str,
    amount: &str,
) -> Result<()> {
    client
        .execute(REASSIGN_RESERVATION, &[id, &chain, &amount])
        .await?;
    Ok(())
}

pub(crate) async fn insert_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    id: &Uuid,
    kind: &str,
    chain: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_ATTEMPT,
            &[attempt, id, &kind, &chain, instructions, &key],
        )
        .await?;
    Ok(())
}

/// Returns `(trade_id, kind, reference, chain, customer_actor, desk_actor)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn submit_context(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<(Uuid, String, Option<String>, String, String, String)> {
    let row = client
        .query_opt(SUBMIT_CONTEXT, &[attempt])
        .await?
        .ok_or(Error::NotFound)?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
    ))
}

pub(crate) async fn set_attempt_reference(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    reference: &str,
) -> Result<()> {
    client
        .execute(SET_ATTEMPT_REFERENCE, &[attempt, &reference])
        .await?;
    Ok(())
}

pub(crate) async fn trade_terms(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<Value> {
    let row = client.query_one(TRADE_TERMS, &[id]).await?;
    Ok(row.get(0))
}

pub(crate) async fn mark_confirming(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(MARK_CONFIRMING, &[id, &kind]).await?;
    Ok(())
}

/// Returns `(trade_id, kind, valid, evidence)` for an already credited identity.
pub(crate) async fn evidence_by_identity(
    client: &(impl GenericClient + Sync),
    chain: &str,
    identity: &str,
) -> Result<Option<(Uuid, String, bool, Value)>> {
    let row = client
        .query_opt(EVIDENCE_BY_IDENTITY, &[&chain, &identity])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2), row.get(3))))
}

pub(crate) async fn evidence_identity(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(EVIDENCE_IDENTITY, &[id, &kind])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn refresh_evidence(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
    evidence: &Value,
    included_at: &chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    client
        .execute(REFRESH_EVIDENCE, &[id, &kind, evidence, included_at])
        .await?;
    Ok(())
}

pub(crate) async fn insert_evidence(
    client: &(impl GenericClient + Sync),
    chain: &str,
    identity: &str,
    id: &Uuid,
    kind: &str,
    evidence: &Value,
    included_at: &chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_EVIDENCE,
            &[&chain, &identity, id, &kind, evidence, included_at],
        )
        .await?;
    Ok(())
}

pub(crate) async fn verify_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(VERIFY_ATTEMPT, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn payment_included_at(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    Ok(client
        .query_opt(PAYMENT_INCLUDED_AT, &[id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn review_payment_receipt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(REVIEW_PAYMENT_RECEIPT, &[id])
        .await?
        .is_some())
}

pub(crate) async fn payout_receipt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client.query_opt(PAYOUT_IDENTITY, &[id]).await?.is_some())
}

pub(crate) async fn refund_receipt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client.query_opt(REFUND_IDENTITY, &[id]).await?.is_some())
}

pub(crate) async fn insert_fee_accrued(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    fee_units: &str,
    policy: &Value,
) -> Result<()> {
    client
        .execute(
            INSERT_FEE_ACCRUED,
            &[&Uuid::new_v4(), id, &fee_units, policy],
        )
        .await?;
    Ok(())
}

pub(crate) async fn debit_reservation(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<()> {
    client.execute(DEBIT_RESERVATION, &[id]).await?;
    Ok(())
}

/// Returns `(attempt, trade_id, kind, chain, instructions, reference)` per open attempt.
#[allow(clippy::type_complexity)]
pub(crate) async fn open_attempts(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, Uuid, String, String, Value, Option<String>)>> {
    let rows = client.query(OPEN_ATTEMPTS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
            )
        })
        .collect())
}

pub(crate) async fn locate_reference(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    reference: &str,
) -> Result<()> {
    client
        .execute(LOCATE_REFERENCE, &[attempt, &reference])
        .await?;
    Ok(())
}

pub(crate) async fn valid_evidence(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<Option<Value>> {
    Ok(client
        .query_opt(VALID_EVIDENCE_JSON, &[id, &kind])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn invalidate_evidence(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(INVALIDATE_EVIDENCE, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn reset_attempts(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(RESET_ATTEMPTS, &[id, &kind]).await?;
    Ok(())
}

pub(crate) async fn map_trade_to_review(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<()> {
    client.execute(MAP_TRADE_TO_REVIEW, &[id]).await?;
    Ok(())
}

pub(crate) async fn guarded_map_trade_to_review(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<()> {
    client.execute(GUARDED_MAP_TRADE_TO_REVIEW, &[id]).await?;
    Ok(())
}

/// Returns the number of failed attempts changed.
pub(crate) async fn fail_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    proof: &Value,
) -> Result<u64> {
    Ok(client.execute(FAIL_ATTEMPT, &[attempt, proof]).await?)
}

/// Returns `(trade_id, kind, state, failure_evidence, customer_actor, desk_actor)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn retry_context(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<(Uuid, String, String, Option<Value>, String, String)> {
    let row = client
        .query_opt(RETRY_CONTEXT, &[attempt])
        .await?
        .ok_or(Error::NotFound)?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
    ))
}

pub(crate) async fn authorize_retry(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<()> {
    client.execute(AUTHORIZE_RETRY, &[attempt]).await?;
    Ok(())
}

pub(crate) async fn reviewed_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    state: &str,
) -> Result<()> {
    client.execute(REVIEWED_TRADE, &[id, &state]).await?;
    Ok(())
}

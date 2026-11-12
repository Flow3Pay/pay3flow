use crate::otc::error::{Error, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const PENDING_CALL: &str =
    "SELECT id,terms_hash,created_at FROM otc_ever_calls WHERE address=$1 AND state='unknown'";
const LOCK_WALLET: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1,0))";
const SIGNING_PAUSED: &str = "SELECT signing_paused FROM otc_controls WHERE singleton=TRUE";
const CALL_BY_KEY: &str = "SELECT id,instructions,state,reference FROM otc_ever_calls WHERE actor=$1 AND idempotency_key=$2";
const UNRESOLVED_CALL: &str = "SELECT id FROM otc_ever_calls WHERE address=$1 AND state='unknown'";
const BIND_AGREEMENT: &str = "INSERT INTO otc_ever_agreements(address,terms_hash,manifest) VALUES($1,$2,$3) ON CONFLICT(address) DO NOTHING";
const AGREEMENT_HASH: &str = "SELECT terms_hash FROM otc_ever_agreements WHERE address=$1";
const INSERT_CALL: &str = "INSERT INTO otc_ever_calls(id,address,actor,public_key,terms_hash,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7)";
const CALL_VIEW: &str =
    "SELECT state,reference,evidence,address,terms_hash FROM otc_ever_calls WHERE id=$1";
const CALL_RECEIPT: &str =
    "SELECT address,instructions,created_at,state,reference FROM otc_ever_calls WHERE id=$1";
const LOCK_CALL: &str = "SELECT state,reference FROM otc_ever_calls WHERE id=$1 FOR UPDATE";
const CALL_BY_REFERENCE: &str = "SELECT id FROM otc_ever_calls WHERE reference=$1 AND id<>$2";
const RECONCILE_CALL: &str =
    "UPDATE otc_ever_calls SET state=$2,reference=$3,evidence=$4 WHERE id=$1 AND state='unknown'";

/// Advisory lock keyed on `ever-multisig:<address>`; the caller formats the key.
pub(crate) async fn lock_wallet(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(LOCK_WALLET, &[&key]).await?;
    Ok(())
}

/// Returns `(id, terms_hash, created_at)` of the unresolved handoff for a wallet.
pub(crate) async fn pending_call(
    client: &(impl GenericClient + Sync),
    address: &str,
) -> Result<Option<(Uuid, String, DateTime<Utc>)>> {
    let row = client.query_opt(PENDING_CALL, &[&address]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn signing_paused(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client.query_one(SIGNING_PAUSED, &[]).await?.get(0))
}

/// Returns `(id, instructions, state, reference)` for an idempotent replay.
#[allow(clippy::type_complexity)]
pub(crate) async fn call_by_key(
    client: &(impl GenericClient + Sync),
    actor: &str,
    key: &str,
) -> Result<Option<(Uuid, Value, String, Option<String>)>> {
    let row = client.query_opt(CALL_BY_KEY, &[&actor, &key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2), row.get(3))))
}

pub(crate) async fn unresolved_call(
    client: &(impl GenericClient + Sync),
    address: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_CALL, &[&address])
        .await?
        .is_some())
}

pub(crate) async fn bind_agreement(
    client: &(impl GenericClient + Sync),
    address: &str,
    hash: &str,
    manifest: &Value,
) -> Result<()> {
    client
        .execute(BIND_AGREEMENT, &[&address, &hash, &manifest])
        .await?;
    Ok(())
}

pub(crate) async fn agreement_hash(
    client: &(impl GenericClient + Sync),
    address: &str,
) -> Result<String> {
    let row = client.query_one(AGREEMENT_HASH, &[&address]).await?;
    Ok(row.get(0))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_call(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    address: &str,
    actor: &str,
    public_key: &str,
    hash: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_CALL,
            &[
                id,
                &address,
                &actor,
                &public_key,
                &hash,
                &instructions,
                &key,
            ],
        )
        .await?;
    Ok(())
}

/// Returns `(state, reference, evidence, address, terms_hash)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn call_view(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<(String, Option<String>, Option<Value>, String, String)> {
    let row = client
        .query_opt(CALL_VIEW, &[id])
        .await?
        .ok_or(Error::NotFound)?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
}

/// Returns `(address, instructions, created_at, state, reference)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn call_receipt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<(String, Value, DateTime<Utc>, String, Option<String>)> {
    let row = client
        .query_opt(CALL_RECEIPT, &[id])
        .await?
        .ok_or(Error::NotFound)?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
}

/// Returns `(state, reference)` of the locked handoff.
pub(crate) async fn lock_call(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<(String, Option<String>)> {
    let row = client.query_one(LOCK_CALL, &[id]).await?;
    Ok((row.get(0), row.get(1)))
}

pub(crate) async fn call_by_reference(
    client: &(impl GenericClient + Sync),
    reference: &str,
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(CALL_BY_REFERENCE, &[&reference, id])
        .await?
        .is_some())
}

pub(crate) async fn reconcile_call(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    state: &str,
    reference: &str,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(RECONCILE_CALL, &[id, &state, &reference, evidence])
        .await?;
    Ok(())
}

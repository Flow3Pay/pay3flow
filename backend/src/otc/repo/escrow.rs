use crate::otc::error::{Error, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const ESCROW_TRADE: &str = "SELECT * FROM otc_trades t WHERE id=$1 AND (customer_actor=$2 OR desk_actor=$2 OR (state NOT IN ('quoted','booking_pending','rejected') AND EXISTS(SELECT 1 FROM otc_login_wallets w WHERE w.actor=$2 AND w.chain='ethereum' AND w.address=t.terms->'escrow'->>'arbitrator')))";
const ESCROW_EXISTS: &str = "SELECT trade_id FROM otc_escrows WHERE trade_id=$1";
const OBSERVER_CHECKPOINT: &str = "SELECT checkpoint FROM otc_observers WHERE chain='ethereum'";
const INSERT_ESCROW: &str = "INSERT INTO otc_escrows(trade_id,deployment,policy,scan_from) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING";
const ESCROW_DETAILS: &str = "SELECT deployment,safe,vault,deployed_at,expires_at,state,observed_at FROM otc_escrows WHERE trade_id=$1";
const CONSENT_OWNERS: &str =
    "SELECT owner FROM otc_escrow_consents WHERE trade_id=$1 ORDER BY owner";
const PROPOSALS: &str = "SELECT id,action,hash,typed_data,dispute,reason,evidence FROM otc_escrow_proposals WHERE trade_id=$1 ORDER BY created_at";
const APPROVAL_OWNERS: &str =
    "SELECT owner FROM otc_escrow_approvals WHERE proposal_id=$1 ORDER BY owner";
const CALLS: &str = "SELECT id,action,state,instructions,reference,actor,proposal_id FROM otc_escrow_calls WHERE trade_id=$1 ORDER BY created_at";
const EVIDENCE: &str =
    "SELECT kind,evidence,valid FROM otc_escrow_evidence WHERE trade_id=$1 ORDER BY included_at";
const LOCK_TRADE: &str = "SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE";
const LOCK_TRADE_ROW: &str = "SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE";
const CONSENT_SIGNATURE: &str =
    "SELECT signature FROM otc_escrow_consents WHERE trade_id=$1 AND owner=$2";
const INSERT_CONSENT: &str =
    "INSERT INTO otc_escrow_consents(trade_id,owner,signature,idempotency_key) VALUES($1,$2,$3,$4)";
const SIGNING_PAUSED: &str = "SELECT signing_paused FROM otc_controls WHERE singleton";
const PROPOSAL_BY_KEY: &str = "SELECT action,dispute,reason,evidence FROM otc_escrow_proposals WHERE trade_id=$1 AND author=$2 AND idempotency_key=$3";
const LOCK_ESCROW: &str =
    "SELECT safe,vault,expires_at,state FROM otc_escrows WHERE trade_id=$1 FOR UPDATE";
const VALID_EVIDENCE: &str =
    "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid";
const UNRESOLVED_ATTEMPT: &str = "SELECT id FROM otc_attempts WHERE trade_id=$1 AND state IN ('unknown','submitted','confirming','review')";
const PROPOSAL_EXISTS: &str =
    "SELECT id FROM otc_escrow_proposals WHERE trade_id=$1 AND nonce=$2 AND action=$3";
const INSERT_PROPOSAL: &str = "INSERT INTO otc_escrow_proposals(id,trade_id,action,nonce,hash,typed_data,dispute,reason,evidence,author,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)";
const APPROVAL_CONTEXT: &str = "SELECT p.hash,p.nonce,e.safe,e.expires_at,e.state FROM otc_escrow_proposals p JOIN otc_escrows e ON e.trade_id=p.trade_id WHERE p.id=$1 AND p.trade_id=$2";
const PROPOSAL_BY_ID: &str =
    "SELECT action,nonce,typed_data FROM otc_escrow_proposals WHERE id=$1 AND trade_id=$2";
const APPROVAL_SIGNATURES: &str =
    "SELECT signature FROM otc_escrow_approvals WHERE proposal_id=$1 ORDER BY owner";
const APPROVAL_SIGNATURE: &str =
    "SELECT signature FROM otc_escrow_approvals WHERE proposal_id=$1 AND owner=$2";
const INSERT_APPROVAL: &str =
    "INSERT INTO otc_escrow_approvals(proposal_id,owner,signature) VALUES($1,$2,$3)";
const CALL_BY_KEY: &str = "SELECT id,action,proposal_id,instructions,state,reference FROM otc_escrow_calls WHERE trade_id=$1 AND actor=$2 AND idempotency_key=$3";
const CONSENT_BY_OWNER: &str =
    "SELECT signature FROM otc_escrow_consents WHERE trade_id=$1 AND owner=$2";
const ADVISORY_LOCK: &str = "SELECT pg_advisory_xact_lock(hashtext($1))";
const UNRESOLVED_HANDOFF: &str = "SELECT id FROM otc_escrow_calls WHERE sender=$1 AND state IN ('unknown','submitted') UNION ALL SELECT id FROM otc_attempts WHERE instructions->'leg'->>'chain'='ethereum' AND instructions->'leg'->>'sender'=$1 AND state IN ('prepared','unknown','submitted','confirming') LIMIT 1";
const INSERT_CALL: &str = "INSERT INTO otc_escrow_calls(id,trade_id,actor,action,proposal_id,sender,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8)";
const SET_CALL_REFERENCE: &str = "UPDATE otc_escrow_calls SET reference=$4,state=CASE WHEN state='unknown' THEN 'submitted' ELSE state END WHERE id=$1 AND trade_id=$2 AND actor=$3 AND (reference IS NULL OR reference=$4)";
const ATTEMPT_BY_KEY: &str = "SELECT id,instructions,state FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3";
const ESCROW_EVER_STATE: &str =
    "SELECT expires_at,state,observed_at FROM otc_escrows WHERE trade_id=$1 FOR UPDATE";
const FUNDING_EVIDENCE: &str =
    "SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='funding' AND valid";
const PRIOR_ATTEMPT: &str = "SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND (state!='failed' OR NOT retry_authorized)";
const ESCROW_VAULT: &str = "SELECT vault FROM otc_escrows WHERE trade_id=$1";
const EVERSALE_HANDOFF: &str = "SELECT id FROM otc_attempts WHERE instructions->'leg'->>'chain'='everscale' AND instructions->'leg'->>'sender'=$1 AND state IN ('unknown','submitted','confirming')";
const EVERSALE_CHECKPOINT: &str = "SELECT checkpoint FROM otc_observers WHERE chain='everscale' AND NOT paused AND last_scan>now()-interval '60 seconds'";
const INSERT_EVER_ATTEMPT: &str = "INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,'everscale','unknown',$4,$5)";
const SET_TRADE_STATE: &str = "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1";
const CONFIRM_ATTEMPT: &str = "UPDATE otc_attempts SET state='confirming' WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted')";
const EVIDENCE_IDENTITY: &str = "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2";
const INSERT_EVIDENCE: &str = "INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES('everscale',$1,$2,$3,$4,$5) ON CONFLICT(trade_id,kind) DO UPDATE SET valid=TRUE,evidence=$4,last_checked_at=now()";
const VERIFY_ATTEMPT: &str =
    "UPDATE otc_attempts SET state='verified' WHERE trade_id=$1 AND kind=$2";
const DEBIT_RESERVATION: &str = "UPDATE otc_reservations SET status='debit' WHERE trade_id=$1";

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

pub(crate) async fn trade_for_participant(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<tokio_postgres::Row> {
    client
        .query_opt(ESCROW_TRADE, &[id, &actor])
        .await?
        .ok_or(Error::NotFound)
}

pub(crate) async fn exists(client: &(impl GenericClient + Sync), trade_id: &Uuid) -> Result<bool> {
    Ok(client
        .query_opt(ESCROW_EXISTS, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn observer_checkpoint(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<Value> {
    Ok(client
        .query_one(OBSERVER_CHECKPOINT, &[&chain])
        .await?
        .get(0))
}

pub(crate) async fn insert(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    deployment: &Value,
    policy: &Value,
    scan_from: &str,
) -> Result<()> {
    client
        .execute(INSERT_ESCROW, &[trade_id, deployment, policy, &scan_from])
        .await?;
    Ok(())
}

/// Returns `(deployment, safe, vault, deployed_at, expires_at, state, observed_at)`.
pub(crate) async fn details(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(
    Value,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
    String,
    Option<DateTime<Utc>>,
)> {
    let row = client.query_one(ESCROW_DETAILS, &[trade_id]).await?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
        row.get(6),
    ))
}

pub(crate) async fn consent_owners(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<String>> {
    Ok(client
        .query(CONSENT_OWNERS, &[trade_id])
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

/// Returns `(id, action, hash, typed_data, dispute, reason, evidence)` per proposal.
pub(crate) async fn proposals(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<(Uuid, String, String, Value, bool, String, Value)>> {
    Ok(client
        .query(PROPOSALS, &[trade_id])
        .await?
        .iter()
        .map(|row| {
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
            )
        })
        .collect())
}

pub(crate) async fn approval_owners(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
) -> Result<Vec<String>> {
    Ok(client
        .query(APPROVAL_OWNERS, &[proposal_id])
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

/// Returns `(id, action, state, instructions, reference, actor, proposal_id)` per call.
pub(crate) async fn calls(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<
    Vec<(
        Uuid,
        String,
        String,
        Value,
        Option<String>,
        String,
        Option<Uuid>,
    )>,
> {
    Ok(client
        .query(CALLS, &[trade_id])
        .await?
        .iter()
        .map(|row| {
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
            )
        })
        .collect())
}

/// Returns `(kind, evidence, valid)` per recorded escrow event.
pub(crate) async fn evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<(String, Value, bool)>> {
    Ok(client
        .query(EVIDENCE, &[trade_id])
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

pub(crate) async fn consent_signature(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    owner: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(CONSENT_SIGNATURE, &[trade_id, &owner])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn insert_consent(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    owner: &str,
    signature: &str,
    key: &str,
) -> Result<()> {
    client
        .execute(INSERT_CONSENT, &[trade_id, &owner, &signature, &key])
        .await?;
    Ok(())
}

pub(crate) async fn signing_paused(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client.query_one(SIGNING_PAUSED, &[]).await?.get(0))
}

/// Returns `(action, dispute, reason, evidence)` of an existing proposal key.
pub(crate) async fn proposal_by_key(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    actor: &str,
    key: &str,
) -> Result<Option<(String, bool, String, Value)>> {
    Ok(client
        .query_opt(PROPOSAL_BY_KEY, &[trade_id, &actor, &key])
        .await?
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3))))
}

/// Returns `(safe, vault, expires_at, state)` of a locked escrow.
pub(crate) async fn lock_escrow(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(Option<String>, Option<String>, DateTime<Utc>, String)> {
    let row = client.query_one(LOCK_ESCROW, &[trade_id]).await?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub(crate) async fn has_valid_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(VALID_EVIDENCE, &[trade_id, &kind])
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

pub(crate) async fn proposal_exists(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    nonce: &str,
    action: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(PROPOSAL_EXISTS, &[trade_id, &nonce, &action])
        .await?
        .is_some())
}

pub(crate) async fn insert_proposal(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    action: &str,
    nonce: &str,
    hash: &str,
    typed_data: &Value,
    dispute: bool,
    reason: &str,
    evidence: &Value,
    author: &str,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_PROPOSAL,
            &[
                id, trade_id, &action, &nonce, &hash, typed_data, &dispute, &reason, evidence,
                &author, &key,
            ],
        )
        .await?;
    Ok(())
}

/// Returns `(hash, nonce, safe, expires_at, state)` for proposal approval.
pub(crate) async fn approval_context(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
    trade_id: &Uuid,
) -> Result<Option<(String, String, String, DateTime<Utc>, String)>> {
    Ok(client
        .query_opt(APPROVAL_CONTEXT, &[proposal_id, trade_id])
        .await?
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4))))
}

pub(crate) async fn proposal_by_id(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
    trade_id: &Uuid,
) -> Result<Option<(String, String, Value)>> {
    Ok(client
        .query_opt(PROPOSAL_BY_ID, &[proposal_id, trade_id])
        .await?
        .map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn approval_signatures(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
) -> Result<Vec<String>> {
    Ok(client
        .query(APPROVAL_SIGNATURES, &[proposal_id])
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

pub(crate) async fn approval_signature(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
    owner: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(APPROVAL_SIGNATURE, &[proposal_id, &owner])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn insert_approval(
    client: &(impl GenericClient + Sync),
    proposal_id: &Uuid,
    owner: &str,
    signature: &str,
) -> Result<()> {
    client
        .execute(INSERT_APPROVAL, &[proposal_id, &owner, &signature])
        .await?;
    Ok(())
}

/// Returns `(id, action, proposal_id, instructions, state, reference)` of an existing call key.
pub(crate) async fn call_by_key(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    actor: &str,
    key: &str,
) -> Result<Option<(Uuid, String, Option<Uuid>, Value, String, Option<String>)>> {
    Ok(client
        .query_opt(CALL_BY_KEY, &[trade_id, &actor, &key])
        .await?
        .map(|row| {
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
            )
        }))
}

pub(crate) async fn consent_by_owner(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    owner: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(CONSENT_BY_OWNER, &[trade_id, &owner])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn advisory_lock(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(ADVISORY_LOCK, &[&key]).await?;
    Ok(())
}

pub(crate) async fn has_unresolved_handoff(
    client: &(impl GenericClient + Sync),
    owner: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_HANDOFF, &[&owner])
        .await?
        .is_some())
}

pub(crate) async fn insert_call(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    actor: &str,
    action: &str,
    proposal_id: &Option<Uuid>,
    sender: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_CALL,
            &[
                id,
                trade_id,
                &actor,
                &action,
                proposal_id,
                &sender,
                instructions,
                &key,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn set_call_reference(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    actor: &str,
    reference: &str,
) -> std::result::Result<u64, tokio_postgres::Error> {
    client
        .execute(SET_CALL_REFERENCE, &[id, trade_id, &actor, &reference])
        .await
}

pub(crate) async fn attempt_by_key(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
    key: &str,
) -> Result<Option<(Uuid, Value, String)>> {
    Ok(client
        .query_opt(ATTEMPT_BY_KEY, &[trade_id, &kind, &key])
        .await?
        .map(|row| (row.get(0), row.get(1), row.get(2))))
}

/// Returns `(expires_at, state, observed_at)` of a locked EVER escrow.
pub(crate) async fn lock_ever_escrow(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(DateTime<Utc>, String, Option<DateTime<Utc>>)> {
    let row = client.query_one(ESCROW_EVER_STATE, &[trade_id]).await?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

pub(crate) async fn has_funding_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(FUNDING_EVIDENCE, &[trade_id])
        .await?
        .is_some())
}

pub(crate) async fn has_prior_attempt(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(PRIOR_ATTEMPT, &[trade_id, &kind])
        .await?
        .is_some())
}

pub(crate) async fn ever_vault(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<String> {
    Ok(client.query_one(ESCROW_VAULT, &[trade_id]).await?.get(0))
}

pub(crate) async fn eversale_advisory_lock(
    client: &(impl GenericClient + Sync),
    key: &str,
) -> Result<()> {
    client.query_one(ADVISORY_LOCK, &[&key]).await?;
    Ok(())
}

pub(crate) async fn has_eversale_handoff(
    client: &(impl GenericClient + Sync),
    sender: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(EVERSALE_HANDOFF, &[&sender])
        .await?
        .is_some())
}

pub(crate) async fn eversale_checkpoint(client: &(impl GenericClient + Sync)) -> Result<Value> {
    Ok(client.query_one(EVERSALE_CHECKPOINT, &[]).await?.get(0))
}

pub(crate) async fn insert_ever_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    kind: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_EVER_ATTEMPT,
            &[id, trade_id, &kind, instructions, &key],
        )
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

pub(crate) async fn confirm_attempt(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(CONFIRM_ATTEMPT, &[trade_id, &kind]).await?;
    Ok(())
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

pub(crate) async fn insert_ever_evidence(
    client: &(impl GenericClient + Sync),
    identity: &str,
    trade_id: &Uuid,
    kind: &str,
    evidence: &Value,
    included_at: DateTime<Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_EVIDENCE,
            &[&identity, trade_id, &kind, evidence, &included_at],
        )
        .await?;
    Ok(())
}

pub(crate) async fn verify_attempt(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    kind: &str,
) -> Result<()> {
    client.execute(VERIFY_ATTEMPT, &[trade_id, &kind]).await?;
    Ok(())
}

pub(crate) async fn debit_reservation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<()> {
    client.execute(DEBIT_RESERVATION, &[trade_id]).await?;
    Ok(())
}

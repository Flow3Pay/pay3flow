use crate::otc::error::Result;
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const INBOX_ACTIVITY: &str = "SELECT body FROM otc_inbox WHERE actor=$1 AND activity_id=$2";
const KNOWN_PROPOSAL: &str = "SELECT body FROM otc_rfq WHERE body->>'id'=$1 AND actor=$2 UNION ALL SELECT terms->'proposal' FROM otc_trades WHERE terms->'proposal'->>'id'=$1 AND desk_actor=$2";
const LOCK_DESK_OFFER: &str =
    "SELECT * FROM otc_trades WHERE offer->>'id'=$1 AND desk_actor=$2 FOR UPDATE";
const LOCK_CUSTOMER_OFFER: &str =
    "SELECT * FROM otc_trades WHERE offer->>'id'=$1 AND customer_actor=$2 FOR UPDATE";
const SET_TRADE_STATE: &str = "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1";
const INSERT_INBOX: &str = "INSERT INTO otc_inbox(actor,activity_id,body) VALUES($1,$2,$3)";
const MARK_DELIVERED: &str =
    "UPDATE otc_outbox SET delivered_at=now() WHERE activity_id=$1 AND actor=$2";
const ADVANCE_CURSOR: &str = "INSERT INTO otc_relay_cursors(actor,sequence) VALUES($1,$2) ON CONFLICT(actor) DO UPDATE SET sequence=GREATEST(otc_relay_cursors.sequence,$2)";
const LEASE_JOBS: &str = "UPDATE otc_outbox SET lease_until=now()+interval '60 seconds' WHERE activity_id IN (SELECT activity_id FROM otc_outbox WHERE delivered_at IS NULL AND queued_at IS NULL AND next_attempt_at<=now() AND (lease_until IS NULL OR lease_until<now()) ORDER BY CASE purpose WHEN 'subscription' THEN 0 ELSE 1 END,created_at FOR UPDATE SKIP LOCKED LIMIT 10) RETURNING activity_id,actor,body";
const MARK_QUEUED: &str = "UPDATE otc_outbox SET queued_at=now(),delivered_at=CASE WHEN purpose IN ('listing','subscription') THEN now() ELSE delivered_at END,lease_until=NULL WHERE activity_id=$1";
const RESCHEDULE_JOB: &str = "UPDATE otc_outbox SET attempts=attempts+1,next_attempt_at=now()+interval '15 seconds',lease_until=NULL WHERE activity_id=$1";
const PROVISIONED_ACTORS: &str =
    "SELECT actor FROM otc_actor_links WHERE NOT revoked AND provisioned";
const ACTOR_SEQUENCE: &str = "SELECT sequence FROM otc_relay_cursors WHERE actor=$1";

pub(crate) async fn inbox_activity(
    client: &(impl GenericClient + Sync),
    actor: &str,
    id: &str,
) -> Result<Option<Value>> {
    Ok(client
        .query_opt(INBOX_ACTIVITY, &[&actor, &id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn known_proposal(
    client: &(impl GenericClient + Sync),
    id: &str,
    author: &str,
) -> Result<Option<Value>> {
    Ok(client
        .query_opt(KNOWN_PROPOSAL, &[&id, &author])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn lock_desk_offer(
    client: &(impl GenericClient + Sync),
    id: &str,
    actor: &str,
) -> Result<Option<tokio_postgres::Row>> {
    Ok(client.query_opt(LOCK_DESK_OFFER, &[&id, &actor]).await?)
}

pub(crate) async fn lock_customer_offer(
    client: &(impl GenericClient + Sync),
    id: &str,
    actor: &str,
) -> Result<Option<tokio_postgres::Row>> {
    Ok(client
        .query_opt(LOCK_CUSTOMER_OFFER, &[&id, &actor])
        .await?)
}

pub(crate) async fn insert_inbox(
    client: &(impl GenericClient + Sync),
    actor: &str,
    id: &str,
    body: &Value,
) -> Result<()> {
    client.execute(INSERT_INBOX, &[&actor, &id, &body]).await?;
    Ok(())
}

pub(crate) async fn mark_delivered(
    client: &(impl GenericClient + Sync),
    id: &str,
    author: &str,
) -> Result<()> {
    client.execute(MARK_DELIVERED, &[&id, &author]).await?;
    Ok(())
}

pub(crate) async fn advance_cursor(
    client: &(impl GenericClient + Sync),
    actor: &str,
    sequence: i64,
) -> Result<()> {
    client.execute(ADVANCE_CURSOR, &[&actor, &sequence]).await?;
    Ok(())
}

/// Returns `(activity_id, actor, body)` leased for delivery.
#[allow(clippy::type_complexity)]
pub(crate) async fn lease_jobs(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, String, Value)>> {
    Ok(client
        .query(LEASE_JOBS, &[])
        .await?
        .iter()
        .map(|job| (job.get(0), job.get(1), job.get(2)))
        .collect())
}

pub(crate) async fn mark_queued(client: &(impl GenericClient + Sync), id: &str) -> Result<()> {
    client.execute(MARK_QUEUED, &[&id]).await?;
    Ok(())
}

pub(crate) async fn reschedule_job(client: &(impl GenericClient + Sync), id: &str) -> Result<()> {
    client.execute(RESCHEDULE_JOB, &[&id]).await?;
    Ok(())
}

pub(crate) async fn provisioned_actors(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<String>> {
    Ok(client
        .query(PROVISIONED_ACTORS, &[])
        .await?
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect())
}

pub(crate) async fn actor_sequence(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<Option<i64>> {
    Ok(client
        .query_opt(ACTOR_SEQUENCE, &[&actor])
        .await?
        .map(|row| row.get::<_, i64>(0)))
}

pub(crate) async fn set_trade_state(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    state: &str,
) -> Result<()> {
    client.execute(SET_TRADE_STATE, &[id, &state]).await?;
    Ok(())
}

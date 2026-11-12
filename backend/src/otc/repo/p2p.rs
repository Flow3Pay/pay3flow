use crate::otc::error::{invalid, Error, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use tokio_postgres::Row;
use uuid::Uuid;

const LOCK_OFFER: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1,0))";
const TRADE_BY_ACTOR_KEY: &str =
    "SELECT * FROM otc_p2p_trades WHERE taker_actor=$1 AND idempotency_key=$2";
const LOCK_PUBLIC_OFFER: &str = "SELECT * FROM otc_public_offers WHERE id=$1 FOR UPDATE";
const MARK_OFFER_TAKEN: &str = "UPDATE otc_public_offers SET state='taken' WHERE id=$1";
const INSERT_TRADE: &str = "INSERT INTO otc_p2p_trades(id,offer_id,owner_actor,taker_actor,body,state,idempotency_key,pay_by) VALUES($1,$2,$3,$4,$5,'payment_pending',$6,$7)";
const TRADE_BY_ID: &str = "SELECT * FROM otc_p2p_trades WHERE id=$1";
const TRADES_BY_ACTOR: &str = "SELECT * FROM otc_p2p_trades WHERE owner_actor=$1 OR taker_actor=$1 ORDER BY created_at DESC LIMIT 50";
const TRADE_BY_ACTOR: &str =
    "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2)";
const LOCK_TRADE_BY_ACTOR: &str =
    "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2) FOR UPDATE";
const CONFIRM_OWNER: &str = "UPDATE otc_p2p_trades SET owner_confirmed_at=COALESCE(owner_confirmed_at,now()),updated_at=now() WHERE id=$1 RETURNING *";
const CONFIRM_TAKER: &str = "UPDATE otc_p2p_trades SET taker_confirmed_at=COALESCE(taker_confirmed_at,now()),updated_at=now() WHERE id=$1 RETURNING *";
const ATTEMPTS_BY_TRADES: &str = "SELECT id,trade_id,kind,state,instructions,reference FROM otc_p2p_attempts WHERE trade_id = ANY($1) ORDER BY created_at";
const ATTEMPT_BY_KEY: &str =
    "SELECT id,instructions,state FROM otc_p2p_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3";
const FEE_COLLECTED: &str = "SELECT id FROM otc_p2p_fees WHERE trade_id=$1 AND state='collected'";
const LOCK_ACCRUED_FEE_UNITS_FIRST: &str = "SELECT fee_units::text,payer_actor,asset,precision FROM otc_p2p_fees WHERE trade_id=$1 AND state='accrued' FOR UPDATE";
const LOCK_ACCRUED_FEE_PAYER_FIRST: &str = "SELECT payer_actor,fee_units::text,asset,precision FROM otc_p2p_fees WHERE trade_id=$1 AND state='accrued' FOR UPDATE";
const VERIFIED_PAYMENT_INSTRUCTIONS: &str = "SELECT instructions FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND state='verified' ORDER BY updated_at DESC LIMIT 1";
const UNRESOLVED_ATTEMPT: &str =
    "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind=$2 AND state!='failed'";
const INSERT_ATTEMPT: &str = "INSERT INTO otc_p2p_attempts(id,trade_id,kind,payer_actor,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,'prepared',$6,$7)";
const TRADE_BODY_BY_ACTOR: &str =
    "SELECT body FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2)";
const PAYMENT_ATTEMPT_BY_KEY: &str = "SELECT id,state,instructions FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND idempotency_key=$2";
const UNRESOLVED_PAYMENT_ATTEMPT: &str =
    "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND state!='failed'";
const INSERT_NEAR_PAYMENT_ATTEMPT: &str = "INSERT INTO otc_p2p_attempts(id,trade_id,kind,payer_actor,chain,state,instructions,idempotency_key) VALUES($1,$2,'payment',$3,'near-intents','prepared',$4,$5)";
const FEE_ATTEMPT_BY_KEY: &str = "SELECT id,state,instructions FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='fee' AND idempotency_key=$2";
const UNRESOLVED_FEE_ATTEMPT: &str =
    "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='fee' AND state!='failed'";
const INSERT_NEAR_FEE_ATTEMPT: &str = "INSERT INTO otc_p2p_attempts(id,trade_id,kind,payer_actor,chain,state,instructions,idempotency_key) VALUES($1,$2,'fee',$3,'near-intents','prepared',$4,$5)";
const ATTEMPT_SUBMIT_CONTEXT: &str = "SELECT a.trade_id,a.kind,a.reference,a.chain,t.owner_actor,t.taker_actor,a.instructions FROM otc_p2p_attempts a JOIN otc_p2p_trades t ON t.id=a.trade_id WHERE a.id=$1";
const SUBMIT_ATTEMPT: &str = "UPDATE otc_p2p_attempts SET reference=$2,state=CASE WHEN state='prepared' THEN 'submitted' ELSE state END,updated_at=now() WHERE id=$1";
const LOCK_TRADE: &str = "SELECT * FROM otc_p2p_trades WHERE id=$1 FOR UPDATE";
const OPEN_PAYMENT_ATTEMPT: &str = "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND state NOT IN ('failed','verified')";
const CANCEL_TRADE: &str =
    "UPDATE otc_p2p_trades SET state='cancelled',updated_at=now() WHERE id=$1";
const REOPEN_OFFER: &str =
    "UPDATE otc_public_offers SET state='open' WHERE id=$1 AND state='taken' AND expires_at>now()";
const ATTEMPTS_TO_POLL: &str = "SELECT a.id,a.trade_id,a.kind,a.chain,a.instructions,a.reference FROM otc_p2p_attempts a JOIN otc_p2p_trades t ON t.id=a.trade_id WHERE a.state IN ('prepared','submitted','confirming') AND (t.state IN ('payment_pending','delivery_pending') OR (t.state='completed' AND a.kind='fee')) ORDER BY a.created_at LIMIT 50";
const FAIL_POLLED_ATTEMPT: &str = "UPDATE otc_p2p_attempts SET state='failed',evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('prepared','submitted','confirming')";
const CONFIRM_POLLED_ATTEMPT: &str = "UPDATE otc_p2p_attempts SET state='confirming',evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('submitted','confirming')";
const SET_ATTEMPT_REFERENCE: &str =
    "UPDATE otc_p2p_attempts SET reference=$2,state='submitted' WHERE id=$1 AND reference IS NULL";
const FAIL_ATTEMPT: &str =
    "UPDATE otc_p2p_attempts SET state='failed',updated_at=now() WHERE id=$1";
const KEEP_CONFIRMING: &str = "UPDATE otc_p2p_attempts SET state='confirming',updated_at=now() WHERE id=$1 AND state='submitted'";
const STALE_TRADES: &str = "SELECT id,offer_id FROM otc_p2p_trades WHERE state='payment_pending' AND pay_by<now() AND NOT EXISTS(SELECT 1 FROM otc_p2p_attempts WHERE trade_id=otc_p2p_trades.id AND kind='payment' AND state='verified')";
const EXPIRE_TRADE: &str =
    "UPDATE otc_p2p_trades SET state='expired',updated_at=now() WHERE id=$1 AND state='payment_pending'";
const OVERDUE_TRADES: &str = "SELECT id FROM otc_p2p_trades WHERE state='delivery_pending' AND payout_by IS NOT NULL AND payout_by<now()";
const LOCK_FEE_ATTEMPT: &str = "SELECT payer_actor,instructions,chain FROM otc_p2p_attempts WHERE id=$1 AND trade_id=$2 AND kind='fee' FOR UPDATE";
const VERIFY_FEE_ATTEMPT: &str = "UPDATE otc_p2p_attempts SET state='verified',evidence=$2,updated_at=now() WHERE id=$1 AND kind='fee' AND state IN ('submitted','confirming','prepared')";
const COLLECT_FEE: &str =
    "UPDATE otc_p2p_fees SET state='collected',collected_at=now() WHERE trade_id=$1 AND state='accrued'";
const VERIFY_ATTEMPT: &str =
    "UPDATE otc_p2p_attempts SET state='verified',evidence=$2,updated_at=now() WHERE id=$1";
const ADVANCE_TRADE: &str =
    "UPDATE otc_p2p_trades SET state=$2,payout_by=$3,updated_at=now() WHERE id=$1";
const INSERT_FEE: &str = "INSERT INTO otc_p2p_fees(id,trade_id,offer_id,asset,basis_asset,precision,basis_units,fee_units,net_units,rate,version,state,payer_actor) VALUES($1,$2,$3,$4,$5,6,$6::text::numeric,$7::text::numeric,$8::text::numeric,$9::text::numeric,$10,'accrued',$11) ON CONFLICT (trade_id) DO NOTHING";
const SETTLE_OFFER: &str =
    "UPDATE otc_public_offers SET state='settled' WHERE id=$1 AND state='taken'";

/// Advisory lock keyed on an offer; format the key at the call site.
pub(crate) async fn lock_offer(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(LOCK_OFFER, &[&key]).await?;
    Ok(())
}

pub(crate) async fn trade_by_actor_key(
    client: &(impl GenericClient + Sync),
    actor: &str,
    key: &str,
) -> Result<Option<Row>> {
    Ok(client
        .query_opt(TRADE_BY_ACTOR_KEY, &[&actor, &key])
        .await?)
}

pub(crate) async fn lock_public_offer(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<Row>> {
    Ok(client.query_opt(LOCK_PUBLIC_OFFER, &[id]).await?)
}

pub(crate) async fn mark_offer_taken(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<()> {
    client.execute(MARK_OFFER_TAKEN, &[id]).await?;
    Ok(())
}

pub(crate) async fn insert_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    offer_id: &Uuid,
    owner: &str,
    taker: &str,
    body: &Value,
    key: &str,
    pay_by: &DateTime<Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_TRADE,
            &[&id, &offer_id, &owner, &taker, &body, &key, &pay_by],
        )
        .await?;
    Ok(())
}

pub(crate) async fn trade_by_id(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<Row> {
    Ok(client.query_one(TRADE_BY_ID, &[id]).await?)
}

pub(crate) async fn trades_by_actor(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<Vec<Row>> {
    Ok(client.query(TRADES_BY_ACTOR, &[&actor]).await?)
}

pub(crate) async fn trade_by_actor(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Option<Row>> {
    Ok(client.query_opt(TRADE_BY_ACTOR, &[id, &actor]).await?)
}

pub(crate) async fn lock_trade_by_actor(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Option<Row>> {
    Ok(client.query_opt(LOCK_TRADE_BY_ACTOR, &[id, &actor]).await?)
}

pub(crate) async fn confirm_by_owner(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Row> {
    Ok(client.query_one(CONFIRM_OWNER, &[id]).await?)
}

pub(crate) async fn confirm_by_taker(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Row> {
    Ok(client.query_one(CONFIRM_TAKER, &[id]).await?)
}

/// Returns `(trade_id, attempt view)` pairs ordered by `created_at`.
pub(crate) async fn attempts_by_trades(
    client: &(impl GenericClient + Sync),
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Value)>> {
    let rows = client.query(ATTEMPTS_BY_TRADES, &[&ids]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get::<_, Uuid>("trade_id"),
                serde_json::json!({"id": row.get::<_, Uuid>("id"), "kind": row.get::<_, String>("kind"),
                    "state": row.get::<_, String>("state"),
                    "instructions": row.get::<_, Value>("instructions"),
                    "reference": row.get::<_, Option<String>>("reference")}),
            )
        })
        .collect())
}

pub(crate) async fn attempt_by_key(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
    key: &str,
) -> Result<Option<(Uuid, Value, String)>> {
    let row = client.query_opt(ATTEMPT_BY_KEY, &[id, &kind, &key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn fee_collected(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<bool> {
    Ok(client.query_opt(FEE_COLLECTED, &[id]).await?.is_some())
}

/// Returns `(fee_units, payer_actor, asset, precision)` with `fee_units` as decimal text.
pub(crate) async fn lock_accrued_fee_units_first(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<(String, String, String, i16)> {
    let row = client
        .query_opt(LOCK_ACCRUED_FEE_UNITS_FIRST, &[id])
        .await?
        .ok_or_else(|| {
            Error::Conflict("there is no outstanding service fee for this deal".into())
        })?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

/// Returns `(payer_actor, fee_units, asset, precision)` with `fee_units` as decimal text.
pub(crate) async fn lock_accrued_fee_payer_first(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<(String, String, String, i16)>> {
    let row = client
        .query_opt(LOCK_ACCRUED_FEE_PAYER_FIRST, &[id])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2), row.get(3))))
}

pub(crate) async fn verified_payment_instructions(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Value> {
    let row = client
        .query_opt(VERIFIED_PAYMENT_INSTRUCTIONS, &[id])
        .await?
        .ok_or_else(|| {
            Error::Conflict("verified stablecoin payment is required before fee collection".into())
        })?;
    Ok(row.get(0))
}

pub(crate) async fn unresolved_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    kind: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_ATTEMPT, &[id, &kind])
        .await?
        .is_some())
}

pub(crate) async fn insert_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    id: &Uuid,
    kind: &str,
    actor: &str,
    chain: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_ATTEMPT,
            &[&attempt, &id, &kind, &actor, &chain, &instructions, &key],
        )
        .await?;
    Ok(())
}

pub(crate) async fn trade_body_by_actor(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Value> {
    let row = client
        .query_opt(TRADE_BODY_BY_ACTOR, &[id, &actor])
        .await?
        .ok_or(Error::NotFound)?;
    Ok(row.get(0))
}

pub(crate) async fn payment_attempt_by_key(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    key: &str,
) -> Result<Option<(Uuid, String, Value)>> {
    let row = client
        .query_opt(PAYMENT_ATTEMPT_BY_KEY, &[id, &key])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn unresolved_payment_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_PAYMENT_ATTEMPT, &[id])
        .await?
        .is_some())
}

pub(crate) async fn insert_near_payment_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    id: &Uuid,
    actor: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_NEAR_PAYMENT_ATTEMPT,
            &[&attempt, &id, &actor, &instructions, &key],
        )
        .await?;
    Ok(())
}

pub(crate) async fn fee_attempt_by_key(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    key: &str,
) -> Result<Option<(Uuid, String, Value)>> {
    let row = client.query_opt(FEE_ATTEMPT_BY_KEY, &[id, &key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn unresolved_fee_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(UNRESOLVED_FEE_ATTEMPT, &[id])
        .await?
        .is_some())
}

pub(crate) async fn insert_near_fee_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    id: &Uuid,
    actor: &str,
    instructions: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_NEAR_FEE_ATTEMPT,
            &[&attempt, &id, &actor, &instructions, &key],
        )
        .await?;
    Ok(())
}

/// Returns `(trade_id, kind, reference, chain, owner_actor, taker_actor, instructions)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn attempt_submit_context(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<Option<(Uuid, String, Option<String>, String, String, String, Value)>> {
    let row = client.query_opt(ATTEMPT_SUBMIT_CONTEXT, &[attempt]).await?;
    Ok(row.map(|row| {
        (
            row.get(0),
            row.get(1),
            row.get(2),
            row.get(3),
            row.get(4),
            row.get(5),
            row.get(6),
        )
    }))
}

pub(crate) async fn submit_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    reference: &str,
) -> Result<()> {
    client
        .execute(SUBMIT_ATTEMPT, &[attempt, &reference])
        .await?;
    Ok(())
}

pub(crate) async fn lock_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<Row> {
    client
        .query_opt(LOCK_TRADE, &[id])
        .await?
        .ok_or(Error::NotFound)
}

pub(crate) async fn open_payment_attempt(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client
        .query_opt(OPEN_PAYMENT_ATTEMPT, &[id])
        .await?
        .is_some())
}

pub(crate) async fn cancel_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(CANCEL_TRADE, &[id]).await?;
    Ok(())
}

pub(crate) async fn reopen_offer(
    client: &(impl GenericClient + Sync),
    offer_id: &Uuid,
) -> Result<()> {
    client.execute(REOPEN_OFFER, &[offer_id]).await?;
    Ok(())
}

/// Returns `(id, trade_id, kind, chain, instructions, reference)` per polled attempt.
#[allow(clippy::type_complexity)]
pub(crate) async fn attempts_to_poll(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, Uuid, String, String, Value, Option<String>)>> {
    let rows = client.query(ATTEMPTS_TO_POLL, &[]).await?;
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

pub(crate) async fn fail_polled_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(FAIL_POLLED_ATTEMPT, &[attempt, evidence])
        .await?;
    Ok(())
}

pub(crate) async fn confirm_polled_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(CONFIRM_POLLED_ATTEMPT, &[attempt, evidence])
        .await?;
    Ok(())
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

pub(crate) async fn fail_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<()> {
    client.execute(FAIL_ATTEMPT, &[attempt]).await?;
    Ok(())
}

pub(crate) async fn keep_confirming(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
) -> Result<()> {
    client.execute(KEEP_CONFIRMING, &[attempt]).await?;
    Ok(())
}

/// Returns `(trade_id, offer_id)` for unpaid trades past `pay_by`.
pub(crate) async fn stale_trades(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, Uuid)>> {
    let rows = client.query(STALE_TRADES, &[]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub(crate) async fn expire_trade(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(EXPIRE_TRADE, &[id]).await?;
    Ok(())
}

pub(crate) async fn overdue_trades(client: &(impl GenericClient + Sync)) -> Result<Vec<Uuid>> {
    let rows = client.query(OVERDUE_TRADES, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

/// Returns `(payer_actor, instructions, chain)`.
pub(crate) async fn lock_fee_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    id: &Uuid,
) -> Result<(String, Value, String)> {
    let row = client
        .query_opt(LOCK_FEE_ATTEMPT, &[attempt, id])
        .await?
        .ok_or_else(|| invalid("fee attempt was not found"))?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

pub(crate) async fn verify_fee_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(VERIFY_FEE_ATTEMPT, &[attempt, evidence])
        .await?;
    Ok(())
}

pub(crate) async fn collect_fee(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(COLLECT_FEE, &[id]).await?;
    Ok(())
}

pub(crate) async fn verify_attempt(
    client: &(impl GenericClient + Sync),
    attempt: &Uuid,
    evidence: &Value,
) -> Result<()> {
    client.execute(VERIFY_ATTEMPT, &[attempt, evidence]).await?;
    Ok(())
}

pub(crate) async fn advance_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    state: &str,
    payout_by: &Option<DateTime<Utc>>,
) -> Result<()> {
    client
        .execute(ADVANCE_TRADE, &[id, &state, &payout_by])
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_fee(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    offer_id: &Uuid,
    asset: &str,
    basis_asset: &str,
    basis_units: &str,
    fee_units: &str,
    net_units: &str,
    rate: &str,
    version: &str,
    payer_actor: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_FEE,
            &[
                &Uuid::new_v4(),
                &id,
                &offer_id,
                &asset,
                &basis_asset,
                &basis_units,
                &fee_units,
                &net_units,
                &rate,
                &version,
                &payer_actor,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn settle_offer(
    client: &(impl GenericClient + Sync),
    offer_id: &Uuid,
) -> Result<()> {
    client.execute(SETTLE_OFFER, &[offer_id]).await?;
    Ok(())
}

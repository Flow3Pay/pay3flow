use super::trade;
use crate::otc::{
    error::{Error, Result},
    model::{Terms, Trade},
};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const RFQ_BY_KEY: &str = "SELECT id,body FROM otc_rfq WHERE actor=$1 AND idempotency_key=$2";
const RFQ_ACTOR: &str = "SELECT * FROM otc_rfq WHERE id=$1 FOR UPDATE";
const RFQ_ACTOR_ONLY: &str = "SELECT actor FROM otc_rfq WHERE id=$1";
const TRADE_BY_RFQ: &str = "SELECT * FROM otc_trades WHERE rfq_id=$1";
const INBOX_HAS: &str = "SELECT activity_id FROM otc_inbox WHERE actor=$1 AND activity_id=$2";
const BOOKING_PENDING: &str =
    "SELECT id FROM otc_trades WHERE customer_actor=$1 AND state='booking_pending' LIMIT 1";
const CUSTOMER_QUOTED: &str =
    "SELECT * FROM otc_trades WHERE id=$1 AND customer_actor=$2 FOR UPDATE";
const DESK_QUOTED: &str = "SELECT * FROM otc_trades WHERE id=$1 AND desk_actor=$2 FOR UPDATE";
const INSERT_RFQ: &str = "INSERT INTO otc_rfq(id,actor,direction,input,listing_id,body,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7)";
const INSERT_TRADE: &str = "INSERT INTO otc_trades(id,rfq_id,customer_actor,desk_actor,terms,state) VALUES($1,$2,$3,$4,$5,'quoted')";
const MARK_RFQ_QUOTED: &str = "UPDATE otc_rfq SET state='quoted' WHERE id=$1";
const MARK_BOOKING_PENDING: &str =
    "UPDATE otc_trades SET state='booking_pending',offer=$2,updated_at=now() WHERE id=$1";
const SET_DECISION: &str = "UPDATE otc_trades SET decision=$2,updated_at=now() WHERE id=$1";
const RESERVED_INVENTORY: &str = "SELECT COALESCE(sum(amount),0)::text FROM otc_reservations WHERE chain=$1 AND status!='released'";
const INSERT_RESERVATION: &str = "INSERT INTO otc_reservations(trade_id,chain,amount,status) VALUES($1,$2,$3::text::numeric,'active')";
// Shared with `transfers::prepare` and `wallet::prove`; formatted at the call site.
pub(crate) const ACTOR_LOCK: &str = "SELECT pg_advisory_xact_lock(hashtext($1))";

pub(crate) async fn rfq_by_key(
    client: &(impl GenericClient + Sync),
    actor: &str,
    key: &str,
) -> Result<Option<(Uuid, Value)>> {
    let row = client.query_opt(RFQ_BY_KEY, &[&actor, &key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

pub(crate) async fn pending_booking(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(BOOKING_PENDING, &[&actor])
        .await?
        .is_some())
}

pub(crate) async fn insert_rfq(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
    direction: &str,
    input: &str,
    listing_id: &str,
    body: &Value,
    key: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_RFQ,
            &[&id, &actor, &direction, &input, &listing_id, &body, &key],
        )
        .await?;
    Ok(())
}

pub(crate) async fn rfq_actor(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(RFQ_ACTOR_ONLY, &[id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn lock_rfq(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<tokio_postgres::Row> {
    client
        .query_opt(RFQ_ACTOR, &[id])
        .await?
        .ok_or(Error::NotFound)
}

pub(crate) async fn trade_by_rfq(
    client: &(impl GenericClient + Sync),
    rfq_id: &Uuid,
) -> Result<Option<Trade>> {
    let Some(row) = client.query_opt(TRADE_BY_RFQ, &[rfq_id]).await? else {
        return Ok(None);
    };
    trade(&row).map(Some)
}

pub(crate) async fn inbox_has(
    client: &(impl GenericClient + Sync),
    actor: &str,
    activity_id: &str,
) -> Result<bool> {
    Ok(client
        .query_opt(INBOX_HAS, &[&actor, &activity_id])
        .await?
        .is_some())
}

pub(crate) async fn insert_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    rfq_id: &Uuid,
    terms: &Terms,
    desk_actor: &str,
    serialized: &Value,
) -> Result<()> {
    client
        .execute(
            INSERT_TRADE,
            &[
                &id,
                &rfq_id,
                &terms.customer_actor,
                &desk_actor,
                &serialized,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn mark_rfq_quoted(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(MARK_RFQ_QUOTED, &[id]).await?;
    Ok(())
}

pub(crate) async fn lock_customer_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Trade> {
    let row = client
        .query_opt(CUSTOMER_QUOTED, &[id, &actor])
        .await?
        .ok_or(Error::NotFound)?;
    trade(&row)
}

pub(crate) async fn mark_booking_pending(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    offer: &Value,
) -> Result<()> {
    client.execute(MARK_BOOKING_PENDING, &[id, &offer]).await?;
    Ok(())
}

pub(crate) async fn lock_desk_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Trade> {
    let row = client
        .query_opt(DESK_QUOTED, &[id, &actor])
        .await?
        .ok_or(Error::NotFound)?;
    trade(&row)
}

pub(crate) async fn set_decision(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    decision: &Value,
) -> Result<()> {
    client.execute(SET_DECISION, &[id, &decision]).await?;
    Ok(())
}

pub(crate) async fn reserved_inventory(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<String> {
    Ok(client
        .query_one(RESERVED_INVENTORY, &[&chain])
        .await?
        .get(0))
}

pub(crate) async fn insert_reservation(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    chain: &str,
    amount: &str,
) -> Result<()> {
    client
        .execute(INSERT_RESERVATION, &[trade_id, &chain, &amount])
        .await?;
    Ok(())
}

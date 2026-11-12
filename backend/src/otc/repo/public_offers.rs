use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const OFFERS_BY_ACTOR: &str =
    "SELECT * FROM otc_public_offers WHERE actor=$1 ORDER BY created_at DESC LIMIT 100";
const OPEN_OFFERS: &str = "SELECT * FROM otc_public_offers WHERE state='open' AND expires_at>now() AND body->>'ethereum_chain_id'='1' AND COALESCE(body->>'stablecoin','USDT') IN ('USDT','USDC') ORDER BY created_at DESC LIMIT 100";
const OFFER_HISTORY: &str = "SELECT id, body, created_at FROM (\n                   SELECT id, body, created_at FROM otc_public_offers\n                   ORDER BY created_at DESC LIMIT $1\n                 ) recent ORDER BY created_at ASC";
const LOCK_ACCOUNT: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1,0))";
const OFFER_BY_KEY: &str = "SELECT * FROM otc_public_offers WHERE actor=$1 AND idempotency_key=$2";
const INSERT_OFFER: &str = "INSERT INTO otc_public_offers(id,actor,seller_ever,body,idempotency_key) VALUES($1,$2,$3,$4,$5) RETURNING *";
const CANCEL_OFFER: &str = "UPDATE otc_public_offers SET state='cancelled' WHERE id=$1 AND actor=$2 AND state='open' RETURNING *";

/// An offer row: `(id, seller_ever, body, state, created_at, expires_at)`.
pub(crate) type OfferRow = (Uuid, String, Value, String, DateTime<Utc>, DateTime<Utc>);

/// Returns one [`OfferRow`] per offer.
pub(crate) async fn offers_by_actor(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<Vec<OfferRow>> {
    let rows = client.query(OFFERS_BY_ACTOR, &[&actor]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get("id"),
                row.get("seller_ever"),
                row.get("body"),
                row.get("state"),
                row.get("created_at"),
                row.get("expires_at"),
            )
        })
        .collect())
}

/// Returns one [`OfferRow`] per open offer.
pub(crate) async fn open_offers(client: &(impl GenericClient + Sync)) -> Result<Vec<OfferRow>> {
    let rows = client.query(OPEN_OFFERS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get("id"),
                row.get("seller_ever"),
                row.get("body"),
                row.get("state"),
                row.get("created_at"),
                row.get("expires_at"),
            )
        })
        .collect())
}

/// Returns `(id, body, created_at)` for the most recent offers, oldest first.
pub(crate) async fn offer_history(
    client: &(impl GenericClient + Sync),
    limit: i64,
) -> Result<Vec<(Uuid, Value, DateTime<Utc>)>> {
    let rows = client.query(OFFER_HISTORY, &[&limit]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("id"), row.get("body"), row.get("created_at")))
        .collect())
}

pub(crate) async fn lock_account(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(LOCK_ACCOUNT, &[&key]).await?;
    Ok(())
}

/// Returns `(id, seller_ever, body)` of an existing publication with this key.
pub(crate) async fn offer_by_key(
    client: &(impl GenericClient + Sync),
    actor: &str,
    key: &str,
) -> Result<Option<(Uuid, String, Value)>> {
    let row = client.query_opt(OFFER_BY_KEY, &[&actor, &key]).await?;
    Ok(row.map(|row| (row.get("id"), row.get("seller_ever"), row.get("body"))))
}

/// Returns the [`OfferRow`] of the new offer.
pub(crate) async fn insert_offer(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
    seller: &str,
    terms: &Value,
    key: &str,
) -> Result<OfferRow> {
    let row = client
        .query_one(INSERT_OFFER, &[id, &actor, &seller, terms, &key])
        .await?;
    Ok((
        row.get("id"),
        row.get("seller_ever"),
        row.get("body"),
        row.get("state"),
        row.get("created_at"),
        row.get("expires_at"),
    ))
}

/// Returns the [`OfferRow`] of the cancelled offer.
pub(crate) async fn cancel_offer(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<Option<OfferRow>> {
    let row = client.query_opt(CANCEL_OFFER, &[id, &actor]).await?;
    Ok(row.map(|row| {
        (
            row.get("id"),
            row.get("seller_ever"),
            row.get("body"),
            row.get("state"),
            row.get("created_at"),
            row.get("expires_at"),
        )
    }))
}

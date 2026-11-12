use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio_postgres::{Client, Error};

const BOOK: &str = "SELECT id, body, expires_at FROM otc_public_offers \
                 WHERE state='open' AND expires_at>now() \
                 ORDER BY created_at DESC LIMIT $1";

/// Returns `(id, body, expires_at)` per open offer, newest first.
pub(crate) async fn book(
    client: &Client,
    rows: i64,
) -> Result<Vec<(uuid::Uuid, Value, DateTime<Utc>)>, Error> {
    let fetched = client.query(BOOK, &[&rows]).await?;
    Ok(fetched
        .iter()
        .map(|row| (row.get("id"), row.get("body"), row.get("expires_at")))
        .collect())
}

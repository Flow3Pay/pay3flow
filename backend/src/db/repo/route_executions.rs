use anyhow::Result;
use serde_json::Value;
use uuid::Uuid;

use crate::db::DbPoolObject;

const COUNT_COMPLETED_LAST_HOUR: &str = "SELECT COUNT(*)::BIGINT FROM route_executions \
             WHERE status = 'completed' AND updated_at >= now() - INTERVAL '1 hour'";
const INSERT: &str = r#"
INSERT INTO route_executions
    (id, anonymous_id, idempotency_key, route_id, provider, status,
     from_asset, to_asset, input_amount, source_address, recipient,
     action, provider_reference, quote_expires_at)
VALUES ($1, $2, $3, $4, $5, 'awaiting_signature', $6, $7, $8, $9, $10, $11, $12, $13)
"#;
const MARK_SUBMITTED: &str = r#"
UPDATE route_executions
SET status = 'submitted', submitted_reference = $3, updated_at = now()
WHERE id = $1 AND anonymous_id = $2 AND status = 'awaiting_signature'
"#;
const SET_STATUS: &str = r#"
UPDATE route_executions
SET status = $3, provider_status = $4, updated_at = now()
WHERE id = $1 AND anonymous_id = $2
"#;
const ID_BY_IDEMPOTENCY: &str =
    "SELECT id FROM route_executions WHERE anonymous_id = $1 AND idempotency_key = $2";
const GET_STORED: &str = r#"
SELECT route_id, provider, status, from_asset, to_asset, input_amount,
       source_address, recipient, action, provider_reference,
       submitted_reference, provider_status, quote_expires_at, updated_at
FROM route_executions
WHERE id = $1 AND anonymous_id = $2
"#;

pub async fn count_completed_last_hour(client: &DbPoolObject) -> Result<i64> {
    let row = client.query_one(COUNT_COMPLETED_LAST_HOUR, &[]).await?;
    Ok(row.get(0))
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    client: &DbPoolObject,
    id: &Uuid,
    anonymous_id: &Uuid,
    idempotency_key: &str,
    route_id: &str,
    provider: &str,
    from_asset: &str,
    to_asset: &str,
    input_amount: &str,
    source_address: &str,
    recipient: &str,
    action: &Value,
    provider_reference: Option<&str>,
    quote_expires_at: &chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT,
            &[
                &id,
                &anonymous_id,
                &idempotency_key,
                &route_id,
                &provider,
                &from_asset,
                &to_asset,
                &input_amount,
                &source_address,
                &recipient,
                &action,
                &provider_reference,
                &quote_expires_at,
            ],
        )
        .await?;
    Ok(())
}

pub async fn mark_submitted(
    client: &DbPoolObject,
    id: &Uuid,
    anonymous_id: &Uuid,
    reference: &str,
) -> Result<u64> {
    Ok(client
        .execute(MARK_SUBMITTED, &[&id, &anonymous_id, &reference])
        .await?)
}

pub async fn set_status(
    client: &DbPoolObject,
    id: &Uuid,
    anonymous_id: &Uuid,
    status: &str,
    provider_status: &Value,
) -> Result<()> {
    client
        .execute(SET_STATUS, &[&id, &anonymous_id, &status, &provider_status])
        .await?;
    Ok(())
}

pub async fn id_by_idempotency(
    client: &DbPoolObject,
    anonymous_id: &Uuid,
    key: &str,
) -> Result<Option<Uuid>> {
    let row = client
        .query_opt(ID_BY_IDEMPOTENCY, &[&anonymous_id, &key])
        .await?;
    Ok(row.map(|row| row.get(0)))
}

pub type StoredRow = tokio_postgres::Row;

pub async fn get_stored(
    client: &DbPoolObject,
    id: &Uuid,
    anonymous_id: &Uuid,
) -> Result<Option<StoredRow>> {
    Ok(client.query_opt(GET_STORED, &[&id, &anonymous_id]).await?)
}

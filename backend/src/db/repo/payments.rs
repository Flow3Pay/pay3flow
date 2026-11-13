//! Payment transaction, route, acquirer and webhook statements.

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::db::repo::reputation::Conn;
use crate::payments::model::{Minor, NewRoute, NewTransaction};
use crate::payments::status::{RouteStatus, TransactionStatus};

const INSERT_TX: &str = r#"
INSERT INTO transactions
    (user_id, from_amount, from_currency, to_currency, from_account, to_account, method, fees, idempotency_key)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING
RETURNING id, user_id, status, from_amount, from_currency, to_amount, to_currency,
          from_account, to_account, method, fees, provider, external_id, idempotency_key, created_at, updated_at
"#;

const SELECT_TX: &str = r#"
SELECT id, user_id, status, from_amount, from_currency, to_amount, to_currency,
       from_account, to_account, method, fees, provider, external_id, idempotency_key, created_at, updated_at
FROM transactions
"#;

const TX_BY_ID: &str = "SELECT_TX_PREFIX WHERE id = $1";
const TX_BY_IDEMPOTENCY_KEY: &str = "SELECT_TX_PREFIX WHERE idempotency_key = $1";
const TXS_FOR_USER: &str = "SELECT_TX_PREFIX WHERE user_id = $1 ORDER BY created_at DESC";
const TRANSITION_TX: &str =
    "UPDATE transactions SET status = $2, updated_at = now() WHERE id = $1 AND status = $3";
const SET_PROVIDER_FIELDS: &str =
    "UPDATE transactions SET provider = $2, external_id = $3, updated_at = now() WHERE id = $1";
const SET_AMOUNTS: &str = r#"
UPDATE transactions
SET to_amount = $2, to_currency = $3, fees = $4, updated_at = now()
WHERE id = $1
"#;
const INSERT_ROUTE: &str = r#"
INSERT INTO routes (transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source)
VALUES ($1, $2, $3, $4, $5, $6, $7)
RETURNING id, transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source, created_at
"#;
const ROUTE_FOR_TRANSACTION: &str = r#"
SELECT id, transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source, created_at
FROM routes WHERE transaction_id = $1
"#;
const TRANSITION_ROUTE: &str =
    "UPDATE routes SET status = $2, updated_at = now() WHERE id = $1 AND status = $3";
const ACQUIRER_BY_SLUG: &str =
    "SELECT id, slug, name, fee_percent, fee_fixed, active FROM acquirers WHERE slug = $1";
const CREDENTIAL_ENCRYPTED: &str =
    "SELECT value_encrypted FROM credentials WHERE acquirer_id = $1 AND name = $2";
const INSERT_WEBHOOK: &str = r#"
INSERT INTO provider_webhooks (provider, event_id, event_type, payload, signature)
VALUES ($1, $2, $3, $4, $5)
RETURNING id
"#;
const MARK_WEBHOOK_PROCESSED: &str =
    "UPDATE provider_webhooks SET status = $2, error = $3, processed_at = now() WHERE id = $1";

/// `SELECT_TX` without its trailing newline, for the one-line query variants.
fn tx_select_prefix() -> &'static str {
    SELECT_TX
        .strip_suffix('\n')
        .expect("SELECT_TX ends with a newline")
}

/// `(id, user_id, status, from_amount, from_currency, to_amount, to_currency,
/// from_account, to_account, method, fees, provider, external_id,
/// idempotency_key, created_at, updated_at)`.
pub async fn insert_transaction<C>(
    client: &C,
    tx: &NewTransaction,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(INSERT_TX).await?;
    Ok(client
        .query_opt(
            &stmt,
            &[
                &tx.user_id,
                &tx.from_amount,
                &tx.from_currency,
                &tx.to_currency,
                &tx.from_account,
                &tx.to_account,
                &tx.method,
                &tx.fees,
                &tx.idempotency_key,
            ],
        )
        .await?)
}

pub async fn transaction_by_id<C>(client: &C, id: &Uuid) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client
        .prepare_cached(&tx_select_prefix().replace("SELECT_TX_PREFIX", SELECT_TX))
        .await?;
    Ok(client.query_opt(&stmt, &[id]).await?)
}

pub async fn transaction_by_idempotency_key<C>(
    client: &C,
    key: &str,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client
        .prepare_cached(&tx_select_prefix().replace("SELECT_TX_PREFIX", SELECT_TX))
        .await?;
    Ok(client.query_opt(&stmt, &[&key]).await?)
}

pub async fn transactions_for_user<C>(
    client: &C,
    user_id: &Uuid,
) -> Result<Vec<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client
        .prepare_cached(&tx_select_prefix().replace("SELECT_TX_PREFIX", SELECT_TX))
        .await?;
    Ok(client.query(&stmt, &[user_id]).await?)
}

pub async fn transition_status<C>(
    client: &C,
    id: &Uuid,
    from: TransactionStatus,
    to: TransactionStatus,
) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(TRANSITION_TX).await?;
    let n = client
        .execute(&stmt, &[id, &to.as_str(), &from.as_str()])
        .await?;
    Ok(n == 1)
}

pub async fn set_provider_fields<C>(
    client: &C,
    id: &Uuid,
    provider: &str,
    external_id: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(SET_PROVIDER_FIELDS).await?;
    client
        .execute(&stmt, &[id, &provider, &external_id])
        .await?;
    Ok(())
}

pub async fn set_amounts<C>(
    client: &C,
    id: &Uuid,
    to_amount: Option<Minor>,
    to_currency: Option<String>,
    fees: Minor,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(SET_AMOUNTS).await?;
    client
        .execute(&stmt, &[id, &to_amount, &to_currency, &fees])
        .await?;
    Ok(())
}

pub async fn insert_route<C>(client: &C, route: &NewRoute) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(INSERT_ROUTE).await?;
    Ok(client
        .query_opt(
            &stmt,
            &[
                &route.transaction_id,
                &route.acquirer_id,
                &route.acquirer_slug,
                &route.fee_percent,
                &route.exchange_rate,
                &route.status.as_str(),
                &route.source,
            ],
        )
        .await?)
}

pub async fn route_for_transaction<C>(
    client: &C,
    transaction_id: &Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(ROUTE_FOR_TRANSACTION).await?;
    Ok(client.query_opt(&stmt, &[transaction_id]).await?)
}

pub async fn transition_route<C>(
    client: &C,
    id: &Uuid,
    from: RouteStatus,
    to: RouteStatus,
) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(TRANSITION_ROUTE).await?;
    let n = client
        .execute(&stmt, &[id, &to.as_str(), &from.as_str()])
        .await?;
    Ok(n == 1)
}

/// `(id, slug, name, fee_percent, fee_fixed, active)`.
pub async fn acquirer_by_slug<C>(client: &C, slug: &str) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(ACQUIRER_BY_SLUG).await?;
    Ok(client.query_opt(&stmt, &[&slug]).await?)
}

pub async fn credential_encrypted<C>(
    client: &C,
    acquirer_id: &Uuid,
    name: &str,
) -> Result<Option<String>>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(CREDENTIAL_ENCRYPTED).await?;
    Ok(client
        .query_opt(&stmt, &[acquirer_id, &name])
        .await?
        .map(|row| row.get(0)))
}

pub async fn insert_webhook<C>(
    client: &C,
    provider: &str,
    event_id: Option<&str>,
    event_type: Option<&str>,
    payload: &serde_json::Value,
    signature: Option<&str>,
) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(INSERT_WEBHOOK).await?;
    Ok(client
        .query_opt(
            &stmt,
            &[&provider, &event_id, &event_type, payload, &signature],
        )
        .await?
        .context("webhook insert returned no row")?
        .get(0))
}

pub async fn mark_webhook_processed<C>(
    client: &C,
    id: i64,
    status: &str,
    error: Option<&str>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    let stmt = client.prepare_cached(MARK_WEBHOOK_PROCESSED).await?;
    client.execute(&stmt, &[&id, &status, &error]).await?;
    Ok(())
}

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::db::DbPool;
use crate::payments::model::{
    to_minor, Minor, NewRoute, NewTransaction, Route, Transaction,
};
use crate::payments::status::{RouteStatus, TransactionStatus};

// --- transactions ---

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

pub async fn insert_transaction(pool: &DbPool, tx: &NewTransaction) -> Result<Option<Transaction>> {
    let client = pool.get().await?;
    let stmt = client.prepare_cached(INSERT_TX).await?;
    let row = client
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
        .await?;
    Ok(row.map(row_to_tx))
}

pub async fn transaction_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<Transaction>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_TX} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_tx))
}

pub async fn transaction_by_idempotency_key(
    pool: &DbPool,
    key: &str,
) -> Result<Option<Transaction>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_TX} WHERE idempotency_key = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[&key]).await?;
    Ok(row.map(row_to_tx))
}

pub async fn transactions_for_user(pool: &DbPool, user_id: &Uuid) -> Result<Vec<Transaction>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_TX} WHERE user_id = $1 ORDER BY created_at DESC"))
        .await?;
    let rows = client.query(&stmt, &[user_id]).await?;
    Ok(rows.into_iter().map(row_to_tx).collect())
}

/// Guarded status transition. Returns `Ok(true)` if exactly one row flipped
/// `from` -> `to`, `Ok(false)` if no row matched (`transaction id + expected
/// status`), which means a concurrent update won the race (PLAN #35/#45).
pub async fn transition_status(
    pool: &DbPool,
    id: &Uuid,
    from: TransactionStatus,
    to: TransactionStatus,
) -> Result<bool> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached("UPDATE transactions SET status = $2, updated_at = now() WHERE id = $1 AND status = $3")
        .await?;
    let n = client
        .execute(&stmt, &[id, &to.as_str(), &from.as_str()])
        .await?;
    Ok(n == 1)
}

pub async fn set_provider_fields(
    pool: &DbPool,
    id: &Uuid,
    provider: &str,
    external_id: &str,
) -> Result<()> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            "UPDATE transactions SET provider = $2, external_id = $3, updated_at = now() WHERE id = $1",
        )
        .await?;
    client
        .execute(&stmt, &[id, &provider, &external_id])
        .await?;
    Ok(())
}

/// Update settlement numbers after the route is known: to-amount (post fees,
/// post conversion), to-currency and total fees.
pub async fn set_amounts(
    pool: &DbPool,
    id: &Uuid,
    to_amount: Option<Minor>,
    to_currency: Option<String>,
    fees: Minor,
) -> Result<()> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
UPDATE transactions
SET to_amount = $2, to_currency = $3, fees = $4, updated_at = now()
WHERE id = $1
"#)
        .await?;
    client
        .execute(&stmt, &[id, &to_amount, &to_currency, &fees])
        .await?;
    Ok(())
}

/// Record the executing provider + its id on the transaction.
pub async fn set_provider(
    pool: &DbPool,
    id: &Uuid,
    provider: &str,
    external_id: &str,
) -> Result<()> {
    set_provider_fields(pool, id, provider, external_id).await
}

// --- routes ---

pub async fn insert_route(pool: &DbPool, route: &NewRoute) -> Result<Option<Route>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
INSERT INTO routes (transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source)
VALUES ($1, $2, $3, $4, $5, $6, $7)
RETURNING id, transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source, created_at
"#)
        .await?;
    let row = client
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
        .await?;
    Ok(row.map(row_to_route))
}

pub async fn route_for_transaction(pool: &DbPool, transaction_id: &Uuid) -> Result<Option<Route>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
SELECT id, transaction_id, acquirer_id, acquirer_slug, fee_percent, exchange_rate, status, source, created_at
FROM routes WHERE transaction_id = $1
"#)
        .await?;
    let row = client.query_opt(&stmt, &[transaction_id]).await?;
    Ok(row.map(row_to_route))
}

pub async fn transition_route(
    pool: &DbPool,
    id: &Uuid,
    from: RouteStatus,
    to: RouteStatus,
) -> Result<bool> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached("UPDATE routes SET status = $2, updated_at = now() WHERE id = $1 AND status = $3")
        .await?;
    let n = client
        .execute(&stmt, &[id, &to.as_str(), &from.as_str()])
        .await?;
    Ok(n == 1)
}

// --- acquirers ---

#[derive(Debug, Clone)]
pub struct AcquirerRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub fee_percent: f64,
    pub fee_fixed: Minor,
    pub active: bool,
}

/// Upsert an acquirer by slug (used to seed/refresh the acquirer table from the
/// curated seed list, PLAN #28). Returns the acquirer id (existing or new).
pub async fn upsert_acquirer(pool: &DbPool, seed: &crate::acquirer::AcquirerSeed) -> Result<Uuid> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
INSERT INTO acquirers (slug, name, geo, currencies, fee_percent, fee_fixed, amount_currency, status, active)
VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', TRUE)
ON CONFLICT (slug) DO UPDATE SET
    name = EXCLUDED.name,
    geo = EXCLUDED.geo,
    currencies = EXCLUDED.currencies,
    fee_percent = EXCLUDED.fee_percent,
    fee_fixed = EXCLUDED.fee_fixed,
    amount_currency = EXCLUDED.amount_currency,
    updated_at = now()
RETURNING id
"#)
        .await?;
    let f = crate::routing::profile::AcquirerProfile::from(seed);
    let row = client
        .query_opt(
            &stmt,
            &[
                &seed.slug,
                &seed.name,
                &seed.geo,
                &seed.currencies,
                &f.fee_percent,
                &to_minor(f.fee_flat),
                &f.amount_currency,
            ],
        )
        .await?
        .context("acquirer upsert returned no row")?;
    Ok(row.get(0))
}

pub async fn acquirer_by_slug(pool: &DbPool, slug: &str) -> Result<Option<AcquirerRow>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            "SELECT id, slug, name, fee_percent, fee_fixed, active FROM acquirers WHERE slug = $1",
        )
        .await?;
    let row = client.query_opt(&stmt, &[&slug]).await?;
    Ok(row.map(|row| AcquirerRow {
        id: row.get(0),
        slug: row.get(1),
        name: row.get(2),
        fee_percent: row.get(3),
        fee_fixed: row.get(4),
        active: row.get(5),
    }))
}

/// Upsert a fictional solver's passport row (PLAN #28) from the fake catalog.
/// Fee = worst-case pair commission; currencies = union of pair legs.
pub async fn upsert_fake_acquirer(
    pool: &DbPool,
    acq: &crate::fake_acquirers::FakeAcquirer,
) -> Result<Uuid> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
INSERT INTO acquirers (slug, name, geo, currencies, fee_percent, fee_fixed, amount_currency, status, active)
VALUES ($1, $2, $3, $4, $5, 0, $6, 'active', TRUE)
ON CONFLICT (slug) DO UPDATE SET
    name = EXCLUDED.name,
    geo = EXCLUDED.geo,
    currencies = EXCLUDED.currencies,
    fee_percent = EXCLUDED.fee_percent,
    amount_currency = EXCLUDED.amount_currency,
    updated_at = now()
RETURNING id
"#)
        .await?;
    let currencies = acq.currencies_token();
    let fee = acq.worst_commission();
    let amount_currency = acq
        .limits
        .split("max ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().nth(1))
        .unwrap_or("USD")
        .to_uppercase();
    let row = client
        .query_opt(
            &stmt,
            &[
                &acq.slug,
                &acq.name,
                &acq.geo,
                &currencies,
                &fee,
                &amount_currency,
            ],
        )
        .await?
        .context("fake acquirer upsert returned no row")?;
    Ok(row.get(0))
}

// --- credentials (PLAN #29, secrets kept out of regular fetches) ---

pub async fn set_credential(
    pool: &DbPool,
    acquirer_id: &Uuid,
    name: &str,
    value_encrypted: &str,
) -> Result<()> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
INSERT INTO credentials (acquirer_id, name, value_encrypted)
VALUES ($1, $2, $3)
ON CONFLICT (acquirer_id, name) DO UPDATE SET value_encrypted = EXCLUDED.value_encrypted, updated_at = now()
"#)
        .await?;
    client
        .execute(&stmt, &[acquirer_id, &name, &value_encrypted])
        .await?;
    Ok(())
}

pub async fn get_credential_encrypted(
    pool: &DbPool,
    acquirer_id: &Uuid,
    name: &str,
) -> Result<Option<String>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached("SELECT value_encrypted FROM credentials WHERE acquirer_id = $1 AND name = $2")
        .await?;
    let row = client.query_opt(&stmt, &[acquirer_id, &name]).await?;
    Ok(row.map(|row| row.get(0)))
}

// --- webhooks (PLAN #31) ---

pub async fn insert_webhook(
    pool: &DbPool,
    provider: &str,
    event_id: Option<&str>,
    event_type: Option<&str>,
    payload: &serde_json::Value,
    signature: Option<&str>,
) -> Result<i64> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(r#"
INSERT INTO provider_webhooks (provider, event_id, event_type, payload, signature)
VALUES ($1, $2, $3, $4, $5)
RETURNING id
"#)
        .await?;
    let row = client
        .query_opt(&stmt, &[&provider, &event_id, &event_type, payload, &signature])
        .await?
        .context("webhook insert returned no row")?;
    Ok(row.get(0))
}

pub async fn mark_webhook_processed(
    pool: &DbPool,
    id: i64,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(
            "UPDATE provider_webhooks SET status = $2, error = $3, processed_at = now() WHERE id = $1",
        )
        .await?;
    client.execute(&stmt, &[&id, &status, &error]).await?;
    Ok(())
}

// --- row mappers ---

fn row_to_tx(row: tokio_postgres::Row) -> Transaction {
    Transaction {
        id: row.get(0),
        user_id: row.get(1),
        status: TransactionStatus::parse(&row.get::<_, String>(2))
            .unwrap_or(TransactionStatus::Pending),
        from_amount: row.get::<_, Minor>(3),
        from_currency: row.get(4),
        to_amount: row.try_get::<_, Option<Minor>>(5).ok().flatten(),
        to_currency: row.try_get::<_, Option<String>>(6).ok().flatten(),
        from_account: row.get(7),
        to_account: row.get(8),
        method: row.try_get::<_, Option<String>>(9).ok().flatten(),
        fees: row.get::<_, Minor>(10),
        provider: row.try_get::<_, Option<String>>(11).ok().flatten(),
        external_id: row.try_get::<_, Option<String>>(12).ok().flatten(),
        idempotency_key: row.try_get::<_, Option<String>>(13).ok().flatten(),
        created_at: row.get::<_, DateTime<Utc>>(14),
        updated_at: row.get::<_, DateTime<Utc>>(15),
    }
}

fn row_to_route(row: tokio_postgres::Row) -> Route {
    Route {
        id: row.get(0),
        transaction_id: row.get(1),
        acquirer_id: row.try_get::<_, Option<Uuid>>(2).ok().flatten(),
        acquirer_slug: row.get(3),
        fee_percent: row.get(4),
        exchange_rate: row.try_get::<_, Option<f64>>(5).ok().flatten(),
        status: RouteStatus::parse(&row.get::<_, String>(6)).unwrap_or(RouteStatus::Pending),
        source: row.get(7),
        created_at: row.get::<_, DateTime<Utc>>(8),
    }
}
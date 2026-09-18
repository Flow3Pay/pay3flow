pub mod seed;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;

/// One bank exchange pair (`PLAN 2△ / 46a`): "the card of `from_bank` with
/// scheme `from_scheme` pays the card of `to_bank` with scheme `to_scheme`".
/// The catalog lives here on the backend, not hardcoded in the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangePair {
    pub id: Uuid,
    pub from_scheme: String,
    pub from_bank: String,
    pub from_bank_icon_url: String,
    pub to_scheme: String,
    pub to_bank: String,
    pub to_bank_icon_url: String,
    pub country: String,
    /// Comma-separated `from_currency,to_currency` (e.g. `RUB,EUR`).
    pub currencies: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_limit_minor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_limit_currency: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl ExchangePair {
    pub fn currencies_list(&self) -> Vec<&str> {
        self.currencies
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Query filters for `GET /api/exchange-pairs` (`PLAN 46c`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PairFilters {
    #[serde(default)]
    pub country: Option<String>,
    /// Matches either leg of the pair's `currencies`.
    #[serde(default)]
    pub currency: Option<String>,
    /// Matches either `from_scheme` or `to_scheme`.
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub from_scheme: Option<String>,
    #[serde(default)]
    pub to_scheme: Option<String>,
}

impl PairFilters {
    /// Cache key: the filters are the cache identity. The response for a given
    /// filter combination is cached for the service TTL (default 5 min).
    fn cache_key(&self) -> String {
        format!(
            "c={}|cur={}|s={}|fs={}|ts={}",
            self.country.as_deref().unwrap_or(""),
            self.currency.as_deref().unwrap_or(""),
            self.scheme.as_deref().unwrap_or(""),
            self.from_scheme.as_deref().unwrap_or(""),
            self.to_scheme.as_deref().unwrap_or(""),
        )
    }
}

/// Create payload for `/api/admin/exchange-pairs` (`PLAN 46e`).
#[derive(Debug, Clone, Deserialize)]
pub struct NewPair {
    pub from_scheme: String,
    pub from_bank: String,
    #[serde(default)]
    pub from_bank_icon_url: Option<String>,
    pub to_scheme: String,
    pub to_bank: String,
    #[serde(default)]
    pub to_bank_icon_url: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub currencies: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub daily_limit_minor: Option<i64>,
    #[serde(default)]
    pub daily_limit_currency: Option<String>,
}

/// Patch payload for `POST /api/admin/exchange-pairs/:id` — every field is
/// optional; only the provided fields are written.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PairPatch {
    #[serde(default)]
    pub from_scheme: Option<String>,
    #[serde(default)]
    pub from_bank: Option<String>,
    #[serde(default)]
    pub from_bank_icon_url: Option<String>,
    #[serde(default)]
    pub to_scheme: Option<String>,
    #[serde(default)]
    pub to_bank: Option<String>,
    #[serde(default)]
    pub to_bank_icon_url: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub currencies: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub daily_limit_minor: Option<i64>,
    #[serde(default)]
    pub daily_limit_currency: Option<String>,
}

const SELECT_COLS: &str = "
id, from_scheme, from_bank, from_bank_icon_url, to_scheme, to_bank,
to_bank_icon_url, country, currencies, status, daily_limit_minor,
daily_limit_currency, updated_at
";

const UPSERT_SQL: &str = r#"
INSERT INTO exchange_pairs
    (from_scheme, from_bank, from_bank_icon_url, to_scheme, to_bank, to_bank_icon_url,
     country, currencies, status, daily_limit_minor, daily_limit_currency)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (from_scheme, from_bank, to_scheme, to_bank) DO UPDATE SET
    from_bank_icon_url = EXCLUDED.from_bank_icon_url,
    to_bank_icon_url = EXCLUDED.to_bank_icon_url,
    country = EXCLUDED.country,
    currencies = EXCLUDED.currencies,
    status = EXCLUDED.status,
    daily_limit_minor = EXCLUDED.daily_limit_minor,
    daily_limit_currency = EXCLUDED.daily_limit_currency,
    updated_at = now()
RETURNING
"#;

/// Clean an optional string: trimmed, and `None` when blank.
fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// --- repository ---

fn row_to_pair(row: &tokio_postgres::Row) -> ExchangePair {
    ExchangePair {
        id: row.get(0),
        from_scheme: row.get(1),
        from_bank: row.get(2),
        from_bank_icon_url: row.get(3),
        to_scheme: row.get(4),
        to_bank: row.get(5),
        to_bank_icon_url: row.get(6),
        country: row.get(7),
        currencies: row.get(8),
        status: row.get(9),
        daily_limit_minor: row.try_get::<_, Option<i64>>(10).ok().flatten(),
        daily_limit_currency: row.try_get::<_, Option<String>>(11).ok().flatten(),
        updated_at: row.get(12),
    }
}

/// Enabled pairs (the public catalog), restricted by the filters.
async fn repo_list(pool: &DbPool, filters: &PairFilters) -> Result<Vec<ExchangePair>> {
    let client = pool.get().await?;
    let mut sql = format!("SELECT {SELECT_COLS} FROM exchange_pairs WHERE status = 'enabled'");
    let mut values: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

    if let Some(value) = clean(filters.country.clone()) {
        values.push(Box::new(value));
        sql.push_str(&format!(" AND country = ${}", values.len()));
    }
    if let Some(value) = clean(filters.from_scheme.clone()) {
        values.push(Box::new(value));
        sql.push_str(&format!(" AND from_scheme = ${}", values.len()));
    }
    if let Some(value) = clean(filters.to_scheme.clone()) {
        values.push(Box::new(value));
        sql.push_str(&format!(" AND to_scheme = ${}", values.len()));
    }
    if let Some(value) = clean(filters.scheme.clone()) {
        values.push(Box::new(value));
        sql.push_str(&format!(
            " AND (from_scheme = ${} OR to_scheme = ${})",
            values.len(),
            values.len()
        ));
    }
    if let Some(value) = clean(filters.currency.clone()) {
        values.push(Box::new(value));
        sql.push_str(&format!(
            " AND (${} = ANY(string_to_array(currencies, ',')))",
            values.len()
        ));
    }
    sql.push_str(" ORDER BY from_bank, to_bank");

let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = values
        .iter()
        .map(|v| v.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    let stmt = client.prepare_cached(&sql).await?;
    let rows = client.query(&stmt, &params).await?;
    Ok(rows.iter().map(row_to_pair).collect())
}

pub async fn repo_upsert(pool: &DbPool, body: &NewPair) -> Result<ExchangePair> {
    let client = pool.get().await?;
    let stmt = client.prepare_cached(&format!("{UPSERT_SQL} {SELECT_COLS}")).await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &body.from_scheme,
                &body.from_bank,
                &clean(body.from_bank_icon_url.clone()).unwrap_or_default(),
                &body.to_scheme,
                &body.to_bank,
                &clean(body.to_bank_icon_url.clone()).unwrap_or_default(),
                &clean(body.country.clone()).unwrap_or_default(),
                &clean(body.currencies.clone()).unwrap_or_else(|| "USD,USD".into()),
                &body.status.clone().unwrap_or_else(|| "enabled".into()),
                &body.daily_limit_minor,
                &body.daily_limit_currency,
            ],
        )
        .await?
        .context("exchange pair upsert returned no row")?;
    Ok(row_to_pair(&row))
}

pub async fn repo_patch(pool: &DbPool, id: Uuid, patch: &PairPatch) -> Result<Option<ExchangePair>> {
    let client = pool.get().await?;
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

    if let Some(value) = clean(patch.from_scheme.clone()) {
        values.push(Box::new(value));
        sets.push(format!("from_scheme = ${}", values.len()));
    }
    if let Some(value) = clean(patch.from_bank.clone()) {
        values.push(Box::new(value));
        sets.push(format!("from_bank = ${}", values.len()));
    }
    if let Some(value) = clean(patch.from_bank_icon_url.clone()) {
        values.push(Box::new(value));
        sets.push(format!("from_bank_icon_url = ${}", values.len()));
    }
    if let Some(value) = clean(patch.to_scheme.clone()) {
        values.push(Box::new(value));
        sets.push(format!("to_scheme = ${}", values.len()));
    }
    if let Some(value) = clean(patch.to_bank.clone()) {
        values.push(Box::new(value));
        sets.push(format!("to_bank = ${}", values.len()));
    }
    if let Some(value) = clean(patch.to_bank_icon_url.clone()) {
        values.push(Box::new(value));
        sets.push(format!("to_bank_icon_url = ${}", values.len()));
    }
    if let Some(value) = clean(patch.country.clone()) {
        values.push(Box::new(value));
        sets.push(format!("country = ${}", values.len()));
    }
    if let Some(value) = clean(patch.currencies.clone()) {
        values.push(Box::new(value));
        sets.push(format!("currencies = ${}", values.len()));
    }
    if let Some(value) = clean(patch.status.clone()) {
        values.push(Box::new(value));
        sets.push(format!("status = ${}", values.len()));
    }
    if let Some(value) = clean(patch.daily_limit_currency.clone()) {
        values.push(Box::new(value));
        sets.push(format!("daily_limit_currency = ${}", values.len()));
    }
    if let Some(minor) = patch.daily_limit_minor {
        values.push(Box::new(minor));
        sets.push(format!("daily_limit_minor = ${}", values.len()));
    }

    if sets.is_empty() {
        return repo_by_id(pool, id).await;
    }
    let sql = format!(
        "UPDATE exchange_pairs SET {}, updated_at = now() WHERE id = ${} RETURNING {SELECT_COLS}",
        sets.join(", "),
        values.len() + 1
    );
let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = values
        .iter()
        .map(|v| v.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    params.push(&id);
    let row = client.query_opt(&sql, &params).await?;
    Ok(row.map(|r| row_to_pair(&r)))
}

pub async fn repo_by_id(pool: &DbPool, id: Uuid) -> Result<Option<ExchangePair>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("SELECT {SELECT_COLS} FROM exchange_pairs WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[&id]).await?;
    Ok(row.map(|r| row_to_pair(&r)))
}

// --- service (with in-memory TTL cache, PLAN 46c) ---

struct CacheEntry {
    pairs: Vec<ExchangePair>,
    expires_at: Instant,
}

pub struct ExchangePairsService {
    pool: DbPool,
    cache_ttl: Duration,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl Clone for ExchangePairsService {
    fn clone(&self) -> Self {
        // Cloned instances share the pool but keep their own (empty) cache.
        Self {
            pool: self.pool.clone(),
            cache_ttl: self.cache_ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl ExchangePairsService {
    pub fn new(pool: DbPool, cache_ttl: Duration) -> Self {
        Self {
            pool,
            cache_ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Enabled pairs per the filters, served from the in-memory TTL cache.
    pub async fn list(&self, filters: &PairFilters) -> Result<Vec<ExchangePair>> {
        let key = filters.cache_key();
        if let Ok(guard) = self.cache.lock() {
            if let Some(entry) = guard.get(&key) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.pairs.clone());
                }
            }
        }
        let pairs = repo_list(&self.pool, filters).await?;
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(
                key,
                CacheEntry {
                    pairs: pairs.clone(),
                    expires_at: Instant::now() + self.cache_ttl,
                },
            );
        }
        Ok(pairs)
    }

    pub async fn create(&self, body: &NewPair) -> Result<ExchangePair> {
        let pair = repo_upsert(&self.pool, body).await?;
        self.invalidate();
        Ok(pair)
    }

    pub async fn update(&self, id: Uuid, patch: &PairPatch) -> Result<Option<ExchangePair>> {
        let pair = repo_patch(&self.pool, id, patch).await?;
        self.invalidate();
        Ok(pair)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<ExchangePair>> {
        repo_by_id(&self.pool, id).await
    }

    fn invalidate(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.clear();
        }
    }
}

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;

/// One bank in the directory. The payment form picks a sending bank and a
/// receiving bank from these; the user composes the exchange themselves, so the
/// backend owns the canonical worldwide list instead of the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bank {
    pub id: Uuid,
    pub name: String,
    /// `sender`, `receiver` or `both`.
    pub role: String,
    pub country: String,
    pub currency: String,
    pub domain: String,
    pub icon_url: String,
    /// Card schemes this bank issues, e.g. `["Visa", "MasterCard"]`.
    pub schemes: Vec<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

/// One page of the bank directory (`GET /api/banks`).
#[derive(Debug, Serialize)]
pub struct BankPage {
    pub items: Vec<Bank>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

/// Query filters for `GET /api/banks`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BankFilters {
    /// `sender` / `receiver`; `both` banks always qualify.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub scheme: Option<String>,
    /// Free-text bank-name search.
    #[serde(default)]
    pub q: Option<String>,
}

impl BankFilters {
    fn cache_key(&self) -> String {
        format!(
            "r={}|c={}|cur={}|s={}|q={}",
            self.role.as_deref().unwrap_or(""),
            self.country.as_deref().unwrap_or(""),
            self.currency.as_deref().unwrap_or(""),
            self.scheme.as_deref().unwrap_or(""),
            self.q.as_deref().unwrap_or(""),
        )
    }
}

/// Create/upsert payload (used by the seed and the admin endpoint).
#[derive(Debug, Clone, Deserialize)]
pub struct NewBank {
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub schemes: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

const BANK_COLS: &str =
    "id, name, role, country, currency, domain, icon_url, schemes, status, updated_at";

const BANK_UPSERT: &str = r#"
INSERT INTO banks (name, role, country, currency, domain, icon_url, schemes, status)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (name) DO UPDATE SET
    role = EXCLUDED.role,
    country = EXCLUDED.country,
    currency = EXCLUDED.currency,
    domain = EXCLUDED.domain,
    icon_url = EXCLUDED.icon_url,
    schemes = EXCLUDED.schemes,
    status = EXCLUDED.status,
    updated_at = now()
RETURNING
"#;

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

// --- repository ---

fn row_to_bank(row: &tokio_postgres::Row) -> Bank {
    Bank {
        id: row.get(0),
        name: row.get(1),
        role: row.get(2),
        country: row.get(3),
        currency: row.get(4),
        domain: row.get(5),
        icon_url: row.get(6),
        schemes: split_csv(&row.get::<_, String>(7)),
        status: row.get(8),
        updated_at: row.get(9),
    }
}

/// Enabled banks, restricted by the filters.
async fn repo_list(
    pool: &DbPool,
    filters: &BankFilters,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Bank>> {
    let client = pool.get().await?;
    let (where_clause, mut values) = build_where(filters);
    let mut sql = format!("SELECT {BANK_COLS} FROM banks WHERE status = 'enabled'{where_clause}");
    sql.push_str(" ORDER BY name");
    if let Some(limit) = limit {
        values.push(Box::new(limit as i64));
        sql.push_str(&format!(" LIMIT ${}", values.len()));
    }
    if let Some(offset) = offset {
        values.push(Box::new(offset as i64));
        sql.push_str(&format!(" OFFSET ${}", values.len()));
    }

    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = values
        .iter()
        .map(|v| v.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    let stmt = client.prepare_cached(&sql).await?;
    let rows = client.query(&stmt, &params).await?;
    Ok(rows.iter().map(row_to_bank).collect())
}

/// Total enabled banks matching the filters (same WHERE as `repo_list`).
pub async fn repo_count(pool: &DbPool, filters: &BankFilters) -> Result<i64> {
    let client = pool.get().await?;
    let (where_clause, values) = build_where(filters);
    let sql = format!("SELECT COUNT(*) FROM banks WHERE status = 'enabled'{where_clause}");

    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = values
        .iter()
        .map(|v| v.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    let stmt = client.prepare_cached(&sql).await?;
    let row = client.query_one(&stmt, &params).await?;
    Ok(row.get(0))
}

/// Shared `AND ...` filter fragment and its bound values.
fn build_where(
    filters: &BankFilters,
) -> (
    String,
    Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) {
    let mut sql = String::new();
    let mut values: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();

    if let Some(role) = clean(filters.role.clone()) {
        if role == "sender" || role == "receiver" {
            values.push(Box::new(role));
            values.push(Box::new("both".to_string()));
            sql.push_str(&format!(
                " AND role IN (${}, ${})",
                values.len() - 1,
                values.len()
            ));
        }
    }
    if let Some(country) = clean(filters.country.clone()) {
        values.push(Box::new(country));
        sql.push_str(&format!(" AND country = ${}", values.len()));
    }
    if let Some(currency) = clean(filters.currency.clone()) {
        values.push(Box::new(currency));
        sql.push_str(&format!(" AND currency = ${}", values.len()));
    }
    if let Some(scheme) = clean(filters.scheme.clone()) {
        values.push(Box::new(scheme));
        sql.push_str(&format!(
            " AND ${} = ANY(string_to_array(schemes, ','))",
            values.len()
        ));
    }
    if let Some(q) = clean(filters.q.clone()) {
        values.push(Box::new(format!("%{q}%")));
        sql.push_str(&format!(" AND name ILIKE ${}", values.len()));
    }
    (sql, values)
}

pub async fn repo_upsert(pool: &DbPool, body: &NewBank) -> Result<Bank> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{BANK_UPSERT} {BANK_COLS}"))
        .await?;
    let row = client
        .query_opt(
            &stmt,
            &[
                &body.name,
                &clean(body.role.clone()).unwrap_or_else(|| "both".into()),
                &clean(body.country.clone()).unwrap_or_default(),
                &clean(body.currency.clone()).unwrap_or_default(),
                &clean(body.domain.clone()).unwrap_or_default(),
                &clean(body.icon_url.clone()).unwrap_or_default(),
                &clean(body.schemes.clone()).unwrap_or_default(),
                &body.status.clone().unwrap_or_else(|| "enabled".into()),
            ],
        )
        .await?
        .context("bank upsert returned no row")?;
    Ok(row_to_bank(&row))
}

/// Set a bank's `status` ("enabled"/"disabled"); returns `None` when no bank
/// has that name.
pub async fn repo_set_status(pool: &DbPool, name: &str, status: &str) -> Result<Option<Bank>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "UPDATE banks SET status = $2, updated_at = now() WHERE name = $1 RETURNING {BANK_COLS}"
        ))
        .await?;
    let row = client.query_opt(&stmt, &[&name, &status]).await?;
    Ok(row.map(|r| row_to_bank(&r)))
}

// --- service (in-memory TTL cache) ---

struct CacheEntry {
    banks: Vec<Bank>,
    expires_at: Instant,
}

pub struct BanksService {
    pool: DbPool,
    cache_ttl: Duration,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl Clone for BanksService {
    fn clone(&self) -> Self {
        // Cloned instances share the pool but keep their own (empty) cache.
        Self {
            pool: self.pool.clone(),
            cache_ttl: self.cache_ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl BanksService {
    pub fn new(pool: DbPool, cache_ttl: Duration) -> Self {
        Self {
            pool,
            cache_ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Page of enabled banks per the filters. The cache stores the full
    /// filtered result keyed by the filters only, so pagination (including
    /// `total`) is served from it and any page hits are consistent.
    pub async fn list(&self, filters: &BankFilters, limit: u32, offset: u32) -> Result<BankPage> {
        let key = filters.cache_key();
        let banks = self.cached(&key, filters).await?;
        let total = banks.len() as i64;
        let items = banks
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(BankPage {
            items,
            total,
            limit,
            offset,
        })
    }

    async fn cached(&self, key: &str, filters: &BankFilters) -> Result<Vec<Bank>> {
        if let Ok(guard) = self.cache.lock() {
            if let Some(entry) = guard.get(key) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.banks.clone());
                }
            }
        }
        let banks = repo_list(&self.pool, filters, None, None).await?;
        if let Ok(mut guard) = self.cache.lock() {
            guard.insert(
                key.to_string(),
                CacheEntry {
                    banks: banks.clone(),
                    expires_at: Instant::now() + self.cache_ttl,
                },
            );
        }
        Ok(banks)
    }

    pub async fn create(&self, body: &NewBank) -> Result<Bank> {
        let bank = repo_upsert(&self.pool, body).await?;
        self.invalidate();
        Ok(bank)
    }

    /// Set a bank's status (`enabled`/`disabled`); `None` when the name is
    /// unknown.
    pub async fn set_status(&self, name: &str, status: &str) -> Result<Option<Bank>> {
        let bank = repo_set_status(&self.pool, name, status).await?;
        if bank.is_some() {
            self.invalidate();
        }
        Ok(bank)
    }

    fn invalidate(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            guard.clear();
        }
    }
}

//! Project-scoped merchant API keys (Phase 1.1 / portal §5.3, §8.1).
//!
//! Secrets are shown once as `sk_{env}_{hex}` and stored as an Argon2id hash
//! plus a unique prefix used for lookup. `pk_` is the non-secret identifier
//! derived from the same material; presenting `pk_` alone never authenticates.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::{DateTime, Timelike, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use uuid::Uuid;

use crate::db::DbPool;

use super::Error;

/// Product scopes a key may carry. The catalogue matches portal §5.3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    RatesRead,
    QuotesWrite,
    OrdersRead,
    OrdersWrite,
    RefundsWrite,
    OtcWrite,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RatesRead => "rates:read",
            Self::QuotesWrite => "quotes:write",
            Self::OrdersRead => "orders:read",
            Self::OrdersWrite => "orders:write",
            Self::RefundsWrite => "refunds:write",
            Self::OtcWrite => "otc:write",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "rates:read" => Self::RatesRead,
            "quotes:write" => Self::QuotesWrite,
            "orders:read" => Self::OrdersRead,
            "orders:write" => Self::OrdersWrite,
            "refunds:write" => Self::RefundsWrite,
            "otc:write" => Self::OtcWrite,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEnv {
    Test,
    Live,
}

impl KeyEnv {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Live => "live",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "test" => Some(Self::Test),
            "live" => Some(Self::Live),
            _ => None,
        }
    }
}

/// Fresh key material ready to insert. `secret` is shown once and never stored.
#[derive(Debug, Clone)]
pub struct IssuedKey {
    pub secret: String,
    pub prefix: String,
    pub public_id: String,
    pub secret_hash: String,
    pub env: KeyEnv,
}

/// Authenticated key row used by the middleware.
#[derive(Debug, Clone)]
pub struct AuthenticatedKey {
    pub id: Uuid,
    pub project_id: Uuid,
    pub merchant_id: Uuid,
    pub env: KeyEnv,
    pub scopes: Vec<Scope>,
    pub daily_request_limit: i32,
    pub prefix: String,
}

/// Mint `sk_` / `pk_` material and an Argon2id hash. Persistence is separate so
/// the portal create form can preview validation before the insert.
pub fn issue(env: KeyEnv) -> Result<IssuedKey, Error> {
    let mut raw = [0u8; 24];
    OsRng.fill_bytes(&mut raw);
    let body = encode_hex(&raw);
    let secret = format!("sk_{}_{body}", env.as_str());
    let prefix = secret.chars().take(16).collect::<String>();
    let public_id = format!("pk_{}_{}", env.as_str(), &body[..16]);
    let secret_hash = hash_secret(&secret)?;
    Ok(IssuedKey {
        secret,
        prefix,
        public_id,
        secret_hash,
        env,
    })
}

fn hash_secret(secret: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = argon2::Argon2::default();
    Ok(argon
        .hash_password(secret.as_bytes(), &salt)
        .map_err(|error| Error::Internal(anyhow::anyhow!("hash api key: {error}")))?
        .to_string())
}

fn verify_secret(secret: &str, encoded: &str) -> Result<bool, Error> {
    let parsed = PasswordHash::new(encoded)
        .map_err(|error| Error::Internal(anyhow::anyhow!("parse api key hash: {error}")))?;
    Ok(argon2::Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok())
}

/// Look up an active key by the Bearer `sk_…` secret.
pub async fn authenticate(pool: &DbPool, bearer: &str) -> Result<AuthenticatedKey, Error> {
    let secret = bearer.trim();
    if !secret.starts_with("sk_") {
        return Err(Error::Unauthorized("API key required".into()));
    }
    let prefix: String = secret.chars().take(16).collect();
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT k.id, k.project_id, p.merchant_id, k.env, k.scopes, \
                    k.daily_request_limit, k.prefix, k.secret_hash, k.status \
             FROM b2b_api_keys k \
             JOIN b2b_projects p ON p.id = k.project_id \
             WHERE k.prefix = $1",
            &[&prefix],
        )
        .await?;
    let Some(row) = row else {
        return Err(Error::Unauthorized("unknown or revoked API key".into()));
    };
    let status: String = row.get("status");
    if status != "active" {
        return Err(Error::Unauthorized("unknown or revoked API key".into()));
    }
    let secret_hash: String = row.get("secret_hash");
    if !verify_secret(secret, &secret_hash)? {
        return Err(Error::Unauthorized("unknown or revoked API key".into()));
    }
    let env = KeyEnv::parse(row.get::<_, String>("env").as_str())
        .ok_or_else(|| Error::Internal(anyhow::anyhow!("invalid key env in database")))?;
    let scopes = parse_scopes(row.get::<_, Vec<String>>("scopes"))?;
    Ok(AuthenticatedKey {
        id: row.get("id"),
        project_id: row.get("project_id"),
        merchant_id: row.get("merchant_id"),
        env,
        scopes,
        daily_request_limit: row.get("daily_request_limit"),
        prefix: row.get("prefix"),
    })
}

fn parse_scopes(raw: Vec<String>) -> Result<Vec<Scope>, Error> {
    raw.iter()
        .map(|value| {
            Scope::parse(value).ok_or_else(|| {
                Error::Internal(anyhow::anyhow!("unknown scope in database: {value}"))
            })
        })
        .collect()
}

impl AuthenticatedKey {
    pub fn allows(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }
}

/// UTC calendar day window used by the Phase 2 daily limit (§5.3.2 / §12).
pub fn utc_day_bounds(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid")
        .and_utc();
    let end = start + chrono::Duration::days(1);
    (start, end)
}

/// Count requests already charged to this key for the current UTC day.
pub async fn requests_today(pool: &DbPool, key_id: Uuid, now: DateTime<Utc>) -> Result<i64, Error> {
    let (start, end) = utc_day_bounds(now);
    let client = pool.get().await?;
    let count: i64 = client
        .query_one(
            "SELECT COALESCE(SUM(requests), 0)::bigint \
             FROM b2b_api_key_usage \
             WHERE key_id = $1 AND bucket_start >= $2 AND bucket_start < $3",
            &[&key_id, &start, &end],
        )
        .await?
        .get(0);
    Ok(count)
}

/// `true` when the key is over its daily limit (`0` means unlimited).
pub async fn over_daily_limit(
    pool: &DbPool,
    key: &AuthenticatedKey,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    if key.daily_request_limit == 0 {
        return Ok(false);
    }
    let used = requests_today(pool, key.id, now).await?;
    Ok(used >= i64::from(key.daily_request_limit))
}

fn hour_bucket(now: DateTime<Utc>) -> DateTime<Utc> {
    now.date_naive()
        .and_hms_opt(now.hour(), 0, 0)
        .expect("hour truncation is valid")
        .and_utc()
}

/// Atomically upsert one hourly usage row. Latency percentiles are left null
/// until the rollup job fills them; the sum is enough for the live limit meter.
pub async fn record_usage(
    pool: &DbPool,
    key_id: Uuid,
    status: u16,
    latency_ms: u64,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let bucket = hour_bucket(now);
    let is_4xx = i64::from((400..500).contains(&status));
    let is_5xx = i64::from((500..600).contains(&status));
    let latency = i64::try_from(latency_ms).unwrap_or(i64::MAX);
    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO b2b_api_key_usage \
                (key_id, bucket_start, requests, errors_4xx, errors_5xx, latency_sum_ms) \
             VALUES ($1, $2, 1, $3, $4, $5) \
             ON CONFLICT (key_id, bucket_start) DO UPDATE SET \
                requests = b2b_api_key_usage.requests + 1, \
                errors_4xx = b2b_api_key_usage.errors_4xx + EXCLUDED.errors_4xx, \
                errors_5xx = b2b_api_key_usage.errors_5xx + EXCLUDED.errors_5xx, \
                latency_sum_ms = b2b_api_key_usage.latency_sum_ms + EXCLUDED.latency_sum_ms",
            &[&key_id, &bucket, &is_4xx, &is_5xx, &latency],
        )
        .await?;
    client
        .execute(
            "UPDATE b2b_api_keys SET last_used_at = $2 WHERE id = $1",
            &[&key_id, &now],
        )
        .await?;
    Ok(())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// Strip `Bearer ` and return the raw token, if present.
pub fn bearer_token<'a>(authorization: Option<&'a str>) -> Option<&'a str> {
    authorization
        .map(str::trim)
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_roundtrips_argon2_and_keeps_prefix_stable() {
        let issued = issue(KeyEnv::Test).expect("issue");
        assert!(issued.secret.starts_with("sk_test_"));
        assert!(issued.public_id.starts_with("pk_test_"));
        assert_eq!(
            issued.prefix,
            issued.secret.chars().take(16).collect::<String>()
        );
        assert!(verify_secret(&issued.secret, &issued.secret_hash).unwrap());
        assert!(!verify_secret("sk_test_tampered", &issued.secret_hash).unwrap());
    }

    #[test]
    fn scope_parse_covers_catalogue() {
        for scope in [
            Scope::RatesRead,
            Scope::QuotesWrite,
            Scope::OrdersRead,
            Scope::OrdersWrite,
            Scope::RefundsWrite,
            Scope::OtcWrite,
        ] {
            assert_eq!(Scope::parse(scope.as_str()), Some(scope));
        }
        assert_eq!(Scope::parse("nope"), None);
    }

    #[test]
    fn utc_day_bounds_are_midnight_to_midnight() {
        let now = DateTime::parse_from_rfc3339("2026-11-11T15:22:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (start, end) = utc_day_bounds(now);
        assert_eq!(start.to_rfc3339(), "2026-11-11T00:00:00+00:00");
        assert_eq!(end.to_rfc3339(), "2026-11-12T00:00:00+00:00");
    }

    #[test]
    fn bearer_token_strips_prefix() {
        assert_eq!(
            bearer_token(Some("Bearer sk_test_abc")),
            Some("sk_test_abc")
        );
        assert_eq!(bearer_token(Some("sk_test_abc")), None);
        assert_eq!(bearer_token(None), None);
    }
}

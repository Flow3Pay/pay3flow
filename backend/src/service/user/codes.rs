//! One-time email codes (portal §4.4).
//!
//! A code is six digits, lives for ten minutes, is stored as an Argon2id hash
//! and is invalidated after five wrong attempts or a resend. Send history is
//! kept in `auth_code_sends` so the per-email and per-IP limits survive a
//! restart and hold across several backend instances.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::{DateTime, Duration, Utc};
use rand::rngs::OsRng;
use rand::Rng;
use uuid::Uuid;

use crate::db::DbPool;

use super::UserError;

pub const CODE_TTL_MINUTES: i64 = 10;
pub const MAX_ATTEMPTS: i16 = 5;
const SEND_COOLDOWN_SECONDS: i64 = 60;
const SENDS_PER_EMAIL_HOUR: i64 = 5;
const SENDS_PER_IP_HOUR: i64 = 30;

/// Create (or replace) the active code for an email and return its plaintext
/// once. The caller sends it and must call [`discard`] if delivery fails.
pub async fn prepare(
    pool: &DbPool,
    email: &str,
    ip: Option<&str>,
    dev_code: Option<&str>,
) -> Result<String, UserError> {
    let client = pool.get().await?;
    let limits = client
        .query_one(
            r#"
SELECT
    EXISTS(
        SELECT 1 FROM auth_code_sends
        WHERE email = $1 AND sent_at > now() - make_interval(secs => $3)
    ) AS cooling_down,
    (SELECT count(*) FROM auth_code_sends
     WHERE email = $1 AND sent_at > now() - INTERVAL '1 hour') AS email_hour,
    (SELECT count(*) FROM auth_code_sends
     WHERE ip = $2 AND sent_at > now() - INTERVAL '1 hour') AS ip_hour
"#,
            &[&email, &ip, &(SEND_COOLDOWN_SECONDS as f64)],
        )
        .await?;
    let cooling_down: bool = limits.get(0);
    let email_hour: i64 = limits.get(1);
    let ip_hour: i64 = limits.get(2);
    if cooling_down {
        return Err(UserError::RateLimited {
            retry_after_secs: SEND_COOLDOWN_SECONDS,
        });
    }
    if email_hour >= SENDS_PER_EMAIL_HOUR || ip_hour >= SENDS_PER_IP_HOUR {
        return Err(UserError::RateLimited {
            retry_after_secs: 60 * 60,
        });
    }

    let code = match dev_code {
        Some(code) => code.to_string(),
        None => format!("{:06}", OsRng.gen_range(0..1_000_000)),
    };
    let code_hash = hash_code(&code)?;
    let expires_at = Utc::now() + Duration::minutes(CODE_TTL_MINUTES);
    client
        .execute(
            r#"
INSERT INTO auth_codes (email, code_hash, sent_at, expires_at, attempts)
VALUES ($1, $2, now(), $3, 0)
ON CONFLICT (email) DO UPDATE SET
    code_hash = EXCLUDED.code_hash,
    sent_at = now(),
    expires_at = EXCLUDED.expires_at,
    attempts = 0
"#,
            &[&email, &code_hash, &expires_at],
        )
        .await?;
    client
        .execute(
            "INSERT INTO auth_code_sends (email, ip) VALUES ($1, $2)",
            &[&email, &ip],
        )
        .await?;
    client
        .execute(
            "DELETE FROM auth_code_sends WHERE sent_at < now() - INTERVAL '1 day'",
            &[],
        )
        .await?;
    Ok(code)
}

/// Remove the code for an email, e.g. when delivery failed.
pub async fn discard(pool: &DbPool, email: &str) -> Result<(), UserError> {
    let client = pool.get().await?;
    client
        .execute("DELETE FROM auth_codes WHERE email = $1", &[&email])
        .await?;
    Ok(())
}

/// Check a code and consume it on success. A wrong code increments the attempt
/// counter; the fifth wrong attempt deletes the code.
pub async fn verify(
    pool: &DbPool,
    email: &str,
    code: &str,
    now: DateTime<Utc>,
) -> Result<bool, UserError> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT code_hash, expires_at, attempts FROM auth_codes WHERE email = $1",
            &[&email],
        )
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let code_hash: String = row.get(0);
    let expires_at: DateTime<Utc> = row.get(1);
    let attempts: i16 = row.get(2);
    if expires_at <= now || attempts >= MAX_ATTEMPTS {
        discard(pool, email).await?;
        return Ok(false);
    }
    if verify_code(code, &code_hash)? {
        discard(pool, email).await?;
        return Ok(true);
    }
    let next = attempts + 1;
    if next >= MAX_ATTEMPTS {
        client
            .execute("DELETE FROM auth_codes WHERE email = $1", &[&email])
            .await?;
    } else {
        client
            .execute(
                "UPDATE auth_codes SET attempts = $2 WHERE email = $1",
                &[&email, &next],
            )
            .await?;
    }
    Ok(false)
}

/// Mark the account's email as proven and return the user id.
pub async fn mark_email_verified(pool: &DbPool, user_id: Uuid) -> Result<(), UserError> {
    let client = pool.get().await?;
    client
        .execute(
            "UPDATE users SET email_verified_at = COALESCE(email_verified_at, now()) WHERE id = $1",
            &[&user_id],
        )
        .await?;
    Ok(())
}

fn hash_code(code: &str) -> Result<String, UserError> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(argon2::Argon2::default()
        .hash_password(code.as_bytes(), &salt)
        .map_err(|error| UserError::Other(anyhow::anyhow!("hash auth code: {error}")))?
        .to_string())
}

fn verify_code(code: &str, encoded: &str) -> Result<bool, UserError> {
    let parsed = PasswordHash::new(encoded)
        .map_err(|error| UserError::Other(anyhow::anyhow!("parse auth code hash: {error}")))?;
    Ok(argon2::Argon2::default()
        .verify_password(code.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_digit_code_roundtrips_the_hash() {
        let code = "042917";
        let hash = hash_code(code).unwrap();
        assert!(verify_code(code, &hash).unwrap());
        assert!(!verify_code("042918", &hash).unwrap());
    }

    #[test]
    fn generated_codes_are_six_digits() {
        for _ in 0..100 {
            let code = format!("{:06}", OsRng.gen_range(0..1_000_000));
            assert_eq!(code.len(), 6);
            assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
        }
    }
}

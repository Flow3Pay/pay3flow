use chrono::Utc;
use uuid::Uuid;

use crate::core::jwt::Jwt;
use crate::db::repo::user as users;
use crate::db::DbPool;

use super::error::UserError;
use super::{codes, mail::Mailer, register};

/// How an authenticated email is allowed to resolve an account.
///
/// `Any` is the one-page flow, `NewOnly` and `ExistingOnly` keep the deprecated
/// `/api/auth/register` and `/api/auth/login` aliases behaving as before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Any,
    NewOnly,
    ExistingOnly,
}

/// Create and deliver a sign-in code. The response never depends on whether
/// the email is registered (§4.3).
pub async fn request_code(
    pool: &DbPool,
    mailer: &Mailer,
    email: &str,
    ip: Option<&str>,
) -> Result<(), UserError> {
    let email = register::normalize_email(email)?;
    let code = codes::prepare(pool, &email, ip, mailer.dev_code()).await?;
    if mailer.dev_code().is_some() {
        tracing::warn!(%email, "AUTH_DEV_CODE is enabled; skipping email delivery");
        return Ok(());
    }
    if let Err(error) = mailer.send_code(&email, &code).await {
        // A code the user never received must not stay valid.
        if let Err(discard) = codes::discard(pool, &email).await {
            tracing::warn!(error = %discard, "could not discard undelivered auth code");
        }
        return Err(error);
    }
    Ok(())
}

/// Register-or-login: verify the code, then create or reuse the account.
pub async fn authenticate(
    pool: &DbPool,
    jwt: &Jwt,
    email: &str,
    code: &str,
    referral_code: Option<&str>,
) -> Result<String, UserError> {
    let id = resolve_user(pool, email, code, referral_code, Mode::Any).await?;
    Ok(jwt.sign(&id.to_string())?)
}

/// Deprecated alias: fails when the email is already registered.
pub async fn login(pool: &DbPool, jwt: &Jwt, email: &str, code: &str) -> Result<String, UserError> {
    let id = resolve_user(pool, email, code, None, Mode::ExistingOnly).await?;
    Ok(jwt.sign(&id.to_string())?)
}

/// Deprecated alias: creates the account and fails when it already exists.
pub async fn register(
    pool: &DbPool,
    email: &str,
    code: &str,
    referral_code: Option<&str>,
) -> Result<Uuid, UserError> {
    resolve_user(pool, email, code, referral_code, Mode::NewOnly).await
}

async fn resolve_user(
    pool: &DbPool,
    email: &str,
    code: &str,
    referral_code: Option<&str>,
    mode: Mode,
) -> Result<Uuid, UserError> {
    let email = register::normalize_email(email)?;
    if !codes::verify(pool, &email, code, Utc::now()).await? {
        return Err(UserError::InvalidCode);
    }
    let existing = users::user_id_by_email(pool, &email).await?;
    let id = match (existing, mode) {
        (Some(_), Mode::NewOnly) => return Err(UserError::AlreadyExists),
        (None, Mode::ExistingOnly) => return Err(UserError::NotFound),
        (Some(id), _) => id,
        (None, _) => {
            let referral_code = referral_code
                .map(register::normalize_referral_code)
                .transpose()?;
            match users::insert_user(pool, &email, referral_code.as_deref()).await? {
                users::InsertUserOutcome::Created(id) => id,
                users::InsertUserOutcome::EmailExists => users::user_id_by_email(pool, &email)
                    .await?
                    .ok_or(UserError::NotFound)?,
                users::InsertUserOutcome::ReferralCodeNotFound => {
                    return Err(UserError::InvalidReferralCode)
                }
            }
        }
    };
    codes::mark_email_verified(pool, id).await?;
    Ok(id)
}

pub async fn current_user(pool: &DbPool, jwt: &Jwt, token: &str) -> Result<users::User, UserError> {
    let id = jwt.verify(token).map_err(|_| UserError::NotFound)?;
    let id = Uuid::parse_str(&id).map_err(|_| UserError::NotFound)?;
    users::user_by_id(pool, &id)
        .await?
        .ok_or(UserError::NotFound)
}

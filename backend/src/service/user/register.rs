use anyhow::Result;
use uuid::Uuid;

use crate::db::repo::user as users;
use crate::db::DbPool;

use super::{error::UserError, mail};

pub fn send_auth_code(email: &str) -> Result<()> {
    tracing::info!("mail stub: would send auth code to {email}");
    Ok(())
}

pub async fn register_user(
    pool: &DbPool,
    email: &str,
    code: &str,
    referral_code: Option<&str>,
) -> Result<Uuid, UserError> {
    validate_email(email)?;
    if !mail::verify_auth_code(email, code) {
        return Err(UserError::InvalidCode);
    }
    mail::send_code(email).map_err(UserError::from)?;
    let referral_code = referral_code.map(normalize_referral_code).transpose()?;
    match users::insert_user(pool, email, referral_code.as_deref()).await? {
        users::InsertUserOutcome::Created(id) => Ok(id),
        users::InsertUserOutcome::EmailExists => Err(UserError::AlreadyExists),
        users::InsertUserOutcome::ReferralCodeNotFound => Err(UserError::InvalidReferralCode),
    }
}

fn normalize_referral_code(value: &str) -> Result<String, UserError> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(UserError::InvalidReferralCode)
    }
}

fn validate_email(email: &str) -> Result<(), UserError> {
    let valid =
        !email.is_empty() && email.len() <= 254 && email.contains('@') && !email.starts_with('@');
    if valid {
        Ok(())
    } else {
        Err(UserError::InvalidEmail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referral_codes_are_normalized_and_validated() {
        assert_eq!(
            normalize_referral_code(" abcd1234abcd1234 ").unwrap(),
            "ABCD1234ABCD1234"
        );
        assert!(matches!(
            normalize_referral_code("not-a-code"),
            Err(UserError::InvalidReferralCode)
        ));
    }
}

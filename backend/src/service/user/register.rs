use uuid::Uuid;

use crate::db::DbPool;

use super::auth;
use super::error::UserError;

/// Deprecated alias for `/api/auth/register`: the code is already delivered by
/// `/api/auth/code`, so this only verifies it and creates the account.
pub async fn register_user(
    pool: &DbPool,
    email: &str,
    code: &str,
    referral_code: Option<&str>,
) -> Result<Uuid, UserError> {
    auth::register(pool, email, code, referral_code).await
}

/// Lowercase and trim so the same person always hits the same account.
pub fn normalize_email(email: &str) -> Result<String, UserError> {
    let email = email.trim().to_ascii_lowercase();
    let valid =
        !email.is_empty() && email.len() <= 254 && email.contains('@') && !email.starts_with('@');
    if valid {
        Ok(email)
    } else {
        Err(UserError::InvalidEmail)
    }
}

pub fn normalize_referral_code(value: &str) -> Result<String, UserError> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(UserError::InvalidReferralCode)
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

    #[test]
    fn emails_are_trimmed_and_lowercased() {
        assert_eq!(
            normalize_email("  User@Example.COM ").unwrap(),
            "user@example.com"
        );
        assert!(matches!(
            normalize_email("not-an-email"),
            Err(UserError::InvalidEmail)
        ));
    }
}

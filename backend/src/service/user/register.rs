use anyhow::Result;
use uuid::Uuid;

use crate::db::repo::user as users;
use crate::db::DbPool;

use super::{error::UserError, mail};

pub fn send_auth_code(email: &str) -> Result<()> {
    tracing::info!("mail stub: would send auth code to {email}");
    Ok(())
}

pub async fn register_user(pool: &DbPool, email: &str, code: &str) -> Result<Uuid, UserError> {
    validate_email(email)?;
    if !mail::verify_auth_code(email, code) {
        return Err(UserError::InvalidCode);
    }
    mail::send_code(email).map_err(UserError::from)?;
    let id = users::insert_user(pool, email).await?;
    id.ok_or(UserError::AlreadyExists)
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

use uuid::Uuid;

use crate::core::db::DbPool;
use crate::core::jwt::Jwt;

use super::{error::UserError, mail, store};

pub async fn login(
    pool: &DbPool,
    jwt: &Jwt,
    email: &str,
    code: &str,
) -> Result<String, UserError> {
    if !mail::verify_auth_code(email, code) {
        return Err(UserError::InvalidCode);
    }
    let id = store::user_id_by_email(pool, email).await?.ok_or(UserError::NotFound)?;
    Ok(jwt.sign(&id.to_string())?)
}

pub async fn current_user(
    pool: &DbPool,
    jwt: &Jwt,
    token: &str,
) -> Result<store::User, UserError> {
    let id = jwt.verify(token).map_err(|_| UserError::NotFound)?;
    let id = Uuid::parse_str(&id).map_err(|_| UserError::NotFound)?;
    store::user_by_id(pool, &id).await?.ok_or(UserError::NotFound)
}
use crate::core::jwt::Jwt;
use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub jwt: Jwt,
}

impl AppState {
    pub fn new(pool: DbPool, jwt: Jwt) -> Self {
        Self { pool, jwt }
    }
}
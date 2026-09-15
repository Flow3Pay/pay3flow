use crate::activitypub;
use crate::core::jwt::Jwt;
use crate::db::DbPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub jwt: Jwt,
    pub ap: activitypub::Service,
}

impl AppState {
    pub fn new(pool: DbPool, jwt: Jwt, ap: activitypub::Service) -> Self {
        Self { pool, jwt, ap }
    }
}
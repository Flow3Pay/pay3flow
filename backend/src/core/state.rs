use crate::activitypub;
use crate::core::jwt::Jwt;
use crate::db::DbPool;
use crate::routing::RoutePicker;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub jwt: Jwt,
    pub ap: activitypub::Service,
    pub picker: RoutePicker,
}

impl AppState {
    pub fn new(pool: DbPool, jwt: Jwt, ap: activitypub::Service, picker: RoutePicker) -> Self {
        Self {
            pool,
            jwt,
            ap,
            picker,
        }
    }
}

use crate::activitypub;
use crate::core::jwt::Jwt;
use crate::db::DbPool;
use crate::payments::PaymentService;
use crate::routing::RoutePicker;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub jwt: Jwt,
    pub ap: activitypub::Service,
    pub picker: RoutePicker,
    pub payments: PaymentService,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: DbPool,
        jwt: Jwt,
        ap: activitypub::Service,
        picker: RoutePicker,
        payments: PaymentService,
    ) -> Self {
        Self {
            pool,
            jwt,
            ap,
            picker,
            payments,
        }
    }
}
use crate::activitypub;
use crate::banks::BanksService;
use crate::core::jwt::Jwt;
use crate::core::redis::RedisPool;
use crate::db::DbPool;
use crate::p2p::P2pSearchService;
use crate::pairs::ExchangePairsService;
use crate::payments::PaymentService;
use crate::route_engine::{
    LiveEdgeQuoteSource, NearIntentsProvider, RouteEngine, RouteQuoteService,
};
use crate::routing::RoutePicker;
use crate::service_reputation::ServiceReputation;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub jwt: Jwt,
    pub ap: activitypub::Service,
    pub picker: RoutePicker,
    pub payments: PaymentService,
    /// Bank exchange-pair router catalog (PLAN 2△ / 46c).
    pub pairs: ExchangePairsService,
    /// Worldwide payment-method directory the swap form picks from (PLAN 2△).
    pub banks: BanksService,
    /// Bearer token guarding the admin endpoints (PLAN 46e).
    pub admin_token: String,
    /// Optional Redis pool for fmatch candidate caching (PLAN #37d).
    pub redis: Option<RedisPool>,
    /// Read-only fan-out search over public P2P advertisement boards.
    pub p2p: P2pSearchService,
    /// Private route capability graph and stable fmatch route registry.
    pub route_engine: RouteEngine,
    /// NEAR Intents provider used by internal route pricing and refreshes.
    pub near_intents: NearIntentsProvider,
    /// Internal live quote service addressed by a previously published route id.
    pub route_quotes: Arc<RouteQuoteService<LiveEdgeQuoteSource>>,
    /// Global usage and feedback for services shown in public route results.
    pub reputation: ServiceReputation,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: DbPool,
        jwt: Jwt,
        ap: activitypub::Service,
        picker: RoutePicker,
        payments: PaymentService,
        pairs: ExchangePairsService,
        banks: BanksService,
        admin_token: String,
        redis: Option<RedisPool>,
        p2p: P2pSearchService,
        route_engine: RouteEngine,
        near_intents: NearIntentsProvider,
        route_quotes: Arc<RouteQuoteService<LiveEdgeQuoteSource>>,
        reputation: ServiceReputation,
    ) -> Self {
        Self {
            pool,
            jwt,
            ap,
            picker,
            payments,
            pairs,
            banks,
            admin_token,
            redis,
            p2p,
            route_engine,
            near_intents,
            route_quotes,
            reputation,
        }
    }
}

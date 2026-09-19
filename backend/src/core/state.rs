use crate::activitypub;
use crate::banks::BanksService;
use crate::core::jwt::Jwt;
use crate::core::redis::RedisPool;
use crate::db::DbPool;
use crate::p2p::P2pSearchService;
use crate::pairs::ExchangePairsService;
use crate::payments::PaymentService;
use crate::routing::RoutePicker;
use crate::wallet::WalletService;

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
    /// Optional self-hosted EVM wallet. Disabled when WALLET_KEYSTORE_PATH is empty.
    pub wallet: Option<WalletService>,
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
        wallet: Option<WalletService>,
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
            wallet,
        }
    }
}

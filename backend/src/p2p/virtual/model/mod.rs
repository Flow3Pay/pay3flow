pub(in crate::p2p) mod provider;

use crate::p2p::routes::ExchangeMode;
use crate::p2p::{P2pOffer, P2pOfferMarket, P2pRoute, P2pSearchResponse, SourceStatus};

#[derive(Debug, Clone)]
pub(in crate::p2p) struct NormalizedRouteQuery {
    pub(in crate::p2p) source_currency: String,
    pub(in crate::p2p) target_currency: String,
    pub(in crate::p2p) source_amount: f64,
    pub(in crate::p2p) source_network: Option<String>,
    pub(in crate::p2p) target_network: Option<String>,
    pub(in crate::p2p) assets: Vec<String>,
    pub(in crate::p2p) assets_explicit: bool,
    pub(in crate::p2p) source_payment_method: Option<String>,
    pub(in crate::p2p) target_payment_method: Option<String>,
    pub(in crate::p2p) merchant_only: bool,
    pub(in crate::p2p) min_orders: Option<u64>,
    pub(in crate::p2p) min_completion_rate: Option<f64>,
    pub(in crate::p2p) allow_cross_venue: bool,
    pub(in crate::p2p) max_price_deviation_bps: u32,
    pub(in crate::p2p) limit: usize,
    pub(in crate::p2p) sources: Option<String>,
    pub(in crate::p2p) exchange_mode: ExchangeMode,
}

impl NormalizedRouteQuery {
    pub(in crate::p2p) fn includes_p2p(&self) -> bool {
        matches!(self.exchange_mode, ExchangeMode::All | ExchangeMode::P2p)
    }

    pub(in crate::p2p) fn includes_exchangers(&self) -> bool {
        matches!(
            self.exchange_mode,
            ExchangeMode::All | ExchangeMode::Exchanger
        )
    }

    pub(in crate::p2p) fn offer_market(&self) -> Option<P2pOfferMarket> {
        match self.exchange_mode {
            ExchangeMode::All => None,
            ExchangeMode::P2p => Some(P2pOfferMarket::P2p),
            ExchangeMode::Exchanger => Some(P2pOfferMarket::DirectExchange),
        }
    }

    pub(in crate::p2p) fn accepts_offer(&self, offer: &P2pOffer) -> bool {
        match offer.market {
            P2pOfferMarket::P2p => self.includes_p2p(),
            P2pOfferMarket::DirectExchange => self.includes_exchangers(),
        }
    }
}

/// A validated unit of route-search work produced independently by one
/// pipeline routine and consumed by the incremental aggregator.
pub(in crate::p2p) enum RouteBatch {
    FiatAsset {
        asset: String,
        entry: Box<P2pSearchResponse>,
        exit: Box<P2pSearchResponse>,
    },
    FiatToCrypto {
        asset: String,
        response: Box<P2pSearchResponse>,
    },
    CryptoToFiat {
        asset: String,
        response: Box<P2pSearchResponse>,
    },
    Routes {
        routes: Vec<P2pRoute>,
        exhaustive: bool,
    },
    ProviderResult {
        status: SourceStatus,
        routes: Vec<P2pRoute>,
        exhaustive: bool,
    },
}

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::{bail, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::p2p::service::{normalize_sources, P2pSearchQuery, P2pSide};
use crate::route_engine::{canonical_network_id, Asset};
mod query;
mod response;
pub(super) use super::r#virtual::model::provider::*;
pub(super) use super::r#virtual::model::NormalizedRouteQuery;
pub(super) use query::*;
pub(super) use response::*;
mod model;
pub(super) use super::r#virtual::composition::*;
pub use model::{
    CryptoMarketPath, ExchangeMode, P2pRoute, P2pRouteSearchQuery, P2pRouteSearchResponse,
    RouteAssetStatus, RouteCostKind, RouteExecutionDescriptor, RouteFee, RouteProfitability,
};

pub(super) const DEFAULT_ROUTE_LIMIT: usize = 20;
pub(super) const MAX_ROUTE_LIMIT: usize = 100;
pub(super) const LEG_SEARCH_LIMIT: usize = 60;
pub(super) const DEFAULT_MAX_PRICE_DEVIATION_BPS: u32 = 1_000;
pub(super) const MAX_PROVIDER_ASSETS: usize = 12;
pub(super) const MAX_PROVIDER_NETWORK_PAIRS: usize = 32;
pub(super) const MAX_PROVIDER_OFFERS_PER_LEG: usize = 8;
pub(super) const PROVIDER_QUOTE_CACHE_TTL: Duration = Duration::from_secs(30);
pub(super) const MAX_BACKGROUND_PROVIDER_REFRESHES_PER_SEARCH: usize = 8;

#[cfg(test)]
mod tests;

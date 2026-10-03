use std::time::Duration;

use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::time::timeout;
use uuid::Uuid;

use crate::core::redis;
use crate::p2p::routes::P2pRouteSearchResponse;
use crate::p2p::P2pSearchService;

use super::NormalizedRouteQuery;

const REDIS_TIMEOUT: Duration = Duration::from_millis(150);

pub(super) fn route_cache_key(query: &NormalizedRouteQuery) -> Option<String> {
    let encoded = serde_json::to_vec(query).ok()?;
    Some(format!("p2p:routes:v1:{:x}", Sha256::digest(encoded)))
}

pub(super) async fn cached_routes(
    service: &P2pSearchService,
    key: &str,
    search_id: Uuid,
) -> Option<P2pRouteSearchResponse> {
    if service.cache_ttl.is_zero() {
        return None;
    }
    let pool = service.route_cache_redis.as_ref()?;
    match timeout(
        REDIS_TIMEOUT,
        redis::get_json::<P2pRouteSearchResponse>(pool, key),
    )
    .await
    {
        Ok(Ok(Some(mut response))) => {
            response.search_id = search_id;
            Some(response)
        }
        Ok(Ok(None)) => None,
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to read public route cache");
            None
        }
        Err(_) => None,
    }
}

pub(super) async fn cache_routes(
    service: &P2pSearchService,
    key: &str,
    response: &P2pRouteSearchResponse,
) {
    if service.cache_ttl.is_zero() {
        return;
    }
    let Some(pool) = service.route_cache_redis.as_ref() else {
        return;
    };
    let mut public_response = response.clone();
    public_response.search_id = Uuid::nil();
    for route in &mut public_response.routes {
        route.services.clear();
        route.reputation = None;
        route.feedback = None;
        route.service_links.clear();
    }
    let now = Utc::now();
    let mut ttl = service.cache_ttl.as_secs().max(1);
    for expiry in public_response
        .routes
        .iter()
        .filter_map(|route| route.quote_expires_at)
    {
        let remaining = (expiry - now).num_seconds();
        if remaining <= 0 {
            return;
        }
        ttl = ttl.min(remaining as u64);
    }
    match timeout(
        REDIS_TIMEOUT,
        redis::set_json(pool, key, &public_response, ttl),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::warn!(%error, "failed to write public route cache"),
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::NetworkCatalog;
    use crate::p2p::routes::{normalize_query, ExchangeMode, P2pRouteSearchQuery};

    fn query() -> P2pRouteSearchQuery {
        P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "RUB".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: None,
            max_price_deviation_bps: None,
            limit: None,
            sources: None,
            exchange_mode: ExchangeMode::All,
        }
    }

    #[test]
    fn public_route_cache_key_uses_normalized_parameters_and_filters() {
        let normalize =
            |query| normalize_query(query, &[], &NetworkCatalog::test_default(), &[]).unwrap();
        let baseline = route_cache_key(&normalize(query())).unwrap();
        let mut equivalent = query();
        equivalent.source_fiat = " amd ".into();
        equivalent.limit = Some(20);
        assert_eq!(baseline, route_cache_key(&normalize(equivalent)).unwrap());

        let mut filtered = query();
        filtered.target_payment_method = Some("Sberbank".into());
        assert_ne!(baseline, route_cache_key(&normalize(filtered)).unwrap());
    }
}

use super::*;

pub(in crate::p2p) fn response_snapshot(
    search_id: Uuid,
    query: &NormalizedRouteQuery,
    routes: &HashMap<String, P2pRoute>,
    asset_statuses: &[RouteAssetStatus],
) -> P2pRouteSearchResponse {
    let routes_found = routes.len();
    let mut visible_routes = routes.values().cloned().collect::<Vec<_>>();
    sort_routes(&mut visible_routes);
    truncate_routes_preserving_providers(&mut visible_routes, query.limit);
    for (index, route) in visible_routes.iter_mut().enumerate() {
        route.rank = index + 1;
    }
    let (source, stale) = route_discovery_source(asset_statuses);
    P2pRouteSearchResponse {
        search_id,
        routes_found,
        routes_exhaustive: asset_statuses.iter().all(|status| status.routes_exhaustive),
        searched_at: Utc::now(),
        source_fiat: query.source_currency.clone(),
        target_fiat: query.target_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        assets_searched: query.assets.clone(),
        can_exchange_to_target: routes_found > 0,
        routes: visible_routes,
        asset_statuses: asset_statuses.to_vec(),
        source: source.into(),
        stale,
    }
}

pub(in crate::p2p) fn route_discovery_source(
    asset_statuses: &[RouteAssetStatus],
) -> (&'static str, bool) {
    let sources = asset_statuses.iter().flat_map(|status| {
        status
            .entry_discovery_source
            .iter()
            .chain(status.exit_discovery_source.iter())
            .map(String::as_str)
    });
    let mut has_fmatch = false;
    let mut has_cache = false;
    let mut has_fallback = false;
    for source in sources {
        has_fmatch |= source == "fmatch";
        has_cache |= source == "database_cache";
        has_fallback |= source == "provider_fallback";
    }
    if has_fallback {
        ("provider_fallback", false)
    } else if has_cache {
        ("database_cache", true)
    } else if has_fmatch {
        ("fmatch", false)
    } else {
        ("provider", false)
    }
}

pub(in crate::p2p) fn truncate_routes_preserving_providers(
    routes: &mut Vec<P2pRoute>,
    limit: usize,
) {
    if routes.len() <= limit {
        return;
    }

    let reserved_indices = {
        let mut represented_providers = HashSet::new();
        routes
            .iter()
            .enumerate()
            .filter_map(|(index, route)| {
                route_provider_names(route)
                    .iter()
                    .any(|provider| represented_providers.insert(*provider))
                    .then_some(index)
            })
            .take(limit)
            .collect::<HashSet<_>>()
    };
    let mut remaining = limit.saturating_sub(reserved_indices.len());
    let mut index = 0;
    routes.retain(|_| {
        let reserved = reserved_indices.contains(&index);
        index += 1;
        reserved
            || if remaining > 0 {
                remaining -= 1;
                true
            } else {
                false
            }
    });
}

pub(in crate::p2p) fn route_provider_names(route: &P2pRoute) -> Vec<&str> {
    let mut providers = Vec::new();
    if let Some(provider) = route.route_provider.as_deref() {
        providers.push(provider);
    }
    if let Some(provider) = route.market_path.as_ref().map(|path| path.venue.as_str()) {
        providers.push(provider);
    }
    if let Some(provider) = route
        .entry_offer
        .as_ref()
        .map(|offer| offer.source.as_str())
    {
        providers.push(provider);
    }
    if let Some(provider) = route.exit_offer.as_ref().map(|offer| offer.source.as_str()) {
        providers.push(provider);
    }
    providers
}

pub(in crate::p2p) fn sort_routes(routes: &mut [P2pRoute]) {
    routes.sort_by(|left, right| {
        route_target(right)
            .partial_cmp(&route_target(left))
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                right
                    .payment_methods_verified
                    .cmp(&left.payment_methods_verified)
            })
            .then_with(|| right.same_venue.cmp(&left.same_venue))
            .then_with(|| left.route_id.cmp(&right.route_id))
    });
}

pub(in crate::p2p) fn merge_routes(
    routes: &mut HashMap<String, P2pRoute>,
    discovered: Vec<P2pRoute>,
) -> usize {
    let before = routes.len();
    for mut route in discovered {
        let route_id = route_fingerprint(&route);
        route.route_id.clone_from(&route_id);
        match routes.get(&route_id) {
            Some(current) if route_target(current) >= route_target(&route) => {}
            _ => {
                routes.insert(route_id, route);
            }
        }
    }
    routes.len() - before
}

pub(in crate::p2p) fn route_fingerprint(route: &P2pRoute) -> String {
    let entry = route
        .entry_offer
        .as_ref()
        .map(|offer| {
            format!(
                "{}:{}:{}",
                offer.source,
                offer.ad_id,
                offer.payment_methods.join(",")
            )
        })
        .unwrap_or_default();
    let exit = route
        .exit_offer
        .as_ref()
        .map(|offer| {
            format!(
                "{}:{}:{}",
                offer.source,
                offer.ad_id,
                offer.payment_methods.join(",")
            )
        })
        .unwrap_or_default();
    let market = route
        .market_path
        .as_ref()
        .map(|path| format!("{}:{}:{}", path.venue, path.source_pair, path.target_pair))
        .unwrap_or_default();
    let mut identity = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        route.route_kind,
        route.asset,
        route.source_fiat,
        route.target_fiat,
        route.source_network.as_deref().unwrap_or_default(),
        route.target_network.as_deref().unwrap_or_default(),
        route.bridge_currency.as_deref().unwrap_or_default(),
        route.route_provider.as_deref().unwrap_or_default(),
        route.route_path.join(","),
        entry,
        exit,
        market,
        route.source_amount,
    );
    if let Some(quote_id) = route.provider_quote_id.as_deref() {
        identity.push('|');
        identity.push_str(quote_id);
    }
    format!("{:x}", Sha256::digest(identity.as_bytes()))
}

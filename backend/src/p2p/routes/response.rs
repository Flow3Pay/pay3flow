use super::*;
use crate::p2p::{P2pOfferMarket, RouteCostKind, RouteProfitability, SourceStatus};

pub(in crate::p2p) fn response_snapshot(
    search_id: Uuid,
    query: &NormalizedRouteQuery,
    routes: &HashMap<String, P2pRoute>,
    asset_statuses: &[RouteAssetStatus],
    provider_statuses: &HashMap<String, SourceStatus>,
) -> P2pRouteSearchResponse {
    let routes_found = routes.len();
    let mut visible_routes = routes.values().cloned().collect::<Vec<_>>();
    for route in &mut visible_routes {
        route.profitability = profitability_for_route(route, query);
    }
    sort_routes(&mut visible_routes);
    truncate_routes_preserving_providers(&mut visible_routes, query.limit);
    for (index, route) in visible_routes.iter_mut().enumerate() {
        route.rank = index + 1;
    }
    let (source, stale) = route_discovery_source(asset_statuses);
    let mut provider_statuses = provider_statuses.values().cloned().collect::<Vec<_>>();
    provider_statuses.sort_by(|left, right| left.source.cmp(&right.source));
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
        provider_statuses,
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
        let provider_count = routes
            .iter()
            .flat_map(route_provider_names)
            .collect::<HashSet<_>>()
            .len();
        if provider_count == 0 {
            routes.truncate(limit);
            return;
        }
        let per_provider = (limit / provider_count).clamp(1, 8);
        let mut provider_counts = HashMap::new();
        routes
            .iter()
            .enumerate()
            .filter_map(|(index, route)| {
                let providers = route_provider_names(route);
                if providers.iter().any(|provider| {
                    provider_counts.get(provider).copied().unwrap_or(0) < per_provider
                }) {
                    for provider in providers {
                        *provider_counts.entry(provider).or_insert(0) += 1;
                    }
                    Some(index)
                } else {
                    None
                }
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
    providers.sort_unstable();
    providers.dedup();
    providers
}

pub(in crate::p2p) fn sort_routes(routes: &mut [P2pRoute]) {
    routes.sort_by(|left, right| {
        profitability_rank(left)
            .cmp(&profitability_rank(right))
            .then_with(|| profitability_amount(right).cmp(&profitability_amount(left)))
            .then_with(|| {
                route_target(right)
                    .partial_cmp(&route_target(left))
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                right
                    .payment_methods_verified
                    .cmp(&left.payment_methods_verified)
            })
            .then_with(|| right.same_venue.cmp(&left.same_venue))
            .then_with(|| left.route_id.cmp(&right.route_id))
    });
}

fn profitability_rank(route: &P2pRoute) -> u8 {
    match route.profitability.as_ref() {
        Some(RouteProfitability::Confirmed {
            net_profit_minor, ..
        }) if *net_profit_minor > 0 => 0,
        Some(RouteProfitability::Unconfirmed { .. }) => 1,
        Some(RouteProfitability::Confirmed { .. }) => 2,
        None => 3,
    }
}

fn profitability_amount(route: &P2pRoute) -> i64 {
    match route.profitability.as_ref() {
        Some(RouteProfitability::Confirmed {
            net_profit_minor, ..
        }) => *net_profit_minor,
        Some(RouteProfitability::Unconfirmed {
            gross_profit_minor, ..
        }) => *gross_profit_minor,
        None => i64::MIN,
    }
}

pub(in crate::p2p) fn profitability_for_route(
    route: &P2pRoute,
    query: &NormalizedRouteQuery,
) -> Option<RouteProfitability> {
    if !matches!(route.route_kind.as_str(), "crypto_cycle" | "fiat_cycle") {
        return None;
    }
    let source_minor = minor_units(&route.source_amount)?;
    let target_minor = minor_units(&route.target_amount)?;
    let gross_profit_minor = target_minor.checked_sub(source_minor)?;
    let mut missing_costs = Vec::new();

    let source_fee = match query.source_payment_fee_bps {
        Some(bps) => fee_minor(source_minor, bps)?,
        None => {
            missing_costs.push(RouteCostKind::SourcePaymentFee);
            0
        }
    };
    let target_fee = match query.target_payment_fee_bps {
        Some(bps) => fee_minor(target_minor, bps)?,
        None => {
            missing_costs.push(RouteCostKind::TargetPaymentFee);
            0
        }
    };
    if route.requires_asset_transfer && !route.transfer_fee_included {
        missing_costs.push(RouteCostKind::NetworkFee);
    }
    if route.route_provider.is_some() && !route.transfer_fee_included {
        missing_costs.push(RouteCostKind::LiveQuote);
    }
    if route.route_kind == "fiat_cycle"
        || route
            .entry_offer
            .iter()
            .chain(route.exit_offer.iter())
            .any(|offer| offer.market == P2pOfferMarket::DirectExchange)
    {
        missing_costs.push(RouteCostKind::ProviderFee);
    }
    missing_costs.sort_unstable();
    missing_costs.dedup();

    if missing_costs.is_empty() {
        let net_profit_minor = target_minor
            .checked_sub(target_fee)?
            .checked_sub(source_minor.checked_add(source_fee)?)?;
        Some(RouteProfitability::Confirmed {
            net_profit_minor,
            profit_bps: profit_bps(net_profit_minor, source_minor)?,
        })
    } else {
        Some(RouteProfitability::Unconfirmed {
            gross_profit_minor,
            gross_profit_bps: profit_bps(gross_profit_minor, source_minor)?,
            missing_costs,
        })
    }
}

fn minor_units(value: &str) -> Option<i64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.starts_with('-') || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut cents = fraction.bytes().take(2).collect::<Vec<_>>();
    if cents.iter().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    while cents.len() < 2 {
        cents.push(b'0');
    }
    let whole = whole.parse::<i64>().ok()?;
    let fraction = std::str::from_utf8(&cents).ok()?.parse::<i64>().ok()?;
    whole.checked_mul(100)?.checked_add(fraction)
}

fn fee_minor(amount_minor: i64, fee_bps: u32) -> Option<i64> {
    let numerator = i128::from(amount_minor)
        .checked_mul(i128::from(fee_bps))?
        .checked_add(9_999)?;
    i64::try_from(numerator / 10_000).ok()
}

fn profit_bps(profit_minor: i64, source_minor: i64) -> Option<i32> {
    if source_minor <= 0 {
        return None;
    }
    let numerator = i128::from(profit_minor).checked_mul(10_000)?;
    i32::try_from(numerator / i128::from(source_minor)).ok()
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

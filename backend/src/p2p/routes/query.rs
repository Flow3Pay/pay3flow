use super::*;

pub(in crate::p2p) fn normalize_query(
    query: P2pRouteSearchQuery,
    default_assets: &[String],
    networks: &crate::networks::NetworkCatalog,
    provider_assets: &[Asset],
) -> Result<NormalizedRouteQuery> {
    if !query.source_amount.is_finite() || query.source_amount <= 0.0 {
        bail!("source_amount must be a positive finite number");
    }
    let source_currency = query.source_fiat.trim().to_ascii_uppercase();
    let target_currency = query.target_fiat.trim().to_ascii_uppercase();
    if query
        .min_completion_rate
        .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
    {
        bail!("min_completion_rate must be between 0 and 1");
    }
    let assets_explicit = query.intermediary_assets.is_some() || query.assets.is_some();
    let assets = query
        .intermediary_assets
        .as_deref()
        .or(query.assets.as_deref())
        .map(|assets| assets.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_else(|| default_assets.to_vec())
        .into_iter()
        .map(|asset| asset.trim().to_ascii_uppercase())
        .filter(|asset| !asset.is_empty())
        .collect::<Vec<_>>();
    if assets.is_empty() || assets.len() > 24 {
        bail!("assets must contain between 1 and 24 comma-separated codes");
    }
    if assets.iter().any(|asset| {
        !(2..=12).contains(&asset.len()) || !asset.bytes().all(|b| b.is_ascii_alphanumeric())
    }) {
        bail!("every asset must be a 2-12 character alphanumeric code");
    }
    let max_price_deviation_bps = query
        .max_price_deviation_bps
        .unwrap_or(DEFAULT_MAX_PRICE_DEVIATION_BPS);
    if max_price_deviation_bps > 5_000 {
        bail!("max_price_deviation_bps must not exceed 5000");
    }
    let source_network = validate_network(
        networks,
        &query.source_network,
        &source_currency,
        provider_assets,
    )?;
    let target_network = validate_network(
        networks,
        &query.target_network,
        &target_currency,
        provider_assets,
    )?;
    if source_currency.eq_ignore_ascii_case(&target_currency) {
        match (&source_network, &target_network) {
            (Some(source_network), Some(target_network)) if source_network == target_network => {
                bail!("source and target asset/network must differ")
            }
            _ => {}
        }
    }

    Ok(NormalizedRouteQuery {
        source_currency,
        target_currency,
        source_amount: query.source_amount,
        source_network,
        target_network,
        assets,
        assets_explicit,
        source_payment_method: trimmed(query.source_payment_method),
        target_payment_method: trimmed(query.target_payment_method),
        merchant_only: query.merchant_only.unwrap_or(false),
        min_orders: query.min_orders,
        min_completion_rate: query.min_completion_rate,
        allow_cross_venue: query.allow_cross_venue.unwrap_or(false),
        max_price_deviation_bps,
        limit: query
            .limit
            .unwrap_or(DEFAULT_ROUTE_LIMIT)
            .clamp(1, MAX_ROUTE_LIMIT),
        sources: normalize_sources(query.sources)?,
        exchange_mode: query.exchange_mode,
    })
}

pub(in crate::p2p) fn source_selected(query: &NormalizedRouteQuery, provider: &str) -> bool {
    query
        .sources
        .as_deref()
        .is_none_or(|sources| sources.split(',').any(|source| source == provider))
}

pub(in crate::p2p) fn intermediary_asset_priority(symbol: &str, target_symbol: &str) -> u8 {
    if symbol.eq_ignore_ascii_case(target_symbol) {
        0
    } else {
        match symbol.to_ascii_uppercase().as_str() {
            "USDT" => 1,
            "USDC" => 2,
            "BTC" => 3,
            "ETH" => 4,
            _ => 5,
        }
    }
}

pub(in crate::p2p) fn validate_network(
    networks: &crate::networks::NetworkCatalog,
    network_id: &Option<String>,
    currency: &str,
    provider_assets: &[Asset],
) -> Result<Option<String>> {
    let Some(network_id) = network_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let canonical_id = canonical_network_id(network_id);
    let network = networks.compatible_network(&canonical_id, currency);
    match network {
        Some(network) => Ok(Some(network.id.clone())),
        None if provider_assets.iter().any(|asset| {
            asset.symbol.eq_ignore_ascii_case(currency)
                && asset.location.as_deref() == Some(canonical_id.as_str())
        }) =>
        {
            Ok(Some(canonical_id))
        }
        None => bail!("network {network_id} is not compatible with {currency}"),
    }
}

pub(in crate::p2p) fn is_crypto_currency(
    currency: &str,
    networks: &crate::networks::NetworkCatalog,
    provider_assets: &[Asset],
) -> bool {
    networks.is_supported_asset(currency)
        || provider_assets
            .iter()
            .any(|asset| asset.symbol.eq_ignore_ascii_case(currency))
}

pub(in crate::p2p) fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(in crate::p2p) fn leg_query(
    fiat: &str,
    asset: &str,
    side: P2pSide,
    amount: Option<f64>,
    payment_method: Option<String>,
    route: &NormalizedRouteQuery,
) -> P2pSearchQuery {
    P2pSearchQuery {
        fiat: fiat.into(),
        asset: asset.into(),
        side,
        amount,
        payment_method,
        merchant_only: Some(route.merchant_only),
        min_orders: route.min_orders,
        min_completion_rate: route.min_completion_rate,
        limit: Some(LEG_SEARCH_LIMIT),
        sources: route.sources.clone(),
    }
}

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
    if query
        .max_actions
        .is_some_and(|limit| !(2..=8).contains(&limit))
    {
        bail!("max_actions must be between 2 and 8");
    }
    let source_currency = canonical_query_currency(&query.source_fiat);
    let target_currency = canonical_query_currency(&query.target_fiat);
    if query
        .min_completion_rate
        .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
    {
        bail!("min_completion_rate must be between 0 and 1");
    }
    let source_payment_fee_bps = payment_fee_bps(
        query.source_payment_fee_percent,
        "source_payment_fee_percent",
    )?;
    let target_payment_fee_bps = payment_fee_bps(
        query.target_payment_fee_percent,
        "target_payment_fee_percent",
    )?;
    let assets_explicit = query.intermediary_assets.is_some() || query.assets.is_some();
    let mut assets = query
        .intermediary_assets
        .as_deref()
        .or(query.assets.as_deref())
        .map(|assets| assets.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_else(|| default_assets.to_vec())
        .into_iter()
        .map(|asset| canonical_query_currency(&asset))
        .filter(|asset| !asset.is_empty())
        .collect::<Vec<_>>();
    // Bound caller-supplied search lists without limiting the backend catalog.
    if assets.is_empty() || (assets_explicit && assets.len() > 24) {
        bail!("assets must contain between 1 and 24 comma-separated codes");
    }
    if assets_explicit {
        if assets.iter().any(|asset| !is_asset_code(asset)) {
            bail!("every asset must be a 2-12 character alphanumeric code");
        }
    } else {
        // Defaults come from our own network catalog. A ticker that the route engine
        // cannot name (a dotted bridged asset, say) must skip out of the seed list,
        // not reject every search that did not name assets itself.
        let skipped = assets
            .iter()
            .filter(|asset| !is_asset_code(asset))
            .cloned()
            .collect::<Vec<_>>();
        if !skipped.is_empty() {
            tracing::warn!(assets = ?skipped, "p2p.query.unusable_catalog_asset_skipped");
            assets.retain(|asset| is_asset_code(asset));
        }
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
        source_payment_fee_bps,
        target_payment_fee_bps,
        merchant_only: query.merchant_only.unwrap_or(false),
        min_orders: query.min_orders,
        min_completion_rate: query.min_completion_rate,
        allow_cross_venue: query.allow_cross_venue.unwrap_or(false),
        max_actions: query.max_actions,
        max_price_deviation_bps,
        limit: query
            .limit
            .unwrap_or(DEFAULT_ROUTE_LIMIT)
            .clamp(1, MAX_ROUTE_LIMIT),
        sources: normalize_sources(query.sources)?,
        exchange_mode: query.exchange_mode,
    })
}

fn canonical_query_currency(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "TON" => "GRAM".into(),
        value => value.into(),
    }
}

fn is_asset_code(asset: &str) -> bool {
    (2..=12).contains(&asset.len()) && asset.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn payment_fee_bps(value: Option<f64>, field: &str) -> Result<Option<u32>> {
    let Some(percent) = value else {
        return Ok(None);
    };
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        bail!("{field} must be between 0 and 100");
    }
    Ok(Some((percent * 100.0).round() as u32))
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
    network: Option<&str>,
    networks: &crate::networks::NetworkCatalog,
    provider_assets: &[Asset],
) -> bool {
    // A token catalog must not redefine a user's fiat selection. An explicit,
    // validated blockchain network disambiguates a token with the same ticker.
    if network.is_none() && is_fiat_currency(currency) {
        return false;
    }
    networks.is_supported_asset(currency)
        || provider_assets
            .iter()
            .any(|asset| asset.symbol.eq_ignore_ascii_case(currency))
}

pub(in crate::p2p) fn is_fiat_currency(currency: &str) -> bool {
    // ISO 4217 money codes, including legacy codes still used by some venues.
    const CODES: &[&str] = &[
        "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
        "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BYR", "BZD",
        "CAD", "CDF", "CHF", "CLP", "CNY", "COP", "CRC", "CUC", "CUP", "CVE", "CZK", "DJF", "DKK",
        "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP", "GEL", "GHS", "GIP", "GMD",
        "GNF", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG", "HUF", "IDR", "ILS", "INR", "IQD", "IRR",
        "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW", "KRW", "KWD", "KYD", "KZT",
        "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA", "MKD", "MMK", "MNT", "MOP",
        "MRU", "MUR", "MVR", "MWK", "MXN", "MYR", "MZN", "NAD", "NGN", "NIO", "NOK", "NPR", "NZD",
        "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG", "QAR", "RON", "RSD", "RUB", "RUR",
        "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE", "SLL", "SOS", "SRD", "SSP",
        "STN", "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY", "TTD", "TWD", "TZS",
        "UAH", "UGX", "USD", "UYU", "UZS", "VES", "VND", "VUV", "WST", "XAF", "XCD", "XCG", "XOF",
        "XPF", "YER", "ZAR", "ZMW", "ZWG", "ZWL",
    ];
    CODES.contains(&currency)
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
        asset_amount: None,
        payment_method,
        merchant_only: Some(route.merchant_only),
        min_orders: route.min_orders,
        min_completion_rate: route.min_completion_rate,
        limit: Some(LEG_SEARCH_LIMIT),
        sources: route.sources.clone(),
    }
}

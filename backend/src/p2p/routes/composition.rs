use super::*;

pub(super) fn matching_offers(offers: &[P2pOffer], query: &NormalizedRouteQuery) -> Vec<P2pOffer> {
    offers
        .iter()
        .filter(|offer| query.accepts_offer(offer))
        .cloned()
        .collect()
}

pub(super) fn reject_price_outliers(
    offers: Vec<P2pOffer>,
    max_deviation_bps: u32,
) -> Vec<P2pOffer> {
    if offers.len() < 3 || max_deviation_bps == 0 {
        return offers;
    }
    let mut prices = offers
        .iter()
        .filter(|offer| offer.market == P2pOfferMarket::P2p)
        .filter_map(|offer| offer.price.parse::<f64>().ok())
        .filter(|price| price.is_finite() && *price > 0.0)
        .collect::<Vec<_>>();
    if prices.len() < 3 {
        return offers;
    }
    prices.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let median = prices[prices.len() / 2];
    let maximum_deviation = median * f64::from(max_deviation_bps) / 10_000.0;
    offers
        .into_iter()
        .filter(|offer| {
            offer.market == P2pOfferMarket::DirectExchange
                || offer
                    .price
                    .parse::<f64>()
                    .is_ok_and(|price| (price - median).abs() <= maximum_deviation)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct FiatRouteCandidate {
    target_amount: f64,
    entry_index: usize,
    exit_index: usize,
}

impl PartialEq for FiatRouteCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.target_amount.total_cmp(&other.target_amount) == Ordering::Equal
            && self.entry_index == other.entry_index
            && self.exit_index == other.exit_index
    }
}

impl Eq for FiatRouteCandidate {}

impl PartialOrd for FiatRouteCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FiatRouteCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.target_amount
            .total_cmp(&other.target_amount)
            .then_with(|| other.entry_index.cmp(&self.entry_index))
            .then_with(|| other.exit_index.cmp(&self.exit_index))
    }
}

pub(super) fn compose_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
) -> bool {
    let mut entries = entry_offers
        .iter()
        .filter_map(|entry| {
            let entry_price = positive_number(&entry.price)?;
            let acquired_asset = query.source_amount / entry_price;
            positive_number(&entry.available_asset)
                .is_none_or(|available| available >= acquired_asset)
                .then_some((entry, entry_price, acquired_asset))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut exits = exit_offers
        .iter()
        .filter_map(|exit| positive_number(&exit.price).map(|price| (exit, price)))
        .collect::<Vec<_>>();
    exits.sort_by(|left, right| right.1.total_cmp(&left.1));
    if entries.is_empty() || exits.is_empty() {
        return true;
    }

    let candidate_limit = query.limit.saturating_mul(3).max(query.limit);
    routes.reserve(candidate_limit.min(entries.len().saturating_mul(exits.len())));
    let routes_start = routes.len();
    let mut evaluated = HashSet::new();
    let mut source_names = Vec::new();
    let mut seen_sources = HashSet::new();
    for index in 0..entries.len().max(exits.len()) {
        if let Some((entry, _, _)) = entries.get(index) {
            if seen_sources.insert(entry.source.as_str()) {
                source_names.push(entry.source.as_str());
            }
        }
        if let Some((exit, _)) = exits.get(index) {
            if seen_sources.insert(exit.source.as_str()) {
                source_names.push(exit.source.as_str());
            }
        }
    }
    for source in source_names {
        let mut best = None;
        for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
            if entry.source != source {
                continue;
            }
            for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
            if exit.source != source {
                continue;
            }
            for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        if let Some((_, entry_index, exit_index, route)) = best {
            if evaluated.insert((entry_index, exit_index)) {
                routes.push(route);
            }
        }
        if routes.len() - routes_start >= candidate_limit {
            routes[routes_start..].sort_by(|left, right| {
                route_target(right)
                    .partial_cmp(&route_target(left))
                    .unwrap_or(Ordering::Equal)
            });
            return false;
        }
    }

    let mut frontier = BinaryHeap::with_capacity(entries.len());
    for (entry_index, (_, _, acquired_asset)) in entries.iter().enumerate() {
        frontier.push(FiatRouteCandidate {
            target_amount: acquired_asset * exits[0].1,
            entry_index,
            exit_index: 0,
        });
    }

    while let Some(candidate) = frontier.pop() {
        let (entry, _, acquired_asset) = entries[candidate.entry_index];
        let (exit, exit_price) = exits[candidate.exit_index];
        let next_exit_index = candidate.exit_index + 1;
        if next_exit_index < exits.len() {
            frontier.push(FiatRouteCandidate {
                target_amount: acquired_asset * exits[next_exit_index].1,
                entry_index: candidate.entry_index,
                exit_index: next_exit_index,
            });
        }

        if evaluated.insert((candidate.entry_index, candidate.exit_index)) {
            if let Some(route) = compose_fiat_route(
                query,
                asset,
                entry,
                exit,
                acquired_asset,
                acquired_asset * exit_price,
            ) {
                routes.push(route);
                if routes.len() - routes_start >= candidate_limit {
                    routes[routes_start..].sort_by(|left, right| {
                        route_target(right)
                            .partial_cmp(&route_target(left))
                            .unwrap_or(Ordering::Equal)
                    });
                    return frontier.is_empty();
                }
            }
        }
    }
    routes[routes_start..].sort_by(|left, right| {
        route_target(right)
            .partial_cmp(&route_target(left))
            .unwrap_or(Ordering::Equal)
    });
    true
}

pub(super) fn compose_fiat_route(
    query: &NormalizedRouteQuery,
    asset: &str,
    entry: &P2pOffer,
    exit: &P2pOffer,
    acquired_asset: f64,
    target_amount: f64,
) -> Option<P2pRoute> {
    if !offer_networks_compatible(entry, exit) {
        return None;
    }
    let same_venue = entry.source == exit.source;
    if !same_venue && !query.allow_cross_venue {
        return None;
    }
    if positive_number(&exit.available_asset).is_none_or(|available| available < acquired_asset)
        || !covers_target(exit, target_amount)
    {
        return None;
    }
    let mut warnings = vec![
        "Search estimate only: platform fees, account eligibility and execution are not verified."
            .into(),
    ];
    if !same_venue {
        warnings.push(
            "Cross-venue route requires an asset transfer; network fee and compatible network are not included."
                .into(),
        );
    }
    let entry_method_match = query
        .source_payment_method
        .as_deref()
        .map(|method| entry.payment_method_match(method));
    let exit_method_match = query
        .target_payment_method
        .as_deref()
        .map(|method| exit.payment_method_match(method));
    if entry_method_match == Some(PaymentMethodMatch::Unknown) {
        warnings.push(
            "The selected sender bank could not be verified because the venue returned an opaque payment-method ID."
                .into(),
        );
    }
    if exit_method_match == Some(PaymentMethodMatch::Unknown) {
        warnings.push(
            "The selected recipient bank could not be verified because the venue returned an opaque payment-method ID."
                .into(),
        );
    }
    if !entry.source_url_is_exact || !exit.source_url_is_exact {
        warnings.push(
            "At least one selected venue does not expose a public deep-link for this advertisement. Verify the advertiser ID and terms on the venue before sending money."
                .into(),
        );
    }
    let payment_methods_verified = entry_method_match
        .is_none_or(|matched| matched == PaymentMethodMatch::Exact)
        && exit_method_match.is_none_or(|matched| matched == PaymentMethodMatch::Exact);
    Some(P2pRoute {
        route_id: String::new(),
        rank: 0,
        asset: asset.into(),
        entry_network: entry.network.clone().or_else(|| exit.network.clone()),
        source_network: query.source_network.clone(),
        target_network: query.target_network.clone(),
        source_fiat: query.source_currency.clone(),
        source_amount: fixed(query.source_amount, 2),
        acquired_asset_amount: fixed(acquired_asset, 8),
        target_fiat: query.target_currency.clone(),
        target_amount: fixed(target_amount, 2),
        effective_rate: fixed(target_amount / query.source_amount, 8),
        same_venue,
        requires_asset_transfer: !same_venue,
        transfer_fee_included: same_venue,
        route_kind: "fiat_to_fiat".into(),
        bridge_currency: None,
        market_path: None,
        route_provider: None,
        route_provider_url: None,
        provider_quote_id: None,
        route_path: Vec::new(),
        route_fees: Vec::new(),
        quote_expires_at: None,
        payment_methods_verified,
        entry_offer: Some(entry.clone()),
        exit_offer: Some(exit.clone()),
        warnings,
        services: Vec::new(),
        reputation: None,
        feedback: None,
        service_links: Vec::new(),
    })
}

pub(super) fn compose_fiat_to_crypto_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    offers: &[P2pOffer],
) {
    for offer in offers {
        if !offer_matches_network(offer, query.target_network.as_deref()) {
            continue;
        }
        let Some(price) = positive_number(&offer.price) else {
            continue;
        };
        let target_amount = query.source_amount / price;
        if positive_number(&offer.available_asset).is_none_or(|available| available < target_amount)
        {
            continue;
        }
        let payment_methods_verified = query
            .source_payment_method
            .as_deref()
            .is_none_or(|method| offer.payment_method_match(method) == PaymentMethodMatch::Exact);
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: asset.into(),
            entry_network: query
                .target_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_network: query.source_network.clone(),
            target_network: query
                .target_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_fiat: query.source_currency.clone(),
            source_amount: fixed(query.source_amount, 8),
            acquired_asset_amount: fixed(target_amount, 8),
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 8),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_crypto".into(),
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified,
            entry_offer: Some(offer.clone()),
            exit_offer: None,
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

pub(super) fn compose_crypto_to_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    offers: &[P2pOffer],
) {
    for offer in offers {
        if !offer_matches_network(offer, query.source_network.as_deref()) {
            continue;
        }
        let Some(price) = positive_number(&offer.price) else {
            continue;
        };
        if positive_number(&offer.available_asset)
            .is_none_or(|available| available < query.source_amount)
        {
            continue;
        }
        let target_amount = query.source_amount * price;
        if !covers_target(offer, target_amount) {
            continue;
        }
        let payment_methods_verified = query
            .target_payment_method
            .as_deref()
            .is_none_or(|method| offer.payment_method_match(method) == PaymentMethodMatch::Exact);
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: asset.into(),
            entry_network: query
                .source_network
                .clone()
                .or_else(|| offer.network.clone()),
            source_network: query
                .source_network
                .clone()
                .or_else(|| offer.network.clone()),
            target_network: query.target_network.clone(),
            source_fiat: query.source_currency.clone(),
            source_amount: fixed(query.source_amount, 8),
            acquired_asset_amount: fixed(query.source_amount, 8),
            target_fiat: query.target_currency.clone(),
            target_amount: fixed(target_amount, 2),
            effective_rate: fixed(target_amount / query.source_amount, 8),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat".into(),
            bridge_currency: None,
            market_path: None,
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified,
            entry_offer: None,
            exit_offer: Some(offer.clone()),
            warnings: vec![
                "Search estimate only: platform fees, account eligibility and execution are not verified.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

pub(super) fn offer_matches_network(offer: &P2pOffer, requested: Option<&str>) -> bool {
    offer
        .network
        .as_deref()
        .zip(requested)
        .is_none_or(|(actual, requested)| {
            canonical_network_id(actual) == canonical_network_id(requested)
        })
}

pub(super) fn offer_networks_compatible(entry: &P2pOffer, exit: &P2pOffer) -> bool {
    entry
        .network
        .as_deref()
        .zip(exit.network.as_deref())
        .is_none_or(|(entry, exit)| canonical_network_id(entry) == canonical_network_id(exit))
}

pub(super) fn compose_crypto_market_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    venue: &str,
    source_asset: &str,
    target_asset: &str,
    tickers: &[CryptoTicker],
) {
    const SPOT_FEE_RATE: f64 = 0.001;
    let intermediaries = std::iter::once(None).chain(
        query
            .assets
            .iter()
            .map(String::as_str)
            .filter(|asset| !asset.eq_ignore_ascii_case(source_asset))
            .filter(|asset| !asset.eq_ignore_ascii_case(target_asset))
            .map(Some),
    );

    for intermediary in intermediaries {
        let first = match intermediary {
            Some(asset) => conversion_quote(tickers, source_asset, asset),
            None => conversion_quote(tickers, source_asset, target_asset),
        };
        let second = intermediary.and_then(|asset| conversion_quote(tickers, asset, target_asset));
        let Some(first) = first else {
            continue;
        };
        if intermediary.is_some() && second.is_none() {
            continue;
        }

        let intermediary_amount = query.source_amount * first.rate * (1.0 - SPOT_FEE_RATE);
        let second_rate = second.as_ref().map(|quote| quote.rate).unwrap_or(1.0);
        let target_pair = second
            .as_ref()
            .map(|quote| quote.pair.clone())
            .unwrap_or_else(|| first.pair.clone());
        let target_amount = match second {
            Some(second) => intermediary_amount * second.rate * (1.0 - SPOT_FEE_RATE),
            None => intermediary_amount,
        };
        if !target_amount.is_finite() || target_amount <= 0.0 {
            continue;
        }

        let bridge_currency = intermediary.map(str::to_owned);
        let path = CryptoMarketPath {
            venue: venue.into(),
            source_pair: first.pair,
            target_pair,
            source_rate: fixed(first.rate, 12),
            target_rate: fixed(second_rate, 12),
            intermediary_amount: fixed(intermediary_amount, 12),
        };
        routes.push(P2pRoute {
            route_id: String::new(),
            rank: 0,
            asset: target_asset.into(),
            entry_network: query.source_network.clone(),
            source_network: query.source_network.clone(),
            target_network: query.target_network.clone(),
            source_fiat: source_asset.into(),
            source_amount: fixed(query.source_amount, 12),
            acquired_asset_amount: fixed(target_amount, 12),
            target_fiat: target_asset.into(),
            target_amount: fixed(target_amount, 12),
            effective_rate: fixed(target_amount / query.source_amount, 12),
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto".into(),
            bridge_currency,
            market_path: Some(path),
            route_provider: None,
            route_provider_url: None,
            provider_quote_id: None,
            route_path: Vec::new(),
            route_fees: Vec::new(),
            quote_expires_at: None,
            payment_methods_verified: true,
            entry_offer: None,
            exit_offer: None,
            warnings: vec![
                "Spot-market estimate only: trading fees, slippage and execution are not guaranteed.".into(),
                "Deposit and withdrawal network availability and fees are not verified by the selected venue.".into(),
            ],
            services: Vec::new(),
            reputation: None,
            feedback: None,
            service_links: Vec::new(),
        });
    }
}

pub(super) struct ConversionQuote {
    pair: String,
    rate: f64,
}

pub(super) fn conversion_quote(
    tickers: &[CryptoTicker],
    from: &str,
    to: &str,
) -> Option<ConversionQuote> {
    let direct = format!("{}{}", from.to_ascii_uppercase(), to.to_ascii_uppercase());
    if let Some(ticker) = tickers
        .iter()
        .find(|ticker| ticker.symbol.eq_ignore_ascii_case(&direct))
    {
        return Some(ConversionQuote {
            pair: ticker.symbol.clone(),
            rate: ticker.bid,
        });
    }

    let inverse = format!("{}{}", to.to_ascii_uppercase(), from.to_ascii_uppercase());
    tickers
        .iter()
        .find(|ticker| ticker.symbol.eq_ignore_ascii_case(&inverse))
        .map(|ticker| ConversionQuote {
            pair: ticker.symbol.clone(),
            rate: 1.0 / ticker.ask,
        })
}

pub(super) fn upsert_asset_status(
    statuses: &mut Vec<RouteAssetStatus>,
    asset: String,
    entry_sources: &[SourceStatus],
    exit_sources: &[SourceStatus],
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
    routes_built: usize,
    routes_exhaustive: bool,
    entry_discovery_source: Option<&str>,
    exit_discovery_source: Option<&str>,
) {
    let status = RouteAssetStatus {
        asset,
        entry_offers: entry_offers.len(),
        exit_offers: exit_offers.len(),
        routes_built,
        routes_exhaustive,
        can_exchange_to_target: routes_built > 0,
        entry_sources: entry_sources.to_vec(),
        exit_sources: exit_sources.to_vec(),
        entry_discovery_source: entry_discovery_source.map(str::to_owned),
        exit_discovery_source: exit_discovery_source.map(str::to_owned),
    };
    if let Some(current) = statuses
        .iter_mut()
        .find(|current| current.asset == status.asset)
    {
        *current = status;
    } else {
        statuses.push(status);
    }
}

pub(super) fn positive_number(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(super) fn covers_target(offer: &P2pOffer, target_amount: f64) -> bool {
    let Some(minimum) = positive_number(&offer.min_fiat) else {
        return false;
    };
    let Some(maximum) = positive_number(&offer.max_fiat) else {
        return false;
    };
    minimum <= target_amount && target_amount <= maximum
}

pub(super) fn route_target(route: &P2pRoute) -> f64 {
    route.target_amount.parse().unwrap_or_default()
}

pub(super) fn fixed(value: f64, scale: usize) -> String {
    format!("{value:.scale$}")
}

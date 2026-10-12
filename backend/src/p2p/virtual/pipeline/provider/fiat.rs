//! Fiat workflow route production.

use super::*;

impl P2pSearchService {
    pub(in crate::p2p) async fn search_crypto_to_fiat_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
        batches: Option<&mpsc::Sender<RouteBatch>>,
    ) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let source_assets = self.assets_on_network(
            &query.source_currency,
            query.source_network.as_deref(),
            &provider_assets,
        );
        if source_assets.is_empty() {
            return Vec::new();
        }
        let mut exit_searches = FuturesUnordered::new();
        for intermediary in self.intermediary_assets(query).await {
            let service = self.clone();
            let query = query.clone();
            exit_searches.push(async move {
                let exit = service
                    .search_market(
                        leg_query(
                            &query.target_currency,
                            &intermediary.symbol,
                            P2pSide::SellCrypto,
                            None,
                            query.target_payment_method.clone(),
                            &query,
                        ),
                        query.offer_market(),
                    )
                    .await;
                (intermediary, exit)
            });
        }
        let mut searches = FuturesUnordered::new();
        let mut routes = Vec::new();
        while !exit_searches.is_empty() || !searches.is_empty() {
            tokio::select! {
            exit = exit_searches.next(), if !exit_searches.is_empty() => {
            let Some((intermediary, exit)) = exit else {
                continue;
            };
            let Ok(exit) = exit else {
                continue;
            };
            let exit_offers = reject_price_outliers(
                exit.offers
                    .into_iter()
                    .filter(|offer| query.accepts_offer(offer))
                    .collect(),
                query.max_price_deviation_bps,
            )
                .into_iter()
                .filter(|offer| offer_matches_network(offer, intermediary.location.as_deref()))
                .take(MAX_PROVIDER_OFFERS_PER_LEG)
                .collect::<Vec<_>>();
            for source in &source_assets {
                if *source == intermediary {
                    continue;
                }
                let Ok(amount) = Amount::from_f64(query.source_amount, source.clone()) else {
                    continue;
                };
                for capability in capabilities
                    .iter()
                    .filter(|capability| capability.supports(source, &intermediary))
                {
                    let provider = capability.provider.clone();
                    let source = source.clone();
                    let intermediary = intermediary.clone();
                    let amount = amount.clone();
                    let exit_offers = exit_offers.clone();
                    let source_currency = query.source_currency.clone();
                    let target_currency = query.target_currency.clone();
                    let source_amount = query.source_amount;
                    let target_payment_method = query.target_payment_method.clone();
                    let quote_semaphore = self.quote_semaphore.clone();
                    searches.push(async move {
                        quote_provider(
                            provider,
                            source,
                            intermediary,
                            amount,
                            quote_semaphore,
                        )
                        .await
                        .into_iter()
                        .flat_map(|(provider, quote)| {
                        let source_currency = source_currency.clone();
                        let target_currency = target_currency.clone();
                        let target_payment_method = target_payment_method.clone();
                        exit_offers.iter().filter_map(move |offer| {
                            let price = positive_number(&offer.price)?;
                            let output = quote.output.value.parse::<f64>().ok()?;
                            let target_amount = output * price;
                            if !output.is_finite()
                                || output <= 0.0
                                || !covers_target(offer, target_amount)
                                || positive_number(&offer.available_asset)
                                    .is_none_or(|available| available < output)
                            {
                                return None;
                            }
                            let payment_methods_verified = target_payment_method
                                .as_deref()
                                .is_none_or(|method| {
                                    offer.payment_method_match(method)
                                        == PaymentMethodMatch::Exact
                                });
                            let warnings = vec![
                                format!("Live dry quote from {provider}; execution and wallet compatibility are not verified."),
                                "Fiat exit is a live P2P estimate; confirm payment details before sending.".into(),
                            ];
                            let route_quote = quote.clone();
                            Some(P2pRoute {
                                route_id: String::new(),
                                rank: 0,
                                asset: route_quote.from.symbol.clone(),
                                entry_network: route_quote.from.location.clone(),
                                source_network: route_quote.from.location.clone(),
                                target_network: route_quote.to.location.clone(),
                                source_fiat: source_currency.clone(),
                                source_amount: fixed(source_amount, 8),
                                acquired_asset_amount: route_quote.output.value.clone(),
                                provider_input_amount: Some(route_quote.input.value.clone()),
                                target_fiat: target_currency.clone(),
                                target_amount: fixed(target_amount, 2),
                                effective_rate: fixed(target_amount / source_amount, 12),
                                same_venue: false,
                                requires_asset_transfer: true,
                                transfer_fee_included: !route_quote.fees.is_empty(),
                                route_kind: "crypto_to_fiat".into(),
                                profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
                                bridge_currency: None,
                                market_path: None,
                                route_provider: Some(provider.clone()),
                                route_provider_url: route_quote.source_url.clone(),
                                provider_quote_id: route_quote.quote_id.clone(),
                                route_path: route_quote
                                    .path
                                    .into_iter()
                                    .map(|asset| asset.to_string())
                                    .chain(std::iter::once(target_currency.clone()))
                                    .collect(),
                                route_fees: route_quote
                                    .fees
                                    .into_iter()
                                    .map(|fee| RouteFee {
                                        asset: fee.asset.to_string(),
                                        amount: fee.value,
                                    })
                                    .collect(),
                                quote_expires_at: route_quote.expires_at,
                                execution: None,
                                payment_methods_verified,
                                entry_offer: None,
                                exit_offer: Some(offer.clone()),
                                warnings,
                                services: Vec::new(),
                                reputation: None,
                                feedback: None,
                                service_links: Vec::new(),
                            })
                        })
                        })
                        .collect::<Vec<_>>()
                    });
                }
            }
            }
            batch = searches.next(), if !searches.is_empty() => {
            let Some(batch) = batch else {
                continue;
            };
            emit_routes(batches, &batch).await;
            routes.extend(batch);
            }
            }
        }
        routes
    }

    pub(in crate::p2p) async fn search_fiat_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
        batches: Option<&mpsc::Sender<RouteBatch>>,
    ) -> (Vec<P2pRoute>, bool) {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let mut symbols = if query.assets_explicit {
            query.assets.clone()
        } else {
            provider_assets
                .iter()
                .map(|asset| asset.symbol.clone())
                .collect::<Vec<_>>()
        };
        symbols.sort_by_key(|symbol| {
            (
                intermediary_asset_priority(symbol, &query.target_currency),
                symbol.clone(),
            )
        });
        symbols.dedup();
        symbols.truncate(MAX_PROVIDER_ASSETS);

        // Discover each fiat leg once, concurrently, before joining token pairs.
        let mut legs = FuturesUnordered::new();
        for symbol in &symbols {
            for (fiat, side, amount, method) in [
                (
                    &query.source_currency,
                    P2pSide::BuyCrypto,
                    Some(query.source_amount),
                    &query.source_payment_method,
                ),
                (
                    &query.target_currency,
                    P2pSide::SellCrypto,
                    None,
                    &query.target_payment_method,
                ),
            ] {
                let request = leg_query(fiat, symbol, side, amount, method.clone(), query);
                let symbol = symbol.clone();
                legs.push(async move {
                    (
                        symbol,
                        side,
                        self.search_market(request, query.offer_market()).await,
                    )
                });
            }
        }
        let mut entries = HashMap::new();
        let mut exits = HashMap::new();
        while let Some((symbol, side, response)) = legs.next().await {
            let Ok(response) = response else {
                continue;
            };
            let offers = reject_price_outliers(
                response
                    .offers
                    .into_iter()
                    .filter(|offer| query.accepts_offer(offer))
                    .collect(),
                query.max_price_deviation_bps,
            );
            match side {
                P2pSide::BuyCrypto => {
                    entries.insert(symbol, offers);
                }
                P2pSide::SellCrypto => {
                    exits.insert(symbol, offers);
                }
            }
        }
        let mut jobs_by_provider = Vec::new();
        for capability in capabilities.iter() {
            let mut jobs = Vec::new();
            let mut pairs = capability
                .assets
                .iter()
                .flat_map(|from| capability.assets.iter().map(move |to| (from, to)))
                .filter(|(from, to)| {
                    from != to
                        && capability.supports(from, to)
                        && symbols.contains(&from.symbol)
                        && symbols.contains(&to.symbol)
                })
                .collect::<Vec<_>>();
            pairs.sort_by_key(|(from, to)| {
                (
                    intermediary_asset_priority(&from.symbol, ""),
                    intermediary_asset_priority(&to.symbol, ""),
                    network_priority(from.location.as_deref()),
                    network_priority(to.location.as_deref()),
                    from.to_string(),
                    to.to_string(),
                )
            });
            for (from, to) in pairs {
                let Some(entry_offers) = entries.get(&from.symbol) else {
                    continue;
                };
                let Some(exit_offers) = exits.get(&to.symbol) else {
                    continue;
                };
                let exit_offers = diverse_provider_offers(exit_offers, to.location.as_deref());
                if exit_offers.is_empty() {
                    continue;
                }
                let exit_offers = Arc::<[P2pOffer]>::from(exit_offers);
                for entry_offer in diverse_provider_offers(entry_offers, from.location.as_deref()) {
                    let Some(price) = positive_number(&entry_offer.price) else {
                        continue;
                    };
                    let acquired = query.source_amount / price;
                    if !covers_target(&entry_offer, query.source_amount)
                        || positive_number(&entry_offer.available_asset)
                            .is_none_or(|available| available < acquired)
                    {
                        continue;
                    }
                    let Ok(amount) = Amount::from_f64(acquired, from.clone()) else {
                        continue;
                    };
                    jobs.push(FiatProviderQuoteJob {
                        provider: capability.provider.clone(),
                        from: from.clone(),
                        to: to.clone(),
                        amount,
                        entry_offer,
                        exit_offers: exit_offers.clone(),
                    });
                }
            }
            // Interleave asset/network pairs before requesting additional entry offers.
            let mut groups = Vec::<std::collections::VecDeque<FiatProviderQuoteJob>>::new();
            for job in jobs {
                if let Some(group) = groups.last_mut().filter(|group| {
                    group
                        .front()
                        .is_some_and(|first| first.from == job.from && first.to == job.to)
                }) {
                    group.push_back(job);
                } else {
                    groups.push(std::collections::VecDeque::from([job]));
                }
            }
            let mut pair_jobs = Vec::new();
            while groups.iter().any(|group| !group.is_empty()) {
                for group in &mut groups {
                    if let Some(job) = group.pop_front() {
                        pair_jobs.push(job);
                    }
                }
            }
            // A token with many networks must not consume every slot before other symbols.
            let mut symbol_groups = Vec::<std::collections::VecDeque<FiatProviderQuoteJob>>::new();
            for job in pair_jobs {
                if let Some(group) = symbol_groups.iter_mut().find(|group| {
                    group.front().is_some_and(|first| {
                        first.from.symbol == job.from.symbol && first.to.symbol == job.to.symbol
                    })
                }) {
                    group.push_back(job);
                } else {
                    symbol_groups.push(std::collections::VecDeque::from([job]));
                }
            }
            let mut jobs = Vec::new();
            while symbol_groups.iter().any(|group| !group.is_empty()) {
                for group in &mut symbol_groups {
                    if let Some(job) = group.pop_front() {
                        jobs.push(job);
                    }
                }
            }
            jobs_by_provider.push((capability.provider.name().to_string(), jobs));
        }
        let quote_jobs_total = jobs_by_provider
            .iter()
            .map(|(_, jobs)| jobs.len())
            .sum::<usize>();
        let quote_started = Instant::now();
        let mut searches = FuturesUnordered::new();
        // Every provider gets its own budget, independent of the display limit.
        for (provider_name, jobs) in jobs_by_provider {
            searches.push(async move {
                let mut results = Vec::new();
                let total = jobs.len();
                let mut attempted = 0;
                let mut failed = 0;
                let mut budget_expired = false;
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(45);
                for job in jobs.into_iter().take(MAX_PROVIDER_NETWORK_PAIRS) {
                    attempted += 1;
                    let key = provider_quote_key(&provider_name, &job.from, &job.to, &job.amount);
                    let cached = self.cached_provider_quote(&key);
                    let quote = if let Some(quote) = cached {
                        Some(quote)
                    } else {
                        match tokio::time::timeout_at(
                            deadline,
                            quote_provider(
                                job.provider,
                                job.from.clone(),
                                job.to.clone(),
                                job.amount.clone(),
                                self.quote_semaphore.clone(),
                            ),
                        )
                        .await
                        {
                            Ok(result) => result.map(|(_, quote)| quote),
                            Err(_) => {
                                budget_expired = true;
                                break;
                            }
                        }
                    };
                    if let Some(quote) = quote.filter(|quote| {
                        quote.from == job.from
                            && quote.to == job.to
                            && quote.input == job.amount
                            && quote.output.asset == job.to
                            && quote
                                .expires_at
                                .is_none_or(|expires| expires > chrono::Utc::now())
                    }) {
                        if let Ok(mut cache) = self.provider_quote_cache.write() {
                            cache.retain(|_, cached| {
                                cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL
                            });
                            cache.insert(
                                key,
                                crate::p2p::service::CachedProviderQuote {
                                    inserted_at: Instant::now(),
                                    quote: quote.clone(),
                                },
                            );
                        }
                        let quoted = (
                            provider_name.clone(),
                            quote,
                            job.entry_offer,
                            job.exit_offers,
                        );
                        let batch = compose_fiat_provider_quotes(query, vec![quoted.clone()]);
                        emit_routes(batches, &batch).await;
                        results.push(quoted);
                    } else {
                        failed += 1;
                    }
                }
                (
                    provider_name,
                    results,
                    attempted >= total && !budget_expired,
                    failed,
                    budget_expired,
                )
            });
        }
        let mut routes = Vec::new();
        let mut exhaustive = true;
        while let Some((provider, results, complete, failed, budget_expired)) =
            searches.next().await
        {
            exhaustive &= complete;
            if let Some(batches) = batches {
                send_batch(
                    batches,
                    RouteBatch::ProviderResult {
                        status: SourceStatus {
                            source: provider,
                            ok: failed == 0 && !budget_expired,
                            cached: false,
                            latency_ms: quote_started.elapsed().as_millis(),
                            offers_found: results.len(),
                            error: if budget_expired {
                                Some("Provider quote search budget expired".into())
                            } else if failed > 0 {
                                Some(format!("{failed} provider quotes failed or were invalid"))
                            } else {
                                None
                            },
                        },
                        routes: Vec::new(),
                        exhaustive: complete,
                    },
                )
                .await;
            }
            routes.extend(compose_fiat_provider_quotes(query, results));
        }
        tracing::info!(
            quote_jobs_total,
            routes_found = routes.len(),
            elapsed_ms = quote_started.elapsed().as_millis(),
            "p2p.provider_route_search.completed"
        );
        (routes, exhaustive)
    }
}

// Reserve an offer per venue before filling the remaining quote slots by price.
fn diverse_provider_offers(offers: &[P2pOffer], network: Option<&str>) -> Vec<P2pOffer> {
    let matching = offers
        .iter()
        .filter(|offer| offer_matches_network(offer, network))
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut venues = std::collections::HashSet::new();
    for (index, offer) in matching.iter().enumerate() {
        if venues.insert(offer.source.as_str()) {
            selected.push(index);
        }
    }
    selected.truncate(MAX_PROVIDER_OFFERS_PER_LEG);
    for index in 0..matching.len() {
        if selected.len() >= MAX_PROVIDER_OFFERS_PER_LEG {
            break;
        }
        if !selected.contains(&index) {
            selected.push(index);
        }
    }
    selected
        .into_iter()
        .map(|index| matching[index].clone())
        .collect()
}

type FiatProviderQuote = (String, PublicRouteQuote, P2pOffer, Arc<[P2pOffer]>);

fn compose_fiat_provider_quotes(
    query: &NormalizedRouteQuery,
    quote_results: Vec<FiatProviderQuote>,
) -> Vec<P2pRoute> {
    let mut routes = Vec::new();
    for (provider_name, quote, entry, exit_offers) in quote_results {
        let Ok(output_asset) = quote.output.value.parse::<f64>() else {
            continue;
        };
        if !output_asset.is_finite() || output_asset <= 0.0 {
            continue;
        }
        let mut batch = Vec::new();
        for exit in exit_offers.iter() {
            let Some(exit_price) = positive_number(&exit.price) else {
                continue;
            };
            let target_amount = output_asset * exit_price;
            if !covers_target(exit, target_amount)
                || positive_number(&exit.available_asset)
                    .is_none_or(|available| available < output_asset)
            {
                continue;
            }
            let route_quote = quote.clone();
            batch.push(P2pRoute {
                    route_id: String::new(),
                    rank: 0,
                    asset: exit.asset.clone(),
                    entry_network: route_quote.from.location.clone(),
                    source_network: route_quote.from.location.clone(),
                    target_network: route_quote.to.location.clone(),
                    source_fiat: query.source_currency.clone(),
                    source_amount: fixed(query.source_amount, 2),
                    acquired_asset_amount: fixed(output_asset, 8),
                    provider_input_amount: Some(route_quote.input.value.clone()),
                    target_fiat: query.target_currency.clone(),
                    target_amount: fixed(target_amount, 2),
                    effective_rate: fixed(target_amount / query.source_amount, 8),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: false,
                    route_kind: if query.source_currency == query.target_currency {
                        "crypto_cycle".into()
                    } else {
                        "fiat_to_fiat".into()
                    },
                    profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
                    bridge_currency: None,
                    market_path: None,
                    route_provider: Some(provider_name.clone()),
                    route_provider_url: route_quote.source_url.clone(),
                    provider_quote_id: route_quote.quote_id.clone(),
                    route_path: std::iter::once(query.source_currency.clone())
                        .chain(route_quote.path.into_iter().map(|asset| asset.to_string()))
                        .chain(std::iter::once(query.target_currency.clone()))
                        .collect(),
                    route_fees: route_quote
                        .fees
                        .into_iter()
                        .map(|fee| RouteFee {
                            asset: fee.asset.to_string(),
                            amount: fee.value,
                        })
                        .collect(),
                    quote_expires_at: route_quote.expires_at,
                    execution: None,
                    payment_methods_verified: query.source_payment_method.as_deref().is_none_or(|method|
                        entry.payment_method_match(method) == PaymentMethodMatch::Exact)
                        && query.target_payment_method.as_deref().is_none_or(|method|
                            exit.payment_method_match(method) == PaymentMethodMatch::Exact),
                    entry_offer: Some(entry.clone()),
                    exit_offer: Some(exit.clone()),
                    warnings: vec![
                        "Recent fiat entry/exit offers plus a live dry provider quote; refresh each step before execution.".into(),
                        "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.".into(),
                    ],
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
        }
        routes.extend(batch);
    }
    routes
}

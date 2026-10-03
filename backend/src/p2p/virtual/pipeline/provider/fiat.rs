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
            let exit_offers = exit
                .offers
                .into_iter()
                .filter(|offer| query.accepts_offer(offer))
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
                                target_fiat: target_currency.clone(),
                                target_amount: fixed(target_amount, 2),
                                effective_rate: fixed(target_amount / source_amount, 12),
                                same_venue: false,
                                requires_asset_transfer: true,
                                transfer_fee_included: !route_quote.fees.is_empty(),
                                route_kind: "crypto_to_fiat".into(),
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
        let mut intermediary_symbols = if query.assets_explicit {
            query.assets.clone()
        } else {
            provider_assets
                .iter()
                .map(|asset| asset.symbol.clone())
                .collect::<Vec<_>>()
        };
        intermediary_symbols
            .sort_by_key(|symbol| intermediary_asset_priority(symbol, &query.target_currency));
        intermediary_symbols.dedup();
        intermediary_symbols.truncate(MAX_PROVIDER_ASSETS);
        let mut asset_searches = intermediary_symbols
            .into_iter()
            .map(|asset_symbol| {
                let provider_assets = &provider_assets;
                let capabilities = &capabilities;
                async move {
                    let mut quote_jobs = Vec::new();
                    let source_networks = self.assets_on_network(
                        &asset_symbol,
                        query.source_network.as_deref(),
                        provider_assets,
                    );
                    let target_networks = self.assets_on_network(
                        &asset_symbol,
                        query.target_network.as_deref(),
                        provider_assets,
                    );
                    if source_networks.is_empty() || target_networks.is_empty() {
                        return quote_jobs;
                    }
                    let mut network_pairs = source_networks
                        .iter()
                        .flat_map(|source| {
                            target_networks
                                .iter()
                                .filter(move |target| source.location != target.location)
                                .map(move |target| (source.clone(), target.clone()))
                        })
                        .collect::<Vec<_>>();
                    network_pairs.sort_by_key(|(from, to)| {
                        (
                            network_priority(from.location.as_deref()),
                            network_priority(to.location.as_deref()),
                            from.to_string(),
                            to.to_string(),
                        )
                    });
                    if network_pairs.is_empty() {
                        return quote_jobs;
                    }
                    let entry_search = self.search_market(
                        leg_query(
                            &query.source_currency,
                            &asset_symbol,
                            P2pSide::BuyCrypto,
                            Some(query.source_amount),
                            query.source_payment_method.clone(),
                            query,
                        ),
                        query.offer_market(),
                    );
                    let exit_search = self.search_market(
                        leg_query(
                            &query.target_currency,
                            &asset_symbol,
                            P2pSide::SellCrypto,
                            None,
                            query.target_payment_method.clone(),
                            query,
                        ),
                        query.offer_market(),
                    );
                    tokio::pin!(entry_search, exit_search);
                    let (entry, exit) = tokio::select! {
                        entry = &mut entry_search => {
                            let Ok(entry) = entry else { return quote_jobs };
                            if entry.offers.is_empty() { return quote_jobs }
                            (Ok(entry), exit_search.await)
                        }
                        exit = &mut exit_search => {
                            let Ok(exit) = exit else { return quote_jobs };
                            if exit.offers.is_empty() { return quote_jobs }
                            (entry_search.await, Ok(exit))
                        }
                    };
                    let (Ok(entry), Ok(exit)) = (entry, exit) else {
                        return quote_jobs;
                    };
                    let exit_offers = Arc::<[P2pOffer]>::from(
                        exit.offers
                            .into_iter()
                            .filter(|offer| query.accepts_offer(offer))
                            .take(MAX_PROVIDER_OFFERS_PER_LEG)
                            .collect::<Vec<_>>(),
                    );
                    if exit_offers.is_empty() {
                        return quote_jobs;
                    }
                    for entry_offer in entry
                        .offers
                        .into_iter()
                        .filter(|offer| query.accepts_offer(offer))
                        .take(2)
                    {
                        let Some(entry_price) = positive_number(&entry_offer.price) else {
                            continue;
                        };
                        let source_amount = query.source_amount / entry_price;
                        if positive_number(&entry_offer.available_asset)
                            .is_none_or(|available| available < source_amount)
                        {
                            continue;
                        }
                        for (from, to) in network_pairs.iter().take(MAX_PROVIDER_NETWORK_PAIRS) {
                            for capability in capabilities
                                .iter()
                                .filter(|capability| capability.supports(from, to))
                            {
                                let from = from.clone();
                                let to = to.clone();
                                let Ok(amount) = Amount::new(fixed(source_amount, 6), from.clone())
                                else {
                                    continue;
                                };
                                let entry_offer = entry_offer.clone();
                                let exit_offers = exit_offers.clone();
                                let provider = capability.provider.clone();
                                quote_jobs.push(FiatProviderQuoteJob {
                                    provider,
                                    from,
                                    to,
                                    amount,
                                    entry_offer,
                                    exit_offers,
                                });
                            }
                        }
                    }
                    quote_jobs
                }
            })
            .collect::<FuturesUnordered<_>>();

        let quote_started = Instant::now();
        let mut quote_jobs_total = 0;
        let mut jobs_processed = 0;
        let mut refreshes_started = 0;
        let mut routes = Vec::new();
        let provider_route_limit = query.limit.div_ceil(2).max(capabilities.len());
        while let Some(quote_jobs) = asset_searches.next().await {
            quote_jobs_total += quote_jobs.len();
            let mut quote_results = Vec::new();
            for job in quote_jobs
                .into_iter()
                .take(query.limit.saturating_sub(jobs_processed))
            {
                jobs_processed += 1;
                let provider_name = job.provider.name().to_string();
                let key = provider_quote_key(&provider_name, &job.from, &job.to, &job.amount);
                if let Some(quote) = self.cached_provider_quote(&key) {
                    quote_results.push((
                        provider_name,
                        quote,
                        job.entry_offer,
                        job.exit_offers,
                        true,
                    ));
                } else if !self.has_fmatch_backend() {
                    if let Some((provider_name, quote)) = quote_provider(
                        job.provider,
                        job.from,
                        job.to,
                        job.amount,
                        self.quote_semaphore.clone(),
                    )
                    .await
                    {
                        quote_results.push((
                            provider_name,
                            quote,
                            job.entry_offer,
                            job.exit_offers,
                            true,
                        ));
                    }
                } else {
                    if refreshes_started < MAX_BACKGROUND_PROVIDER_REFRESHES_PER_SEARCH {
                        self.refresh_provider_quote(key, &job);
                        refreshes_started += 1;
                    }
                    let output = Amount::new(job.amount.value.clone(), job.to.clone())
                        .expect("provider quote job contains a validated amount");
                    quote_results.push((
                        provider_name.clone(),
                        PublicRouteQuote {
                            provider: provider_name,
                            quote_id: None,
                            description: Some("capability snapshot estimate".into()),
                            source_url: None,
                            from: job.from.clone(),
                            to: job.to.clone(),
                            input: job.amount.clone(),
                            output,
                            fees: Vec::new(),
                            expires_at: None,
                            path: vec![job.from, job.to],
                        },
                        job.entry_offer,
                        job.exit_offers,
                        false,
                    ));
                }
            }
            for (provider_name, quote, entry, exit_offers, quote_confirmed) in quote_results {
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
                    target_fiat: query.target_currency.clone(),
                    target_amount: fixed(target_amount, 2),
                    effective_rate: fixed(target_amount / query.source_amount, 8),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: !route_quote.fees.is_empty(),
                    route_kind: "fiat_to_fiat".into(),
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
                    payment_methods_verified: true,
                    entry_offer: Some(entry.clone()),
                    exit_offer: Some(exit.clone()),
                    warnings: vec![
                        if quote_confirmed {
                            "Recent fiat entry/exit offers plus a cached dry cross-network quote; execution still requires a fresh quote.".into()
                        } else {
                            "Capability-based cross-network estimate; provider fees and live output are refreshed asynchronously and must be confirmed before execution.".into()
                        },
                        "Confirm the source and destination networks, provider deposit address, memo/tag, network fee, and finality before sending.".into(),
                    ],
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
                }
                emit_routes(batches, &batch).await;
                routes.extend(batch);
                if routes.len() >= provider_route_limit {
                    break;
                }
            }
            if routes.len() >= provider_route_limit || jobs_processed >= query.limit {
                break;
            }
        }
        tracing::info!(
            quote_jobs_total,
            refreshes_started,
            routes_found = routes.len(),
            elapsed_ms = quote_started.elapsed().as_millis(),
            "p2p.provider_route_search.completed"
        );
        (routes, false)
    }
}

use super::*;

impl P2pSearchService {
    pub async fn search_routes(
        &self,
        query: P2pRouteSearchQuery,
    ) -> Result<P2pRouteSearchResponse> {
        self.run_route_search(query, Uuid::new_v4(), None).await
    }

    pub(crate) async fn stream_routes(
        &self,
        query: P2pRouteSearchQuery,
        search_id: Uuid,
        updates: mpsc::Sender<P2pRouteSearchResponse>,
    ) -> Result<P2pRouteSearchResponse> {
        self.run_route_search(query, search_id, Some(updates)).await
    }

    async fn run_route_search(
        &self,
        query: P2pRouteSearchQuery,
        search_id: Uuid,
        updates: Option<mpsc::Sender<P2pRouteSearchResponse>>,
    ) -> Result<P2pRouteSearchResponse> {
        let started = Instant::now();
        let provider_assets = self.provider_assets().await;
        let provider_assets_elapsed = started.elapsed();
        let query = normalize_query(
            query,
            &self.default_assets,
            &self.networks,
            &provider_assets,
        )?;
        let mut routes = HashMap::new();
        let mut asset_statuses = Vec::new();
        let source_is_crypto =
            is_crypto_currency(&query.source_currency, &self.networks, &provider_assets);
        let target_is_crypto =
            is_crypto_currency(&query.target_currency, &self.networks, &provider_assets);
        match (source_is_crypto, target_is_crypto) {
            (false, false) => {
                if updates.is_some() {
                    let (progress_updates, mut progress_snapshots) = mpsc::channel(128);
                    let mut searches = query
                        .assets
                        .iter()
                        .map(|asset| {
                            let entry_query = leg_query(
                                &query.source_currency,
                                asset,
                                P2pSide::BuyCrypto,
                                Some(query.source_amount),
                                query.source_payment_method.clone(),
                                &query,
                            );
                            let exit_query = leg_query(
                                &query.target_currency,
                                asset,
                                P2pSide::SellCrypto,
                                None,
                                query.target_payment_method.clone(),
                                &query,
                            );
                            stream_fiat_asset_search(
                                self,
                                asset.clone(),
                                entry_query,
                                exit_query,
                                query.offer_market(),
                                progress_updates.clone(),
                            )
                        })
                        .collect::<FuturesUnordered<_>>();
                    drop(progress_updates);
                    let mut sources_seen = HashMap::new();

                    while !searches.is_empty() {
                        tokio::select! {
                            progress = progress_snapshots.recv() => {
                                let Some(progress) = progress else { continue };
                                let counts = (progress.entry.sources.len(), progress.exit.sources.len());
                                if sources_seen.get(&progress.asset) == Some(&counts) {
                                    continue;
                                }
                                sources_seen.insert(progress.asset.clone(), counts);
                                apply_fiat_asset_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &progress.asset,
                                    &progress.entry,
                                    &progress.exit,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = searches.next() => {
                                let Some(result) = result else { break };
                                let (asset, entry, exit) = result?;
                                let counts = (entry.sources.len(), exit.sources.len());
                                if sources_seen.get(&asset) != Some(&counts) {
                                    sources_seen.insert(asset.clone(), counts);
                                    apply_fiat_asset_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &entry,
                                        &exit,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                        }
                    }
                } else {
                    let mut searches = query
                        .assets
                        .iter()
                        .map(|asset| async {
                            let entry_query = leg_query(
                                &query.source_currency,
                                asset,
                                P2pSide::BuyCrypto,
                                Some(query.source_amount),
                                query.source_payment_method.clone(),
                                &query,
                            );
                            let exit_query = leg_query(
                                &query.target_currency,
                                asset,
                                P2pSide::SellCrypto,
                                None,
                                query.target_payment_method.clone(),
                                &query,
                            );
                            let market = query.offer_market();
                            let (entry, exit) = tokio::join!(
                                self.search_market(entry_query, market),
                                self.search_market(exit_query, market)
                            );
                            (asset.clone(), entry, exit)
                        })
                        .collect::<FuturesUnordered<_>>();

                    while let Some((asset, entry, exit)) = searches.next().await {
                        apply_fiat_asset_response(
                            &mut routes,
                            &mut asset_statuses,
                            &query,
                            &asset,
                            &entry?,
                            &exit?,
                        );
                    }
                }
            }
            (false, true) => {
                let asset = query.target_currency.clone();
                let leg = leg_query(
                    &query.source_currency,
                    &asset,
                    P2pSide::BuyCrypto,
                    Some(query.source_amount),
                    query.source_payment_method.clone(),
                    &query,
                );
                if updates.is_some() {
                    let (leg_updates, mut leg_snapshots) = mpsc::channel(16);
                    let search = self.stream_search_market(leg, leg_updates, query.offer_market());
                    tokio::pin!(search);
                    let mut sources_seen = 0;
                    loop {
                        tokio::select! {
                            response = leg_snapshots.recv() => {
                                let Some(response) = response else { continue };
                                sources_seen = response.sources.len();
                                apply_fiat_to_crypto_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &asset,
                                    &response,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = &mut search => {
                                let response = result?;
                                if response.sources.len() > sources_seen {
                                    apply_fiat_to_crypto_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &response,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                                break;
                            }
                        }
                    }
                } else {
                    let response = self.search_market(leg, query.offer_market()).await?;
                    apply_fiat_to_crypto_response(
                        &mut routes,
                        &mut asset_statuses,
                        &query,
                        &asset,
                        &response,
                    );
                }
            }
            (true, false) => {
                let asset = query.source_currency.clone();
                let leg = leg_query(
                    &query.target_currency,
                    &asset,
                    P2pSide::SellCrypto,
                    None,
                    query.target_payment_method.clone(),
                    &query,
                );
                if updates.is_some() {
                    let (leg_updates, mut leg_snapshots) = mpsc::channel(16);
                    let search = self.stream_search_market(leg, leg_updates, query.offer_market());
                    tokio::pin!(search);
                    let mut sources_seen = 0;
                    loop {
                        tokio::select! {
                            response = leg_snapshots.recv() => {
                                let Some(response) = response else { continue };
                                sources_seen = response.sources.len();
                                apply_crypto_to_fiat_response(
                                    &mut routes,
                                    &mut asset_statuses,
                                    &query,
                                    &asset,
                                    &response,
                                );
                                publish_update(
                                    updates.as_ref(),
                                    response_snapshot(search_id, &query, &routes, &asset_statuses),
                                ).await;
                            }
                            result = &mut search => {
                                let response = result?;
                                if response.sources.len() > sources_seen {
                                    apply_crypto_to_fiat_response(
                                        &mut routes,
                                        &mut asset_statuses,
                                        &query,
                                        &asset,
                                        &response,
                                    );
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                                break;
                            }
                        }
                    }
                } else {
                    let response = self.search_market(leg, query.offer_market()).await?;
                    apply_crypto_to_fiat_response(
                        &mut routes,
                        &mut asset_statuses,
                        &query,
                        &asset,
                        &response,
                    );
                }
            }
            (true, true) if query.includes_exchangers() => {
                let source_asset = query.source_currency.clone();
                let target_asset = query.target_currency.clone();
                if updates.is_some() {
                    let (market_updates, mut market_snapshots) = mpsc::channel(8);
                    let search =
                        self.stream_market_tickers(query.sources.as_deref(), market_updates);
                    tokio::pin!(search);
                    // Provider quotes and spot paths are independent. Neither
                    // is allowed to hold back the other's first snapshot.
                    let provider_search = self.search_provider_routes(&query);
                    tokio::pin!(provider_search);
                    let mut providers_finished = false;
                    let mut market_finished = false;
                    loop {
                        if providers_finished && market_finished {
                            break;
                        }
                        tokio::select! {
                            result = market_snapshots.recv() => {
                                let Some(result) = result else { continue };
                                if apply_crypto_market_result(
                                    &mut routes,
                                    &query,
                                    &source_asset,
                                    &target_asset,
                                    result,
                                ) {
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                            () = &mut search, if !market_finished => {
                                while let Ok(result) = market_snapshots.try_recv() {
                                    if apply_crypto_market_result(
                                        &mut routes,
                                        &query,
                                        &source_asset,
                                        &target_asset,
                                        result,
                                    ) {
                                        publish_update(
                                            updates.as_ref(),
                                            response_snapshot(search_id, &query, &routes, &asset_statuses),
                                        ).await;
                                    }
                                }
                                market_finished = true;
                            }
                            provider_routes = &mut provider_search, if !providers_finished => {
                                providers_finished = true;
                                if merge_routes(&mut routes, provider_routes) > 0 {
                                    publish_update(
                                        updates.as_ref(),
                                        response_snapshot(search_id, &query, &routes, &asset_statuses),
                                    ).await;
                                }
                            }
                        }
                    }
                } else {
                    let (market_results, provider_routes) = tokio::join!(
                        self.search_market_tickers(query.sources.as_deref()),
                        self.search_provider_routes(&query),
                    );
                    for result in market_results {
                        apply_crypto_market_result(
                            &mut routes,
                            &query,
                            &source_asset,
                            &target_asset,
                            result,
                        );
                    }
                    merge_routes(&mut routes, provider_routes);
                }
            }
            (true, true) => {}
        }
        let provider_search_started = Instant::now();
        let mut provider_routes_exhaustive = true;
        let mut provider_routes = if query.includes_exchangers() {
            match (source_is_crypto, target_is_crypto) {
                (false, true) => self.search_fiat_to_crypto_provider_routes(&query).await,
                (true, false) => self.search_crypto_to_fiat_provider_routes(&query).await,
                (false, false) => {
                    let (routes, exhaustive) = self.search_fiat_provider_routes(&query).await;
                    provider_routes_exhaustive = exhaustive;
                    routes
                }
                (true, true) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        if query.includes_exchangers() && !source_is_crypto && !target_is_crypto {
            provider_routes.extend(self.search_direct_fiat_routes(&query).await);
        }
        if merge_routes(&mut routes, provider_routes) > 0 {
            publish_update(
                updates.as_ref(),
                response_snapshot(search_id, &query, &routes, &asset_statuses),
            )
            .await;
        }
        let mut response = response_snapshot(search_id, &query, &routes, &asset_statuses);
        response.routes_exhaustive &= provider_routes_exhaustive;
        tracing::info!(
            source_currency = %query.source_currency,
            target_currency = %query.target_currency,
            routes_found = response.routes_found,
            routes_exhaustive = response.routes_exhaustive,
            provider_assets_ms = provider_assets_elapsed.as_millis(),
            local_search_ms = provider_search_started.duration_since(started).as_millis(),
            provider_search_ms = provider_search_started.elapsed().as_millis(),
            total_ms = started.elapsed().as_millis(),
            "p2p.route_search.completed"
        );
        Ok(response)
    }

    async fn search_direct_fiat_routes(&self, query: &NormalizedRouteQuery) -> Vec<P2pRoute> {
        let mut quotes = Vec::new();
        for provider in self
            .fiat_route_providers
            .iter()
            .filter(|provider| source_selected(query, provider.name()))
            .cloned()
        {
            if !provider.supports_pair(&query.source_currency, &query.target_currency) {
                continue;
            }
            let source_currency = query.source_currency.clone();
            let target_currency = query.target_currency.clone();
            let source_amount = query.source_amount;
            let key = fiat_quote_key(
                provider.name(),
                &source_currency,
                &target_currency,
                source_amount,
            );
            if let Some(quote) = self.cached_fiat_quote(&key) {
                quotes.push(quote);
            } else if !self.has_fmatch_backend() {
                if let Ok(quote) = provider
                    .quote(&source_currency, &target_currency, source_amount)
                    .await
                {
                    quotes.push(quote);
                }
            } else {
                self.refresh_fiat_quote(
                    key,
                    provider,
                    source_currency,
                    target_currency,
                    source_amount,
                );
            }
        }

        let mut routes = Vec::new();
        for quote in quotes {
            if !quote.source_amount.is_finite()
                || quote.source_amount <= 0.0
                || !quote.target_amount.is_finite()
                || quote.target_amount <= 0.0
                || !quote
                    .source_currency
                    .eq_ignore_ascii_case(&query.source_currency)
                || !quote
                    .target_currency
                    .eq_ignore_ascii_case(&query.target_currency)
            {
                continue;
            }
            let price = quote.source_amount / quote.target_amount;
            let mut payment_methods = [
                query.source_payment_method.clone(),
                query.target_payment_method.clone(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            payment_methods.sort();
            payment_methods.dedup();
            let offer = P2pOffer {
                market: P2pOfferMarket::DirectExchange,
                source: quote.provider.clone(),
                ad_id: format!(
                    "indicative-{}-{}",
                    quote.source_currency.to_ascii_lowercase(),
                    quote.target_currency.to_ascii_lowercase()
                ),
                side: P2pSide::BuyCrypto,
                fiat: quote.source_currency.clone(),
                asset: quote.target_currency.clone(),
                network: None,
                price: fixed(price, 12),
                available_asset: "1000000000".into(),
                min_fiat: "1".into(),
                max_fiat: "1000000000".into(),
                payment_methods,
                pay_time_limit_minutes: None,
                advertiser: crate::p2p::service::Advertiser {
                    id: None,
                    nickname: quote.provider.clone(),
                    user_type: Some("service".into()),
                    is_merchant: true,
                    is_verified: true,
                    completed_orders_30d: None,
                    completion_rate_30d: None,
                    positive_rate: None,
                },
                advertiser_profile_url: None,
                source_url: quote.source_url,
                source_url_is_exact: false,
            };
            routes.push(P2pRoute {
                route_id: String::new(),
                rank: 0,
                asset: quote.target_currency.clone(),
                entry_network: None,
                source_network: None,
                target_network: None,
                source_fiat: quote.source_currency,
                source_amount: fixed(quote.source_amount, 2),
                acquired_asset_amount: fixed(quote.target_amount, 2),
                target_fiat: quote.target_currency,
                target_amount: fixed(quote.target_amount, 2),
                effective_rate: fixed(quote.target_amount / quote.source_amount, 12),
                same_venue: true,
                requires_asset_transfer: false,
                transfer_fee_included: true,
                route_kind: "fiat_to_fiat".into(),
                bridge_currency: None,
                market_path: None,
                route_provider: None,
                route_provider_url: None,
                provider_quote_id: None,
                route_path: Vec::new(),
                route_fees: Vec::new(),
                quote_expires_at: None,
                payment_methods_verified: false,
                entry_offer: Some(offer),
                exit_offer: None,
                warnings: vec![
                    "Indicative direct-transfer quote; confirm the live rate, account eligibility, transfer limits, and recipient details with the provider before sending."
                        .into(),
                ],
                services: Vec::new(),
                reputation: None,
                feedback: None,
                service_links: Vec::new(),
            });
        }
        routes
    }
}

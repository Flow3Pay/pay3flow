use super::*;

impl P2pSearchService {
    pub(super) async fn search_fiat_to_crypto_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let target_assets = self.assets_on_network(
            &query.target_currency,
            query.target_network.as_deref(),
            &provider_assets,
        );
        if target_assets.is_empty() {
            return Vec::new();
        }
        let mut intermediary_groups = HashMap::<String, Vec<Asset>>::new();
        for intermediary in self.intermediary_assets(query).await {
            intermediary_groups
                .entry(intermediary.symbol.clone())
                .or_default()
                .push(intermediary);
        }
        let mut entry_searches = FuturesUnordered::new();
        for (symbol, intermediaries) in intermediary_groups {
            let service = self.clone();
            let query = query.clone();
            entry_searches.push(async move {
                let entry = service
                    .search_market(
                        leg_query(
                            &query.source_currency,
                            &symbol,
                            P2pSide::BuyCrypto,
                            Some(query.source_amount),
                            query.source_payment_method.clone(),
                            &query,
                        ),
                        query.offer_market(),
                    )
                    .await;
                (intermediaries, entry)
            });
        }
        let mut searches = FuturesUnordered::new();
        while let Some((intermediaries, entry)) = entry_searches.next().await {
            let Ok(entry) = entry else {
                continue;
            };
            for intermediary in &intermediaries {
                for offer in entry
                    .offers
                    .iter()
                    .filter(|offer| query.accepts_offer(offer))
                    .take(MAX_PROVIDER_OFFERS_PER_LEG)
                {
                    if !offer_matches_network(offer, intermediary.location.as_deref()) {
                        continue;
                    }
                    let Some(price) = positive_number(&offer.price) else {
                        continue;
                    };
                    let acquired = query.source_amount / price;
                    if positive_number(&offer.available_asset)
                        .is_none_or(|available| available < acquired)
                    {
                        continue;
                    }
                    for target in &target_assets {
                        if intermediary == target {
                            continue;
                        }
                        let Ok(amount) = Amount::from_f64(acquired, intermediary.clone()) else {
                            continue;
                        };
                        let capabilities = capabilities.clone();
                        let intermediary = intermediary.clone();
                        let target = target.clone();
                        let offer = offer.clone();
                        let query = query.clone();
                        let quote_semaphore = self.quote_semaphore.clone();
                        searches.push(async move {
                            quote_all_provider_refs(
                                capabilities,
                                intermediary,
                                target,
                                amount,
                                quote_semaphore,
                            )
                            .await
                                .into_iter()
                                .filter_map(|(provider, quote)| {
                                    let output = quote.output.value.parse::<f64>().ok()?;
                                    if !output.is_finite() || output <= 0.0 {
                                        return None;
                                    }
                                    let payment_methods_verified = query
                                        .source_payment_method
                                        .as_deref()
                                        .is_none_or(|method| {
                                            offer.payment_method_match(method)
                                                == PaymentMethodMatch::Exact
                                        });
                                    let mut warnings = vec![
                                        format!("Live dry quote from {provider}; execution and wallet compatibility are not verified."),
                                        "Fiat entry is a live P2P estimate; confirm payment details before sending.".into(),
                                    ];
                                    if !payment_methods_verified {
                                        warnings.push("The selected sender payment method could not be verified by the venue.".into());
                                    }
                                    Some(P2pRoute {
                                        route_id: String::new(),
                                        rank: 0,
                                        asset: quote.to.symbol.clone(),
                                        entry_network: quote.from.location.clone(),
                                        source_network: quote.from.location.clone(),
                                        target_network: quote.to.location.clone(),
                                        source_fiat: query.source_currency.clone(),
                                        source_amount: fixed(query.source_amount, 2),
                                        acquired_asset_amount: quote.output.value.clone(),
                                        target_fiat: query.target_currency.clone(),
                                        target_amount: quote.output.value.clone(),
                                        effective_rate: fixed(output / query.source_amount, 12),
                                        same_venue: false,
                                        requires_asset_transfer: true,
                                        transfer_fee_included: !quote.fees.is_empty(),
                                        route_kind: "fiat_to_crypto".into(),
                                        bridge_currency: None,
                                        market_path: None,
                                        route_provider: Some(provider),
                                        route_provider_url: quote.source_url,
                                        provider_quote_id: quote.quote_id.clone(),
                                        route_path: std::iter::once(query.source_currency.clone())
                                            .chain(quote.path.into_iter().map(|asset| asset.to_string()))
                                            .collect(),
                                        route_fees: quote
                                            .fees
                                            .into_iter()
                                            .map(|fee| RouteFee {
                                                asset: fee.asset.to_string(),
                                                amount: fee.value,
                                            })
                                            .collect(),
                                        quote_expires_at: quote.expires_at,
                                        payment_methods_verified,
                                        entry_offer: Some(offer.clone()),
                                        exit_offer: None,
                                        warnings,
                                        services: Vec::new(),
                                        reputation: None,
                                        feedback: None,
                                        service_links: Vec::new(),
                                    })
                                })
                                .collect::<Vec<_>>()
                        });
                    }
                }
            }
        }
        let mut routes = Vec::new();
        while let Some(batch) = searches.next().await {
            routes.extend(batch);
        }
        routes
    }
}

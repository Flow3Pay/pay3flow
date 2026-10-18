//! Crypto-to-crypto workflow route production.

use super::*;

impl P2pSearchService {
    pub(in crate::p2p) async fn search_provider_routes(
        &self,
        query: &NormalizedRouteQuery,
        batches: Option<&mpsc::Sender<RouteBatch>>,
    ) -> Vec<P2pRoute> {
        let capabilities = self.provider_capabilities_for_query(query).await;
        let provider_assets = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter().cloned())
            .collect::<Vec<_>>();
        let source_networks = self.assets_on_network(
            &query.source_currency,
            query.source_network.as_deref(),
            &provider_assets,
        );
        let target_networks = self.assets_on_network(
            &query.target_currency,
            query.target_network.as_deref(),
            &provider_assets,
        );
        let network_pairs = source_networks
            .iter()
            .flat_map(|source| {
                target_networks
                    .iter()
                    .map(move |target| (source.clone(), target.clone()))
            })
            .filter(|(source, target)| {
                !(source.location == target.location
                    && query
                        .source_currency
                        .eq_ignore_ascii_case(&query.target_currency))
            })
            .collect::<Vec<_>>();
        let mut searches = FuturesUnordered::new();
        let started = Instant::now();
        let failures = super::cycle::QuoteFailures::default();
        let mut statuses = HashMap::new();
        let mut scheduled = std::collections::HashSet::new();
        for capability in capabilities.iter() {
            let name = capability.provider.name().to_string();
            statuses.insert(
                name.clone(),
                SourceStatus {
                    source: name,
                    ok: true,
                    cached: false,
                    latency_ms: 0,
                    offers_found: 0,
                    error: Some("No direct pair in this provider's token/network catalog".into()),
                },
            );
            for (source, target) in network_pairs
                .iter()
                .filter(|(source, target)| capability.supports(source, target))
                .take(MAX_PROVIDER_NETWORK_PAIRS)
            {
                let from = source.clone();
                let to = target.clone();
                let Ok(amount) = Amount::from_f64(query.source_amount, from.clone()) else {
                    continue;
                };
                let provider = capability.provider.clone();
                scheduled.insert(provider.name().to_string());
                let service = self.clone();
                let failures = failures.clone();
                searches.push(async move {
                    let name = provider.name().to_string();
                    let (amount, result) = service
                        .source_quote(provider, from.clone(), to.clone(), amount, failures)
                        .await;
                    (name, from, to, amount, result)
                });
            }
        }

        let mut routes = Vec::new();
        while let Some(result) = searches.next().await {
            let (provider_name, from, to, amount, result) = result;
            let Some(status) = statuses.get_mut(&provider_name) else {
                continue;
            };
            status.latency_ms = started.elapsed().as_millis();
            let quotes = match result {
                Ok(quotes) => {
                    status.offers_found += quotes.len();
                    if status.ok {
                        status.error = None;
                    }
                    quotes
                }
                Err(error) => {
                    status.ok = false;
                    status.error = Some(error.to_string());
                    Vec::new()
                }
            };
            let mut batch = Vec::new();
            for quote in quotes {
                if !super::crypto_paths::valid_quote(&quote, &from, &to, &amount) {
                    continue;
                }
                let Ok(input_value) = quote.input.value.parse::<f64>() else {
                    continue;
                };
                let Ok(output_value) = quote.output.value.parse::<f64>() else {
                    continue;
                };
                if !input_value.is_finite() || !output_value.is_finite() || output_value <= 0.0 {
                    continue;
                }
                let source_network = quote.from.location.clone();
                let target_network = quote.to.location.clone();
                let mut warnings = vec![format!(
                "Live dry quote from {provider_name}; execution and wallet compatibility are not verified."
            )];
                if let Some(description) = quote.description.as_deref() {
                    warnings.push(format!("Quoted exchanger: {description}."));
                }
                if source_network != target_network {
                    warnings.push("Cross-network transfer requires the provider's deposit and withdrawal flow; confirm addresses, memos, network fees, and finality before sending.".into());
                }
                if [&quote.from, &quote.to].iter().any(|asset| {
                    asset.symbol == "EVER" && asset.location.as_deref() == Some("ethereum")
                }) {
                    warnings.push("EVER on Ethereum is an ERC-20 representation (Wrapped EVER/WEVER), not a payout of native EVER on Everscale.".into());
                }
                batch.push(P2pRoute {
                    route_id: String::new(),
                    rank: 0,
                    asset: quote.to.symbol.clone(),
                    entry_network: source_network.clone(),
                    source_network,
                    target_network,
                    source_fiat: quote.from.symbol.clone(),
                    source_amount: quote.input.value.clone(),
                    acquired_asset_amount: quote.output.value.clone(),
                    provider_input_amount: Some(quote.input.value.clone()),
                    target_fiat: quote.to.symbol.clone(),
                    target_amount: quote.output.value,
                    effective_rate: fixed(output_value / input_value, 12),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: !quote.fees.is_empty(),
                    route_kind: "crypto_to_crypto".into(),
                    profitability: None,
                    cycle_legs: Vec::new(),
                    profitability_decimals: None,
                    bridge_currency: None,
                    market_path: None,
                    route_provider: Some(provider_name.clone()),
                    route_provider_url: quote.source_url,
                    provider_quote_id: quote.quote_id.clone(),
                    route_path: quote
                        .path
                        .into_iter()
                        .map(|asset| asset.to_string())
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
                    execution: None,
                    payment_methods_verified: true,
                    entry_offer: None,
                    exit_offer: None,
                    warnings,
                    services: Vec::new(),
                    reputation: None,
                    feedback: None,
                    service_links: Vec::new(),
                });
            }
            routes.extend(batch.iter().cloned());
            if let Some(batches) = batches {
                send_batch(
                    batches,
                    RouteBatch::ProviderResult {
                        status: status.clone(),
                        routes: batch,
                        exhaustive: false,
                    },
                )
                .await;
            }
        }
        if let Some(batches) = batches {
            for status in statuses
                .values()
                .filter(|status| !scheduled.contains(&status.source))
            {
                send_batch(
                    batches,
                    RouteBatch::ProviderResult {
                        status: status.clone(),
                        routes: Vec::new(),
                        exhaustive: false,
                    },
                )
                .await;
            }
        }
        if query.allow_cross_venue {
            routes.extend(
                self.search_composed_crypto_routes(
                    query,
                    &capabilities,
                    &source_networks,
                    &target_networks,
                    batches,
                    &mut statuses,
                )
                .await,
            );
        }
        routes
    }
}

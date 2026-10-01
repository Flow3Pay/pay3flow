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
        for capability in capabilities.iter() {
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
                searches.push(quote_provider_many(
                    capability.provider.clone(),
                    from,
                    to,
                    amount,
                    self.quote_semaphore.clone(),
                ));
            }
        }

        let mut routes = Vec::new();
        while let Some(result) = searches.next().await {
            let Some((provider_name, quotes)) = result else {
                continue;
            };
            let mut batch = Vec::new();
            for quote in quotes {
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
                    target_fiat: quote.to.symbol.clone(),
                    target_amount: quote.output.value,
                    effective_rate: fixed(output_value / input_value, 12),
                    same_venue: false,
                    requires_asset_transfer: true,
                    transfer_fee_included: !quote.fees.is_empty(),
                    route_kind: "crypto_to_crypto".into(),
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
            emit_routes(batches, &batch).await;
            routes.extend(batch);
        }
        routes
    }
}

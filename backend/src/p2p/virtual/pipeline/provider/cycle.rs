//! Wallet cycles use fresh, amount-specific quotes, never cached price coefficients.

use super::*;
use crate::p2p::CryptoCycleLeg;

impl P2pSearchService {
    pub(super) async fn search_crypto_cycles(
        &self,
        query: &NormalizedRouteQuery,
        batches: &mpsc::Sender<RouteBatch>,
    ) -> Result<()> {
        let origin = Asset::new(&query.source_currency, query.source_network.as_deref())?;
        let amount = Amount::from_f64(query.source_amount, origin.clone())?;
        let capabilities = self.provider_capabilities_for_query(query).await;
        let mut intermediaries = capabilities
            .iter()
            .flat_map(|capability| capability.assets.iter())
            .filter(|asset| **asset != origin && asset.qualified())
            .filter(|asset| !query.assets_explicit || query.assets.contains(&asset.symbol))
            .filter(|asset| {
                capabilities
                    .iter()
                    .any(|capability| capability.supports(&origin, asset))
            })
            .cloned()
            .collect::<Vec<_>>();
        intermediaries.sort_by_key(|asset| {
            (
                asset.symbol == origin.symbol,
                intermediary_asset_priority(&asset.symbol, ""),
                asset.location != origin.location,
                asset.to_string(),
            )
        });
        intermediaries.dedup();
        // Reserve one candidate per symbol before additional network variants,
        // otherwise many USDC chains can crowd BTC, ETH or SOL out of the search.
        let mut symbols = std::collections::HashSet::new();
        let mut primary = Vec::new();
        let mut variants = Vec::new();
        for asset in intermediaries {
            if symbols.insert(asset.symbol.clone()) {
                primary.push(asset);
            } else {
                variants.push(asset);
            }
        }
        primary.extend(variants);
        primary.truncate(MAX_PROVIDER_ASSETS);
        let intermediaries = primary;

        let mut entries = FuturesUnordered::new();
        for capability in capabilities.iter() {
            for intermediary in intermediaries.iter().filter(|intermediary| {
                capability.supports(&origin, intermediary)
                    && capabilities.iter().any(|exit| {
                        exit.supports(intermediary, &origin)
                            && (query.allow_cross_venue
                                || exit.provider.name() == capability.provider.name())
                    })
            }) {
                let provider = capability.provider.clone();
                let origin = origin.clone();
                let intermediary = intermediary.clone();
                let amount = amount.clone();
                let semaphore = self.quote_semaphore.clone();
                entries.push(async move {
                    let (_, quotes) = quote_provider_many(
                        provider.clone(),
                        origin.clone(),
                        intermediary.clone(),
                        amount.clone(),
                        semaphore,
                    )
                    .await?;
                    let quotes = quotes
                        .into_iter()
                        .filter(|quote| {
                            quote.provider == provider.name()
                                && valid_cycle_quote(quote, &origin, &intermediary, &amount)
                        })
                        .take(MAX_PROVIDER_OFFERS_PER_LEG)
                        .collect::<Vec<_>>();
                    Some((provider, intermediary, quotes))
                });
            }
        }

        let mut exits = FuturesUnordered::new();
        while !entries.is_empty() || !exits.is_empty() {
            tokio::select! {
                Some(result) = entries.next(), if !entries.is_empty() => {
                    let Some((entry_provider, intermediary, quotes)) = result else { continue; };
                    for entry in quotes {
                        for exit in capabilities.iter().filter(|exit| {
                            exit.supports(&intermediary, &origin)
                                && (query.allow_cross_venue || exit.provider.name() == entry_provider.name())
                        }) {
                            let provider = exit.provider.clone();
                            let semaphore = self.quote_semaphore.clone();
                            let origin = origin.clone();
                            let intermediary = intermediary.clone();
                            let entry = entry.clone();
                            exits.push(async move {
                                let (_, quotes) = quote_provider_many(
                                    provider.clone(), intermediary.clone(), origin.clone(), entry.output.clone(), semaphore,
                                ).await?;
                                Some(quotes.into_iter().filter(|quote| {
                                    quote.provider == provider.name()
                                        && valid_cycle_quote(quote, &intermediary, &origin, &entry.output)
                                }).take(MAX_PROVIDER_OFFERS_PER_LEG).filter_map(|exit| {
                                    crypto_cycle_route(query, &entry, exit)
                                }).collect::<Vec<_>>())
                            });
                        }
                    }
                }
                Some(result) = exits.next(), if !exits.is_empty() => {
                    if let Some(routes) = result {
                        if !send_batch(batches, RouteBatch::Routes { routes, exhaustive: false }).await {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn valid_cycle_quote(quote: &PublicRouteQuote, from: &Asset, to: &Asset, input: &Amount) -> bool {
    quote.from == *from
        && quote.to == *to
        && quote.input.asset == *from
        && quote.output.asset == *to
        && positive_number(&quote.input.value).is_some()
        && positive_number(&quote.input.value) == positive_number(&input.value)
        && positive_number(&quote.output.value).is_some()
        && quote
            .expires_at
            .is_none_or(|expiry| expiry > chrono::Utc::now())
        && (quote.path.is_empty()
            || (quote.path.first() == Some(from) && quote.path.last() == Some(to)))
}

fn cycle_leg(quote: &PublicRouteQuote) -> CryptoCycleLeg {
    CryptoCycleLeg {
        provider: quote.provider.clone(),
        from_asset: quote.from.to_string(),
        to_asset: quote.to.to_string(),
        input_amount: quote.input.value.clone(),
        output_amount: quote.output.value.clone(),
        source_url: quote.source_url.clone(),
        quote_id: quote.quote_id.clone(),
        expires_at: quote.expires_at,
    }
}

fn crypto_cycle_route(
    query: &NormalizedRouteQuery,
    entry: &PublicRouteQuote,
    exit: PublicRouteQuote,
) -> Option<P2pRoute> {
    // Compare at the display's crypto precision; a sub-unit gain is not a route.
    let input = crypto_profit_units(&entry.input.value)?;
    let output = crypto_profit_units(&exit.output.value)?;
    if output <= input {
        return None;
    }
    let input_value = positive_number(&entry.input.value)?;
    let output_value = positive_number(&exit.output.value)?;
    let expires_at = entry.expires_at.into_iter().chain(exit.expires_at).min();
    if expires_at.is_some_and(|expiry| expiry <= chrono::Utc::now()) {
        return None;
    }
    let same_venue = entry.provider == exit.provider;
    let mut path = if entry.path.is_empty() {
        vec![entry.from.to_string(), entry.to.to_string()]
    } else {
        entry.path.iter().map(ToString::to_string).collect()
    };
    if exit.path.is_empty() {
        path.push(exit.to.to_string());
    } else {
        path.extend(exit.path.iter().skip(1).map(ToString::to_string));
    }
    Some(P2pRoute {
        route_id: String::new(),
        rank: 0,
        asset: entry.to.symbol.clone(),
        entry_network: query.source_network.clone(),
        source_network: query.source_network.clone(),
        target_network: query.target_network.clone(),
        source_fiat: query.source_currency.clone(),
        source_amount: entry.input.value.clone(),
        acquired_asset_amount: entry.output.value.clone(),
        provider_input_amount: None,
        target_fiat: query.target_currency.clone(),
        target_amount: exit.output.value.clone(),
        effective_rate: fixed(output_value / input_value, 12),
        same_venue,
        requires_asset_transfer: true,
        // Public swap quotes do not establish the cost of sending from the wallet
        // on both legs (gas, approvals, deposits). Never confirm net arbitrage here.
        transfer_fee_included: false,
        route_kind: "crypto_cycle".into(),
        profitability: None,
        profitability_decimals: Some(8),
        cycle_legs: vec![cycle_leg(entry), cycle_leg(&exit)],
        bridge_currency: Some(entry.to.symbol.clone()),
        market_path: None,
        route_provider: same_venue.then(|| entry.provider.clone()),
        route_provider_url: None,
        provider_quote_id: None,
        route_path: path,
        route_fees: entry.fees.iter().chain(&exit.fees).map(|fee| RouteFee {
            asset: fee.asset.to_string(),
            amount: fee.value.clone(),
        }).collect(),
        quote_expires_at: expires_at,
        execution: None,
        payment_methods_verified: true,
        entry_offer: None,
        exit_offer: None,
        warnings: vec![
            "Positive quoted cycle output; wallet gas, approvals and deposit costs are not fully verified. Net profit is unconfirmed.".into(),
            "The return quote uses the first swap's output. Both rates can change before the sequential swaps finish; re-quote every step before sending.".into(),
        ],
        services: Vec::new(),
        reputation: None,
        feedback: None,
        service_links: Vec::new(),
    })
}

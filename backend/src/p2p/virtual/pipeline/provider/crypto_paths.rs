//! Two independently quoted swaps joined by the exact same token and network.
use super::cycle::{QuoteFailures, QuoteJob, QuoteResult};
use super::*;

const MAX_INTERMEDIARIES_PER_PROVIDER: usize = 8;
const MAX_QUOTES_PER_PROVIDER: usize = 32;

pub(super) fn valid_quote(
    quote: &PublicRouteQuote,
    from: &Asset,
    to: &Asset,
    amount: &Amount,
) -> bool {
    quote.from == *from
        && quote.to == *to
        && quote.input.asset == *from
        && quote.output.asset == *to
        && same_crypto_amount(&quote.input.value, &amount.value)
        && positive_number(&quote.input.value).is_some()
        && positive_number(&quote.output.value).is_some()
        && quote
            .expires_at
            .is_none_or(|expiry| expiry > chrono::Utc::now())
}

impl P2pSearchService {
    pub(super) async fn search_composed_crypto_routes(
        &self,
        query: &NormalizedRouteQuery,
        capabilities: &[RouteProviderCapability],
        origins: &[Asset],
        destinations: &[Asset],
        batches: Option<&mpsc::Sender<RouteBatch>>,
        statuses: &mut HashMap<String, SourceStatus>,
    ) -> Vec<P2pRoute> {
        let started = Instant::now();
        let failures = QuoteFailures::default();
        let mut jobs = HashMap::<String, QuoteJob>::new();
        let mut counts = HashMap::<String, usize>::new();
        let mut entries = FuturesUnordered::new();
        for capability in capabilities {
            let mut candidates = origins
                .iter()
                .flat_map(|from| {
                    capability.assets.iter().filter_map(move |middle| {
                        if middle == from
                            || !middle.qualified()
                            || destinations.contains(middle)
                            || (query.assets_explicit && !query.assets.contains(&middle.symbol))
                            || !capability.supports(from, middle)
                            || !capabilities.iter().any(|exit| {
                                exit.provider.name() != capability.provider.name()
                                    && destinations.iter().any(|to| exit.supports(middle, to))
                            })
                        {
                            return None;
                        }
                        Some((from.clone(), middle.clone()))
                    })
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|(from, middle)| {
                (
                    !destinations.iter().any(|to| to.location == middle.location),
                    intermediary_asset_priority(&middle.symbol, ""),
                    network_priority(middle.location.as_deref()),
                    from.to_string(),
                    middle.to_string(),
                )
            });
            candidates.dedup();
            candidates.truncate(MAX_INTERMEDIARIES_PER_PROVIDER);
            for (from, middle) in candidates {
                let Ok(amount) = Amount::from_f64(query.source_amount, from.clone()) else {
                    continue;
                };
                let name = capability.provider.name().to_string();
                *counts.entry(name.clone()).or_default() += 1;
                let service = self.clone();
                let provider = capability.provider.clone();
                let failures = failures.clone();
                entries.push(async move {
                    let (amount, result) = service
                        .source_quote(provider, from.clone(), middle.clone(), amount, failures)
                        .await;
                    (name, from, middle, amount, result)
                });
            }
        }
        let mut exits = FuturesUnordered::new();
        let mut routes = Vec::new();
        loop {
            tokio::select! {
                Some((name, from, middle, amount, result)) = entries.next(), if !entries.is_empty() => {
                    emit_status(statuses, &name, &result, started, batches).await;
                    let Ok(quotes) = result else { continue };
                    for entry in quotes.into_iter().filter(|quote| valid_quote(quote, &from, &middle, &amount)).take(MAX_PROVIDER_OFFERS_PER_LEG) {
                        for exit in capabilities.iter().filter(|exit| exit.provider.name() != name) {
                            for to in destinations.iter().filter(|to| exit.supports(&middle, to)) {
                                let key = provider_quote_key(exit.provider.name(), &middle, to, &entry.output);
                                let job = if let Some(job) = jobs.get(&key) { job.clone() } else {
                                    let count = counts.entry(exit.provider.name().to_string()).or_default();
                                    if *count >= MAX_QUOTES_PER_PROVIDER { continue; }
                                    *count += 1;
                                    let job = self.cycle_quote_job(exit.provider.clone(), middle.clone(), to.clone(), entry.output.clone(), failures.clone());
                                    jobs.insert(key, job.clone());
                                    job
                                };
                                let entry = entry.clone();
                                let to = to.clone();
                                let name = exit.provider.name().to_string();
                                exits.push(async move { (name, entry, to, job.await) });
                            }
                        }
                    }
                }
                Some((name, entry, to, result)) = exits.next(), if !exits.is_empty() => {
                    emit_status(statuses, &name, &result, started, batches).await;
                    let Ok(quotes) = result else { continue };
                    let batch = quotes.into_iter()
                        .filter(|quote| valid_quote(quote, &entry.to, &to, &entry.output))
                        .filter_map(|exit| crypto_swap_from_quotes(query, &[entry.clone(), exit]))
                        .collect::<Vec<_>>();
                    emit_routes(batches, &batch).await;
                    routes.extend(batch);
                }
                else => break,
            }
        }
        routes
    }
}

async fn emit_status(
    statuses: &mut HashMap<String, SourceStatus>,
    name: &str,
    result: &QuoteResult,
    started: Instant,
    batches: Option<&mpsc::Sender<RouteBatch>>,
) {
    let Some(status) = statuses.get_mut(name) else {
        return;
    };
    status.latency_ms = status.latency_ms.max(started.elapsed().as_millis());
    match result {
        Ok(quotes) => {
            status.offers_found += quotes.len();
            if status.ok {
                status.error = None;
            }
        }
        Err(error) => {
            status.ok = false;
            status.error = Some(error.to_string());
        }
    }
    if let Some(batches) = batches {
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

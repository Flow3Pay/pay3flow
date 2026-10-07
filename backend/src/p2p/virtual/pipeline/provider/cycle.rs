//! Wallet cycles use fresh, amount-specific quotes, never cached price coefficients.

use super::*;
use crate::route_engine::PublicRouteProvider;
use futures::future::{BoxFuture, FutureExt, Shared};

const MAX_CYCLE_ASSETS_PER_PROVIDER: usize = 48;
const CYCLE_SEARCH_BUDGET: std::time::Duration = std::time::Duration::from_secs(60);
type QuoteJob = Shared<BoxFuture<'static, std::result::Result<Vec<PublicRouteQuote>, String>>>;

impl P2pSearchService {
    pub(super) async fn search_crypto_cycles(
        &self,
        query: &NormalizedRouteQuery,
        batches: &mpsc::Sender<RouteBatch>,
    ) -> Result<()> {
        let origin = Asset::new(&query.source_currency, query.source_network.as_deref())?;
        let amount = Amount::from_f64(query.source_amount, origin.clone())?;
        let capabilities = self.provider_capabilities_for_query(query).await;
        let started = Instant::now();
        let mut statuses = HashMap::new();
        let mut jobs = HashMap::<String, QuoteJob>::new();
        let failures = Arc::new(std::sync::Mutex::new(HashMap::<String, String>::new()));
        let mut recorded = std::collections::HashSet::new();
        let mut entries = FuturesUnordered::new();
        for capability in capabilities.iter() {
            // Select separately for each provider: a large catalog must never
            // crowd another provider's supported assets or networks out.
            let mut candidates = capability
                .assets
                .iter()
                .filter(|asset| {
                    **asset != origin
                        && asset.qualified()
                        && (!query.assets_explicit || query.assets.contains(&asset.symbol))
                        && capability.supports(&origin, asset)
                        && capabilities.iter().any(|exit| {
                            exit.supports(asset, &origin)
                                && (query.allow_cross_venue
                                    || exit.provider.name() == capability.provider.name())
                        })
                })
                .cloned()
                .collect::<Vec<_>>();
            candidates.sort_by_key(|asset| {
                (
                    intermediary_asset_priority(&asset.symbol, ""),
                    asset.symbol == origin.symbol,
                    asset.location != origin.location,
                    network_priority(asset.location.as_deref()),
                    asset.to_string(),
                )
            });
            candidates.dedup();
            // Round-robin symbols, then their network variants. Both are part
            // of the graph, including the origin symbol on a different chain.
            let mut groups = Vec::<Vec<Asset>>::new();
            let mut group_indices = HashMap::new();
            for asset in candidates {
                let next = groups.len();
                let index = *group_indices.entry(asset.symbol.clone()).or_insert(next);
                if index == next {
                    groups.push(Vec::new());
                }
                groups[index].push(asset);
            }
            // Reserve capacity for other chains even in catalogs with hundreds
            // of distinct symbols; then fill unused slots with more symbols.
            let mut candidates = groups
                .iter()
                .take(32)
                .filter_map(|group| group.first())
                .cloned()
                .collect::<Vec<_>>();
            for variant in 1..groups.iter().map(Vec::len).max().unwrap_or(0) {
                for group in &groups {
                    if let Some(asset) = group.get(variant) {
                        candidates.push(asset.clone());
                    }
                }
                if candidates.len() >= MAX_CYCLE_ASSETS_PER_PROVIDER {
                    break;
                }
            }
            candidates.extend(
                groups
                    .iter()
                    .skip(32)
                    .filter_map(|group| group.first())
                    .cloned(),
            );
            candidates.truncate(MAX_CYCLE_ASSETS_PER_PROVIDER);
            let name = capability.provider.name().to_string();
            statuses.insert(
                name.clone(),
                SourceStatus {
                    source: name.clone(),
                    ok: true,
                    cached: false,
                    latency_ms: 0,
                    offers_found: 0,
                    error: candidates.is_empty().then(|| {
                        format!("No supported return path for {origin} in this provider's catalog")
                    }),
                },
            );
            let entry_gate = Arc::new(tokio::sync::Semaphore::new(1));
            for intermediary in candidates {
                let job = self.cycle_quote_job(
                    capability.provider.clone(),
                    origin.clone(),
                    intermediary.clone(),
                    amount.clone(),
                    failures.clone(),
                );
                let key = provider_quote_key(&name, &origin, &intermediary, &amount);
                jobs.insert(key.clone(), job.clone());
                let name = name.clone();
                let entry_gate = entry_gate.clone();
                entries.push(async move {
                    // Leave the provider lane available for return quotes, so
                    // a large catalog cannot queue every entry ahead of exits.
                    let result = match entry_gate.acquire_owned().await {
                        Ok(_permit) => job.await,
                        Err(error) => Err(error.to_string()),
                    };
                    (name, key, intermediary, result)
                });
            }
        }
        let mut exits = FuturesUnordered::new();
        let deadline = tokio::time::sleep(CYCLE_SEARCH_BUDGET);
        tokio::pin!(deadline);
        while !entries.is_empty() || !exits.is_empty() {
            let changed = tokio::select! {
                _ = batches.closed() => return Ok(()),
                _ = &mut deadline => {
                    for status in statuses.values_mut() {
                        if status.error.is_none() { status.error = Some("Search time limit reached; some paths remain unchecked".into()); }
                    }
                    break;
                }
                Some((name, key, intermediary, result)) = entries.next(), if !entries.is_empty() => {
                    if recorded.insert(key) { record_cycle_result(&mut statuses, &name, &result, started); }
                    let quotes = result.unwrap_or_default();
                    for entry in quotes.into_iter().filter(|quote| quote.provider == name && valid_cycle_quote(quote, &origin, &intermediary, &amount)).take(MAX_PROVIDER_OFFERS_PER_LEG) {
                        for exit in capabilities.iter().filter(|exit| exit.supports(&intermediary, &origin)
                            && (query.allow_cross_venue || exit.provider.name() == name)) {
                            let exit_name = exit.provider.name().to_string();
                            let key = provider_quote_key(&exit_name, &intermediary, &origin, &entry.output);
                            // Identical input amounts share one exact quote; keep
                            // all exchanger identities when building routes.
                            let job = jobs.entry(key.clone()).or_insert_with(|| self.cycle_quote_job(exit.provider.clone(), intermediary.clone(), origin.clone(), entry.output.clone(), failures.clone())).clone();
                            let origin = origin.clone();
                            let intermediary = intermediary.clone();
                            let entry = entry.clone();
                            exits.push(async move {
                                let result = job.await;
                                let routes = result.as_ref().map(|quotes| quotes.iter().filter(|quote| quote.provider == exit_name && valid_cycle_quote(quote, &intermediary, &origin, &entry.output)).take(MAX_PROVIDER_OFFERS_PER_LEG).filter_map(|exit| crypto_cycle_route(query, &entry, exit.clone())).collect::<Vec<_>>()).unwrap_or_default();
                                (exit_name, key, result, routes)
                            });
                        }
                    }
                    name
                }
                Some((name, key, result, routes)) = exits.next(), if !exits.is_empty() => {
                    if recorded.insert(key) { record_cycle_result(&mut statuses, &name, &result, started); }
                    if !routes.is_empty() && !send_batch(batches, RouteBatch::Routes { routes, exhaustive: false }).await { return Ok(()); }
                    name
                }
            };
            if let Some(status) = statuses.get(&changed) {
                if !send_batch(
                    batches,
                    RouteBatch::ProviderResult {
                        status: status.clone(),
                        routes: Vec::new(),
                        exhaustive: false,
                    },
                )
                .await
                {
                    return Ok(());
                }
            }
        }
        for status in statuses.into_values() {
            if !send_batch(
                batches,
                RouteBatch::ProviderResult {
                    status,
                    routes: Vec::new(),
                    exhaustive: false,
                },
            )
            .await
            {
                break;
            }
        }
        Ok(())
    }

    fn cycle_quote_job(
        &self,
        provider: Arc<dyn PublicRouteProvider>,
        from: Asset,
        to: Asset,
        amount: Amount,
        failures: Arc<std::sync::Mutex<HashMap<String, String>>>,
    ) -> QuoteJob {
        let service = self.clone();
        async move {
            let failure_key = format!("{}|{}|{}", provider.name(), from, amount.value);
            let prior_failure = failures
                .lock()
                .map_err(|error| error.to_string())?
                .get(&failure_key)
                .cloned();
            if let Some(error) = prior_failure {
                return Err(error);
            }
            service
                .paced_cycle_quotes(provider, from, to, amount, failures)
                .await
        }
        .boxed()
        .shared()
    }

    async fn paced_cycle_quotes(
        &self,
        provider: Arc<dyn PublicRouteProvider>,
        from: Asset,
        to: Asset,
        amount: Amount,
        failures: Arc<std::sync::Mutex<HashMap<String, String>>>,
    ) -> std::result::Result<Vec<PublicRouteQuote>, String> {
        let lane = {
            let mut lanes = self
                .cycle_quote_lanes
                .lock()
                .map_err(|error| error.to_string())?;
            lanes
                .entry(provider.name().to_string())
                .or_default()
                .clone()
        };
        // This asynchronous guard intentionally serializes one provider's HTTP
        // calls, while different providers continue independently.
        let mut lane = lane.lock().await;
        if let Some(error) = failures
            .lock()
            .map_err(|error| error.to_string())?
            .get(&format!("{}|{}|{}", provider.name(), from, amount.value))
            .cloned()
        {
            return Err(error);
        }
        if let Some((until, error)) = &lane.blocked_until {
            if *until > tokio::time::Instant::now() {
                return Err(error.clone());
            }
        }
        if let Some(next) = lane.next_start {
            tokio::time::sleep_until(next).await;
        }
        let spacing = match provider.name() {
            "symbiosis" => 350,
            "near-intents" | "cow-swap" => 150,
            _ => 0,
        };
        lane.next_start =
            Some(tokio::time::Instant::now() + std::time::Duration::from_millis(spacing));
        let _permit = self
            .quote_semaphore
            .acquire()
            .await
            .map_err(|error| error.to_string())?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(12),
            provider.quotes(from.clone(), to.clone(), amount.clone()),
        )
        .await
        .map_err(|_| "Quote request timed out after 12 seconds".to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));
        if let Err(error) = &result {
            tracing::warn!(provider = provider.name(), %from, %to, amount = %amount.value, %error, "crypto cycle quote unavailable");
            if error.contains("Temporary swap limits: minimum swap amount") {
                failures.lock().map_err(|error| error.to_string())?.insert(
                    format!("{}|{}|{}", provider.name(), from, amount.value),
                    error.clone(),
                );
            }
            if error.contains("429") || error.contains("quota exceeded") {
                lane.blocked_until = Some((
                    tokio::time::Instant::now() + std::time::Duration::from_secs(60),
                    error.clone(),
                ));
            }
        }
        result
    }
}

fn record_cycle_result(
    statuses: &mut HashMap<String, SourceStatus>,
    name: &str,
    result: &std::result::Result<Vec<PublicRouteQuote>, String>,
    started: Instant,
) {
    if let Some(status) = statuses.get_mut(name) {
        status.latency_ms = started.elapsed().as_millis();
        match result {
            Ok(quotes) => status.offers_found += quotes.len(),
            Err(error) => {
                status.ok = false;
                status.error = Some(error.clone());
            }
        }
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

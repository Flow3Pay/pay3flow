use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;

use crate::p2p::routes::{FiatProviderQuoteJob, PROVIDER_QUOTE_CACHE_TTL};
use crate::p2p::service::{CachedFiatQuote, CachedProviderQuote};
use crate::p2p::P2pSearchService;
use crate::route_engine::PublicRouteQuote;

/// Ceiling for a single external provider quote. Providers normally answer in
/// seconds; this only bounds a provider that never responds.
const QUOTE_TIMEOUT_SECS: u64 = 12;

impl P2pSearchService {
    pub(in crate::p2p) fn cached_provider_quote(&self, key: &str) -> Option<PublicRouteQuote> {
        self.provider_quote_cache
            .read()
            .ok()?
            .get(key)
            .filter(|cached| {
                cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL
                    && cached
                        .quote
                        .expires_at
                        .is_none_or(|expires_at| expires_at > Utc::now())
            })
            .map(|cached| cached.quote.clone())
    }

    pub(in crate::p2p) fn refresh_provider_quote(&self, key: String, job: &FiatProviderQuoteJob) {
        let refresh_key = key.clone();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let provider = job.provider.clone();
        let from = job.from.clone();
        let to = job.to.clone();
        let amount = job.amount.clone();
        let quote_semaphore = self.quote_semaphore.clone();
        let cache = self.provider_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        let Ok(permit) = quote_semaphore.try_acquire_owned() else {
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };
        tokio::spawn(async move {
            let provider_name = provider.name().to_string();
            let result = tokio::time::timeout(
                Duration::from_secs(QUOTE_TIMEOUT_SECS),
                provider.quote(from, to, amount),
            )
            .await;
            drop(permit);
            match result {
                Ok(Ok(quote)) => {
                    if let Ok(mut cache) = cache.write() {
                        cache.insert(
                            key,
                            CachedProviderQuote {
                                inserted_at: Instant::now(),
                                quote,
                            },
                        );
                    }
                }
                Ok(Err(error)) => {
                    tracing::debug!(%error, provider = %provider_name, "background provider quote failed");
                }
                Err(_) => {
                    tracing::debug!(provider = %provider_name, "background provider quote timed out");
                }
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }

    pub(in crate::p2p) fn cached_fiat_quote(
        &self,
        key: &str,
    ) -> Option<crate::p2p::FiatRouteQuote> {
        self.fiat_quote_cache
            .read()
            .ok()?
            .get(key)
            .filter(|cached| cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL)
            .map(|cached| cached.quote.clone())
    }

    /// Fetches a fiat quote and waits for it, so the caller can stream this
    /// provider's route as soon as it answers. `refresh_fiat_quote` only warms
    /// the cache and returns nothing, which hides the result from the current
    /// search and makes it surface later inside an unrelated batch.
    pub(in crate::p2p) async fn fetch_fiat_quote(
        &self,
        key: String,
        provider: Arc<dyn crate::p2p::PublicFiatRouteProvider>,
        source_currency: String,
        target_currency: String,
        source_amount: f64,
    ) -> Option<crate::p2p::FiatRouteQuote> {
        let Ok(permit) = self.quote_semaphore.clone().acquire_owned().await else {
            return None;
        };
        // The permit is held across the timeout so a slow provider cannot keep
        // consuming a request slot.
        let quote = tokio::time::timeout(
            Duration::from_secs(QUOTE_TIMEOUT_SECS),
            provider.quote(&source_currency, &target_currency, source_amount),
        )
        .await
        .ok()
        .and_then(Result::ok);
        drop(permit);
        if let Some(quote) = &quote {
            if let Ok(mut cache) = self.fiat_quote_cache.write() {
                cache.insert(
                    key,
                    CachedFiatQuote {
                        inserted_at: Instant::now(),
                        quote: quote.clone(),
                    },
                );
            }
        }
        quote
    }
}

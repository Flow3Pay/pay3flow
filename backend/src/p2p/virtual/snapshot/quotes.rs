use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;

use crate::p2p::routes::{fiat_quote_key, FiatProviderQuoteJob, PROVIDER_QUOTE_CACHE_TTL};
use crate::p2p::service::{CachedFiatQuote, CachedProviderQuote};
use crate::p2p::P2pSearchService;
use crate::route_engine::PublicRouteQuote;

impl P2pSearchService {
    pub(in crate::p2p) async fn fiat_quote(
        &self,
        provider: Arc<dyn crate::p2p::PublicFiatRouteProvider>,
        source_currency: &str,
        target_currency: &str,
        source_amount: f64,
    ) -> Option<crate::p2p::FiatRouteQuote> {
        let key = fiat_quote_key(
            provider.name(),
            source_currency,
            target_currency,
            source_amount,
        );
        if let Some(quote) = self.cached_fiat_quote(&key) {
            return Some(quote);
        }
        let permit = self.quote_semaphore.clone().acquire_owned().await.ok()?;
        let quote = tokio::time::timeout(
            Duration::from_secs(12),
            provider.quote(source_currency, target_currency, source_amount),
        )
        .await
        .ok()?
        .ok()?;
        drop(permit);
        if let Ok(mut cache) = self.fiat_quote_cache.write() {
            cache.insert(
                key,
                CachedFiatQuote {
                    inserted_at: Instant::now(),
                    quote: quote.clone(),
                },
            );
        }
        Some(quote)
    }

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
            let result =
                tokio::time::timeout(Duration::from_secs(12), provider.quote(from, to, amount))
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

    pub(in crate::p2p) fn refresh_fiat_quote(
        &self,
        key: String,
        provider: Arc<dyn crate::p2p::PublicFiatRouteProvider>,
        source_currency: String,
        target_currency: String,
        source_amount: f64,
    ) {
        let refresh_key = key.clone();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let cache = self.fiat_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        let quote_semaphore = self.quote_semaphore.clone();
        let Ok(permit) = quote_semaphore.try_acquire_owned() else {
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };
        tokio::spawn(async move {
            let quote = tokio::time::timeout(
                Duration::from_secs(12),
                provider.quote(&source_currency, &target_currency, source_amount),
            )
            .await
            .ok()
            .and_then(Result::ok);
            drop(permit);
            if let Some(quote) = quote {
                if let Ok(mut cache) = cache.write() {
                    cache.insert(
                        key,
                        CachedFiatQuote {
                            inserted_at: Instant::now(),
                            quote,
                        },
                    );
                }
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }
}

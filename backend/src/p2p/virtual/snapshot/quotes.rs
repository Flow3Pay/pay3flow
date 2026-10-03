use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;

use crate::p2p::routes::{FiatProviderQuoteJob, PROVIDER_QUOTE_CACHE_TTL};
use crate::p2p::service::{CachedFiatQuote, CachedProviderQuote};
use crate::p2p::P2pSearchService;
use crate::route_engine::{Amount, Asset, PublicRouteQuote};

/// Ceiling for a single external provider quote. Providers normally answer in
/// seconds; this only bounds a provider that never responds.
const QUOTE_TIMEOUT_SECS: u64 = 12;

impl P2pSearchService {
    pub(in crate::p2p) fn indicative_provider_quotes(
        &self,
        provider: &str,
        from: &Asset,
        to: &Asset,
        amount: &Amount,
    ) -> Vec<PublicRouteQuote> {
        let key = format!("{provider}|{from}|{to}");
        let Ok(cache) = self.provider_quote_snapshots.read() else {
            return Vec::new();
        };
        let Some((updated_at, quotes)) = cache.get(&key) else {
            return Vec::new();
        };
        if updated_at.elapsed() > Duration::from_secs(15 * 60) {
            return Vec::new();
        }
        let Ok(requested) = amount.value.parse::<f64>() else {
            return Vec::new();
        };
        quotes
            .iter()
            .filter_map(|quote| {
                let input = quote.input.value.parse::<f64>().ok()?;
                let output = quote.output.value.parse::<f64>().ok()?;
                if !input.is_finite() || input <= 0.0 || !output.is_finite() || output <= 0.0 {
                    return None;
                }
                let mut indicative = quote.clone();
                indicative.input = amount.clone();
                indicative.output =
                    Amount::from_f64(requested * output / input, to.clone()).ok()?;
                indicative.quote_id = None;
                indicative.expires_at = None;
                indicative.description =
                    Some("indicative background rate; confirm with provider".into());
                Some(indicative)
            })
            .collect()
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
            .filter(|cached| cached.inserted_at.elapsed() < Duration::from_secs(5 * 60))
            .map(|cached| cached.quote.clone())
    }

    pub(in crate::p2p) fn indicative_fiat_quote(
        &self,
        provider: &str,
        source: &str,
        target: &str,
        amount: f64,
    ) -> Option<crate::p2p::FiatRouteQuote> {
        let prefix = format!("fiat|{provider}|{source}|{target}|");
        let cache = self.fiat_quote_cache.read().ok()?;
        let quote = cache
            .iter()
            .filter(|(key, cached)| {
                key.starts_with(&prefix)
                    && cached.inserted_at.elapsed() < Duration::from_secs(5 * 60)
            })
            .min_by(|(_, left), (_, right)| {
                (left.quote.source_amount - amount)
                    .abs()
                    .total_cmp(&(right.quote.source_amount - amount).abs())
            })?
            .1
            .quote
            .clone();
        if !quote.source_amount.is_finite() || quote.source_amount <= 0.0 {
            return None;
        }
        let mut indicative = quote;
        indicative.target_amount = amount * indicative.target_amount / indicative.source_amount;
        indicative.source_amount = amount;
        Some(indicative)
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

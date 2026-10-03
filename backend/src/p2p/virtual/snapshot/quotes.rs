use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::redis::{get_json, set_json};
use crate::p2p::routes::{
    fiat_quote_key, provider_coefficient_key, FiatProviderQuoteJob, FIAT_COEFFICIENT_CACHE_TTL,
    PROVIDER_QUOTE_CACHE_TTL,
};
use crate::p2p::service::{CachedFiatQuote, CachedProviderCoefficient, CachedProviderQuote};
use crate::p2p::P2pSearchService;
use crate::route_engine::{Amount, Asset, PublicRouteProvider, PublicRouteQuote};

const BACKGROUND_FIAT_REFERENCE_AMOUNT: f64 = 100.0;
const FIAT_COEFFICIENT_REFRESH_AFTER: Duration = Duration::from_secs(30 * 60);
const BACKGROUND_ROUTE_REFERENCE_AMOUNT: f64 = 1.0;
const PROVIDER_COEFFICIENT_REFRESH_AFTER: Duration = Duration::from_secs(30 * 60);
const PROVIDER_COEFFICIENT_TTL: Duration = Duration::from_secs(35 * 60);
const MAX_PROVIDER_COEFFICIENTS: usize = 50_000;

#[derive(Serialize, Deserialize)]
struct PersistedCoefficient {
    observed_at: DateTime<Utc>,
    output_per_input: f64,
}

#[derive(Serialize, Deserialize)]
struct PersistedFiatCoefficient {
    observed_at: DateTime<Utc>,
    provider: String,
    source_url: String,
    source_currency: String,
    target_currency: String,
    target_per_source: f64,
}

impl P2pSearchService {
    pub(in crate::p2p) fn mark_background_provider_coefficient_failed(
        &self,
        provider: &str,
        from: &Asset,
        to: &Asset,
    ) {
        let key = provider_coefficient_key(provider, from, to);
        if let Ok(mut cache) = self.provider_coefficient_cache.write() {
            cache.insert(
                key,
                CachedProviderCoefficient {
                    inserted_at: Instant::now(),
                    output_per_input: None,
                },
            );
        }
    }

    pub(in crate::p2p) fn background_provider_coefficient_is_due(
        &self,
        provider: &str,
        from: &Asset,
        to: &Asset,
    ) -> bool {
        let key = provider_coefficient_key(provider, from, to);
        self.provider_coefficient_cache
            .read()
            .ok()
            .and_then(|cache| {
                cache.get(&key).map(|cached| {
                    cached.inserted_at.elapsed() >= PROVIDER_COEFFICIENT_REFRESH_AFTER
                })
            })
            .unwrap_or(true)
    }

    pub(in crate::p2p) async fn refresh_background_provider_coefficient(
        &self,
        provider: Arc<dyn PublicRouteProvider>,
        from: Asset,
        to: Asset,
    ) -> Result<()> {
        let provider_name = provider.name().to_string();
        let key = provider_coefficient_key(&provider_name, &from, &to);
        let input = Amount::from_f64(BACKGROUND_ROUTE_REFERENCE_AMOUNT, from.clone())?;
        let _permit = self.quote_semaphore.clone().acquire_owned().await?;
        let quote = tokio::time::timeout(
            Duration::from_secs(12),
            provider.quote(from.clone(), to.clone(), input.clone()),
        )
        .await??;
        if quote.from != from
            || quote.to != to
            || quote.input.asset != from
            || quote.output.asset != to
        {
            anyhow::bail!("provider returned a quote for a different asset pair");
        }
        let input_amount = quote.input.value.parse::<f64>()?;
        let output_amount = quote.output.value.parse::<f64>()?;
        let output_per_input = output_amount / input_amount;
        if !input_amount.is_finite()
            || input_amount <= 0.0
            || !output_per_input.is_finite()
            || output_per_input <= 0.0
        {
            anyhow::bail!("provider returned an invalid exchange coefficient");
        }
        {
            let mut cache = self
                .provider_coefficient_cache
                .write()
                .map_err(|_| anyhow::anyhow!("provider coefficient cache lock poisoned"))?;
            cache.retain(|_, cached| cached.inserted_at.elapsed() < PROVIDER_COEFFICIENT_TTL);
            if cache.len() >= MAX_PROVIDER_COEFFICIENTS && !cache.contains_key(&key) {
                if let Some(oldest_key) = cache
                    .iter()
                    .min_by_key(|(_, cached)| cached.inserted_at)
                    .map(|(key, _)| key.clone())
                {
                    cache.remove(&oldest_key);
                }
            }
            cache.insert(
                key.clone(),
                CachedProviderCoefficient {
                    inserted_at: Instant::now(),
                    output_per_input: Some(output_per_input),
                },
            );
        }
        if let Some(redis) = self.redis.as_ref() {
            let snapshot = PersistedCoefficient {
                observed_at: Utc::now(),
                output_per_input,
            };
            if let Err(error) = tokio::time::timeout(
                Duration::from_millis(100),
                set_json(
                    redis,
                    &redis_coefficient_key(&key),
                    &snapshot,
                    PROVIDER_COEFFICIENT_TTL.as_secs(),
                ),
            )
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Redis write timed out")))
            {
                tracing::debug!(%error, provider = %provider_name, %from, %to, "failed to persist route provider coefficient");
            }
        }
        tracing::info!(
            provider = %provider_name,
            %from,
            %to,
            "background route provider coefficient refreshed"
        );
        Ok(())
    }

    pub(in crate::p2p) async fn cached_provider_coefficient_quote(
        &self,
        provider: &str,
        from: &Asset,
        to: &Asset,
        input: &Amount,
    ) -> Option<PublicRouteQuote> {
        let key = provider_coefficient_key(provider, from, to);
        let cached_coefficient = self
            .provider_coefficient_cache
            .read()
            .ok()
            .and_then(|cache| {
                cache
                    .get(&key)
                    .filter(|cached| cached.inserted_at.elapsed() < PROVIDER_COEFFICIENT_TTL)
                    .and_then(|cached| cached.output_per_input)
            });
        let coefficient = if let Some(coefficient) = cached_coefficient {
            coefficient
        } else {
            let redis = self.redis.as_ref()?;
            let persisted = tokio::time::timeout(
                Duration::from_millis(100),
                get_json::<PersistedCoefficient>(redis, &redis_coefficient_key(&key)),
            )
            .await
            .ok()?
            .ok()??;
            let age = Utc::now()
                .signed_duration_since(persisted.observed_at)
                .to_std()
                .ok()?;
            if age >= PROVIDER_COEFFICIENT_TTL
                || !persisted.output_per_input.is_finite()
                || persisted.output_per_input <= 0.0
            {
                return None;
            }
            let mut cache = self.provider_coefficient_cache.write().ok()?;
            cache.insert(
                key.clone(),
                CachedProviderCoefficient {
                    inserted_at: Instant::now().checked_sub(age)?,
                    output_per_input: Some(persisted.output_per_input),
                },
            );
            persisted.output_per_input
        };
        let input_value = input.value.parse::<f64>().ok()?;
        let output_value = input_value * coefficient;
        if !input_value.is_finite() || input_value <= 0.0 || !output_value.is_finite() {
            return None;
        }
        let output = Amount::new(output_value.to_string(), to.clone()).ok()?;
        Some(PublicRouteQuote {
            provider: provider.into(),
            quote_id: None,
            description: Some("background exchange coefficient estimate".into()),
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: input.clone(),
            output,
            fees: Vec::new(),
            expires_at: None,
            path: vec![from.clone(), to.clone()],
        })
    }

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
        if let Some(quote) = self.cached_fiat_quote(&key, source_amount).await {
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
                key.clone(),
                CachedFiatQuote {
                    inserted_at: Instant::now(),
                    quote: quote.clone(),
                },
            );
        }
        if let Some(redis) = self.redis.clone() {
            let snapshot = PersistedFiatCoefficient {
                observed_at: Utc::now(),
                provider: quote.provider.clone(),
                source_url: quote.source_url.clone(),
                source_currency: quote.source_currency.clone(),
                target_currency: quote.target_currency.clone(),
                target_per_source: quote.target_amount / quote.source_amount,
            };
            let redis_key = redis_fiat_coefficient_key(&key);
            tokio::spawn(async move {
                if let Err(error) = set_json(
                    &redis,
                    &redis_key,
                    &snapshot,
                    FIAT_COEFFICIENT_CACHE_TTL.as_secs(),
                )
                .await
                {
                    tracing::debug!(%error, "failed to persist live fiat exchange coefficient");
                }
            });
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

        let Some(background_permit) = self.try_start_background_pipeline() else {
            if let Ok(mut refreshes) = self.quote_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };

        let provider = job.provider.clone();
        let from = job.from.clone();
        let to = job.to.clone();
        let amount = job.amount.clone();
        let cache = self.provider_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        tokio::spawn(async move {
            let _background_permit = background_permit;
            let provider_name = provider.name().to_string();
            let result =
                tokio::time::timeout(Duration::from_secs(12), provider.quote(from, to, amount))
                    .await;
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

    pub(in crate::p2p) async fn cached_fiat_quote(
        &self,
        key: &str,
        source_amount: f64,
    ) -> Option<crate::p2p::FiatRouteQuote> {
        let cached_quote = self.fiat_quote_cache.read().ok().and_then(|cache| {
            cache
                .get(key)
                .filter(|cached| cached.inserted_at.elapsed() < FIAT_COEFFICIENT_CACHE_TTL)
                .map(|cached| cached.quote.clone())
        });
        if let Some(quote) = cached_quote {
            return scale_fiat_quote(&quote, source_amount);
        }

        let redis = self.redis.as_ref()?;
        let persisted = tokio::time::timeout(
            Duration::from_millis(100),
            get_json::<PersistedFiatCoefficient>(redis, &redis_fiat_coefficient_key(key)),
        )
        .await
        .ok()?
        .ok()??;
        let age = Utc::now()
            .signed_duration_since(persisted.observed_at)
            .to_std()
            .ok()?;
        if age >= FIAT_COEFFICIENT_CACHE_TTL
            || !persisted.target_per_source.is_finite()
            || persisted.target_per_source <= 0.0
        {
            return None;
        }
        let quote = crate::p2p::FiatRouteQuote {
            provider: persisted.provider,
            source_url: persisted.source_url,
            source_currency: persisted.source_currency,
            target_currency: persisted.target_currency,
            source_amount: BACKGROUND_FIAT_REFERENCE_AMOUNT,
            target_amount: BACKGROUND_FIAT_REFERENCE_AMOUNT * persisted.target_per_source,
        };
        self.fiat_quote_cache.write().ok()?.insert(
            key.to_string(),
            CachedFiatQuote {
                inserted_at: Instant::now().checked_sub(age)?,
                quote: quote.clone(),
            },
        );
        scale_fiat_quote(&quote, source_amount)
    }

    pub(in crate::p2p) fn background_fiat_quote_is_due(
        &self,
        provider: &str,
        source_currency: &str,
        target_currency: &str,
    ) -> bool {
        let key = fiat_quote_key(provider, source_currency, target_currency, 0.0);
        self.fiat_quote_cache
            .read()
            .ok()
            .and_then(|cache| {
                cache
                    .get(&key)
                    .map(|cached| cached.inserted_at.elapsed() >= FIAT_COEFFICIENT_REFRESH_AFTER)
            })
            .unwrap_or(true)
    }

    pub(in crate::p2p) async fn refresh_background_fiat_quote(
        &self,
        provider: Arc<dyn crate::p2p::PublicFiatRouteProvider>,
        source_currency: String,
        target_currency: String,
    ) -> Result<()> {
        let key = fiat_quote_key(
            provider.name(),
            &source_currency,
            &target_currency,
            BACKGROUND_FIAT_REFERENCE_AMOUNT,
        );
        let _permit = self.quote_semaphore.clone().acquire_owned().await?;
        let quote = tokio::time::timeout(
            Duration::from_secs(12),
            provider.quote(
                &source_currency,
                &target_currency,
                BACKGROUND_FIAT_REFERENCE_AMOUNT,
            ),
        )
        .await??;
        if !quote.source_amount.is_finite()
            || quote.source_amount <= 0.0
            || !quote.target_amount.is_finite()
            || quote.target_amount <= 0.0
            || !quote.source_currency.eq_ignore_ascii_case(&source_currency)
            || !quote.target_currency.eq_ignore_ascii_case(&target_currency)
        {
            anyhow::bail!("provider returned an invalid fiat exchange coefficient");
        }
        let redis_key = redis_fiat_coefficient_key(&key);
        let persisted = PersistedFiatCoefficient {
            observed_at: Utc::now(),
            provider: quote.provider.clone(),
            source_url: quote.source_url.clone(),
            source_currency: quote.source_currency.clone(),
            target_currency: quote.target_currency.clone(),
            target_per_source: quote.target_amount / quote.source_amount,
        };
        {
            let mut cache = self
                .fiat_quote_cache
                .write()
                .map_err(|_| anyhow::anyhow!("fiat quote cache lock poisoned"))?;
            cache.insert(
                key.clone(),
                CachedFiatQuote {
                    inserted_at: Instant::now(),
                    quote,
                },
            );
        }
        if let Some(redis) = self.redis.as_ref() {
            if let Err(error) = tokio::time::timeout(
                Duration::from_millis(100),
                set_json(
                    redis,
                    &redis_key,
                    &persisted,
                    FIAT_COEFFICIENT_CACHE_TTL.as_secs(),
                ),
            )
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Redis write timed out")))
            {
                tracing::debug!(%error, %source_currency, %target_currency, "failed to persist fiat exchange coefficient");
            }
        }
        tracing::info!(
            provider = %provider.name(),
            %source_currency,
            %target_currency,
            "background fiat exchange coefficient refreshed"
        );
        Ok(())
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

        let Some(background_permit) = self.try_start_background_pipeline() else {
            if let Ok(mut refreshes) = self.quote_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
            return;
        };

        let cache = self.fiat_quote_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        let redis = self.redis.clone();
        tokio::spawn(async move {
            let _background_permit = background_permit;
            let quote = tokio::time::timeout(
                Duration::from_secs(12),
                provider.quote(&source_currency, &target_currency, source_amount),
            )
            .await
            .ok()
            .and_then(Result::ok);
            if let Some(quote) = quote {
                if let Ok(mut cache) = cache.write() {
                    cache.insert(
                        key.clone(),
                        CachedFiatQuote {
                            inserted_at: Instant::now(),
                            quote: quote.clone(),
                        },
                    );
                }
                if let Some(redis) = redis {
                    let persisted = PersistedFiatCoefficient {
                        observed_at: Utc::now(),
                        provider: quote.provider,
                        source_url: quote.source_url,
                        source_currency: quote.source_currency,
                        target_currency: quote.target_currency,
                        target_per_source: quote.target_amount / quote.source_amount,
                    };
                    if let Err(error) = set_json(
                        &redis,
                        &redis_fiat_coefficient_key(&key),
                        &persisted,
                        FIAT_COEFFICIENT_CACHE_TTL.as_secs(),
                    )
                    .await
                    {
                        tracing::debug!(%error, "failed to persist refreshed fiat exchange coefficient");
                    }
                }
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }
}

fn redis_coefficient_key(key: &str) -> String {
    format!("pay3flow:route-coefficient:v1:{key}")
}

fn redis_fiat_coefficient_key(key: &str) -> String {
    format!("pay3flow:fiat-coefficient:v1:{key}")
}

fn scale_fiat_quote(
    quote: &crate::p2p::FiatRouteQuote,
    source_amount: f64,
) -> Option<crate::p2p::FiatRouteQuote> {
    if !source_amount.is_finite() || source_amount <= 0.0 {
        return None;
    }
    let coefficient = quote.target_amount / quote.source_amount;
    let target_amount = source_amount * coefficient;
    (coefficient.is_finite()
        && coefficient > 0.0
        && target_amount.is_finite()
        && target_amount > 0.0)
        .then(|| crate::p2p::FiatRouteQuote {
            source_amount,
            target_amount,
            ..quote.clone()
        })
}

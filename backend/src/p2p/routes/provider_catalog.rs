use super::*;

impl P2pSearchService {
    pub(super) async fn provider_assets(&self) -> Vec<Asset> {
        if self.has_fmatch_backend() {
            self.warm_provider_capabilities();
            return self.cached_provider_assets();
        }
        Self::provider_assets_for(&self.route_providers).await
    }

    pub(super) async fn provider_capabilities(
        providers: &[Arc<dyn PublicRouteProvider>],
    ) -> Arc<[RouteProviderCapability]> {
        let mut capabilities = Vec::with_capacity(providers.len());
        for provider in providers {
            capabilities.push(RouteProviderCapability {
                provider: provider.clone(),
                assets: provider.supported_assets().await.into_iter().collect(),
            });
        }
        capabilities.sort_by_key(|capability| provider_priority(capability.provider.name()));
        capabilities.into()
    }

    pub(crate) fn warm_provider_capabilities(&self) {
        if self
            .provider_capabilities_cache
            .read()
            .is_ok_and(|cache| cache.is_some())
        {
            return;
        }
        let refresh_key = "provider-capabilities".to_string();
        let Ok(mut refreshes) = self.quote_refreshes.lock() else {
            return;
        };
        if !refreshes.insert(refresh_key.clone()) {
            return;
        }
        drop(refreshes);

        let providers = self.route_providers.clone();
        let cache = self.provider_capabilities_cache.clone();
        let active_refreshes = self.quote_refreshes.clone();
        tokio::spawn(async move {
            let mut loads = providers
                .iter()
                .cloned()
                .map(|provider| async move {
                    let name = provider.name().to_string();
                    let assets = provider.supported_assets().await.into_iter().collect();
                    (name, assets)
                })
                .collect::<FuturesUnordered<_>>();
            let mut snapshot = HashMap::new();
            while let Some((name, assets)) = loads.next().await {
                snapshot.insert(name, assets);
            }
            if let Ok(mut cache) = cache.write() {
                *cache = Some(snapshot);
            }
            if let Ok(mut refreshes) = active_refreshes.lock() {
                refreshes.remove(&refresh_key);
            }
        });
    }

    pub(super) fn cached_provider_assets(&self) -> Vec<Asset> {
        let mut assets = self
            .provider_capabilities_cache
            .read()
            .ok()
            .and_then(|cache| cache.clone())
            .into_iter()
            .flat_map(|snapshot| snapshot.into_values().flatten())
            .collect::<Vec<_>>();
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    pub(super) async fn provider_capabilities_for_query(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Arc<[RouteProviderCapability]> {
        let providers = self.route_providers_for_query(query);
        if !self.has_fmatch_backend() {
            return Self::provider_capabilities(&providers).await;
        }
        self.warm_provider_capabilities();
        let snapshot = self
            .provider_capabilities_cache
            .read()
            .ok()
            .and_then(|cache| cache.clone());
        let Some(snapshot) = snapshot else {
            return Vec::new().into();
        };
        let mut capabilities = providers
            .iter()
            .filter_map(|provider| {
                snapshot
                    .get(provider.name())
                    .cloned()
                    .map(|assets| RouteProviderCapability {
                        provider: provider.clone(),
                        assets,
                    })
            })
            .collect::<Vec<_>>();
        capabilities.sort_by_key(|capability| provider_priority(capability.provider.name()));
        capabilities.into()
    }

    pub(super) fn cached_provider_quote(&self, key: &str) -> Option<PublicRouteQuote> {
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

    pub(super) fn refresh_provider_quote(&self, key: String, job: &FiatProviderQuoteJob) {
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

    pub(super) fn cached_fiat_quote(&self, key: &str) -> Option<crate::p2p::FiatRouteQuote> {
        self.fiat_quote_cache
            .read()
            .ok()?
            .get(key)
            .filter(|cached| cached.inserted_at.elapsed() < PROVIDER_QUOTE_CACHE_TTL)
            .map(|cached| cached.quote.clone())
    }

    pub(super) fn refresh_fiat_quote(
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

    pub(super) async fn provider_assets_for(
        providers: &[Arc<dyn PublicRouteProvider>],
    ) -> Vec<Asset> {
        let mut assets = Vec::new();
        for provider in providers {
            assets.extend(provider.supported_assets().await);
        }
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }

    pub(super) fn route_providers_for_query(
        &self,
        query: &NormalizedRouteQuery,
    ) -> Arc<[Arc<dyn PublicRouteProvider>]> {
        self.route_providers
            .iter()
            .filter(|provider| source_selected(query, provider.name()))
            .cloned()
            .collect::<Vec<_>>()
            .into()
    }

    pub(super) async fn intermediary_assets(&self, query: &NormalizedRouteQuery) -> Vec<Asset> {
        let providers = self.route_providers_for_query(query);
        let provider_assets = if self.has_fmatch_backend() {
            self.warm_provider_capabilities();
            self.cached_provider_assets()
        } else {
            Self::provider_assets_for(&providers).await
        };
        let allowed_symbols = query.assets.iter().collect::<HashSet<_>>();
        let mut assets = if query.assets_explicit {
            provider_assets
                .into_iter()
                .filter(|asset| allowed_symbols.contains(&asset.symbol))
                .collect::<Vec<_>>()
        } else {
            provider_assets
        };
        if assets.is_empty() {
            assets = query
                .assets
                .iter()
                .flat_map(|symbol| {
                    self.networks
                        .compatible_networks(symbol)
                        .into_iter()
                        .filter_map(move |network| Asset::new(symbol, Some(&network.id)).ok())
                })
                .collect();
        }
        let target_symbol = query.target_currency.as_str();
        assets.sort_by(|left, right| {
            intermediary_asset_priority(&left.symbol, target_symbol)
                .cmp(&intermediary_asset_priority(&right.symbol, target_symbol))
                .then_with(|| left.symbol.cmp(&right.symbol))
                .then_with(|| left.to_string().cmp(&right.to_string()))
        });
        assets.dedup();
        assets.truncate(MAX_PROVIDER_ASSETS);
        assets
    }

    pub(super) fn assets_on_network(
        &self,
        symbol: &str,
        network: Option<&str>,
        provider_assets: &[Asset],
    ) -> Vec<Asset> {
        let requested_network = network.map(canonical_network_id);
        let mut assets = provider_assets
            .iter()
            .filter(|asset| asset.symbol.eq_ignore_ascii_case(symbol))
            .filter(|asset| {
                requested_network
                    .as_deref()
                    .is_none_or(|network| asset.location.as_deref() == Some(network))
            })
            .cloned()
            .collect::<Vec<_>>();
        if assets.is_empty() {
            assets = self
                .networks
                .compatible_networks(symbol)
                .into_iter()
                .filter(|candidate| {
                    requested_network
                        .as_deref()
                        .is_none_or(|network| candidate.id == network)
                })
                .filter_map(|candidate| Asset::new(symbol, Some(&candidate.id)).ok())
                .collect();
        }
        assets.sort_by_key(|asset| asset.to_string());
        assets.dedup();
        assets
    }
}

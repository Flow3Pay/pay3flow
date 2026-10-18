use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::networks::{self, CryptoNetwork};
use crate::p2p::P2pSearchService;

#[derive(Debug, Default, Deserialize)]
pub struct NetworkFilters {
    pub currency: Option<String>,
}

/// `GET /api/networks` — networks available for crypto assets.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<NetworkFilters>,
) -> Result<Json<Vec<CryptoNetwork>>, AppError> {
    let catalog = networks::list(&state.pool, None)
        .await
        .map_err(AppError::from)?;
    let mut catalog = provider_catalog(catalog, &state.p2p).await;
    if let Some(currency) = filters.currency.as_deref() {
        catalog.retain(|network| {
            network
                .currencies
                .iter()
                .any(|value| value.eq_ignore_ascii_case(currency.trim()))
        });
    }
    Ok(Json(catalog))
}

async fn provider_catalog(
    mut catalog: Vec<CryptoNetwork>,
    service: &P2pSearchService,
) -> Vec<CryptoNetwork> {
    let assets = P2pSearchService::provider_assets_for(&service.route_providers).await;
    for asset in assets {
        let Some(network) = catalog.iter_mut().find(|network| {
            network
                .id
                .eq_ignore_ascii_case(asset.location.as_deref().unwrap_or_default())
        }) else {
            continue;
        };
        if !network
            .currencies
            .iter()
            .any(|currency| currency.eq_ignore_ascii_case(&asset.symbol))
        {
            network.currencies.push(asset.symbol);
            network.currencies.sort();
        }
    }
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route_engine::{Amount, Asset, PublicRouteProvider, PublicRouteQuote};
    use std::sync::Arc;
    use std::time::Duration;

    struct CatalogProvider {
        name: &'static str,
        assets: Vec<Asset>,
    }

    #[async_trait::async_trait]
    impl PublicRouteProvider for CatalogProvider {
        fn name(&self) -> &str {
            self.name
        }
        async fn supported_assets(&self) -> Vec<Asset> {
            self.assets.clone()
        }
        async fn quote(&self, _: Asset, _: Asset, _: Amount) -> anyhow::Result<PublicRouteQuote> {
            anyhow::bail!("catalog-only fixture")
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn network_picker_includes_assets_from_every_provider_without_duplicates() {
        let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
            .with_route_providers(vec![
                Arc::new(CatalogProvider {
                    name: "near-intents",
                    assets: vec![Asset::parse("TON@ton").unwrap()],
                }),
                Arc::new(CatalogProvider {
                    name: "another-provider",
                    assets: vec![
                        Asset::parse("GRAM@ethereum").unwrap(),
                        Asset::parse("TON@ton").unwrap(),
                        Asset::parse("NEWCOIN@ethereum").unwrap(),
                    ],
                }),
            ]);
        let catalog = provider_catalog(
            vec![
                CryptoNetwork {
                    id: "ethereum".into(),
                    name: "Ethereum".into(),
                    currencies: vec!["ETH".into()],
                },
                CryptoNetwork {
                    id: "ton".into(),
                    name: "TON".into(),
                    currencies: vec!["GRAM".into()],
                },
            ],
            &service,
        )
        .await;
        assert_eq!(catalog[0].currencies, ["ETH", "GRAM", "NEWCOIN"]);
        assert_eq!(catalog[1].currencies, ["GRAM"]);
    }
}

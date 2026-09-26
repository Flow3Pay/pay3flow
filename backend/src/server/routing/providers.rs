use axum::extract::{Query, State};
use axum::Json;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::providers::{Provider, ProviderFilters, ProviderSearchMode};

fn search_mode(
    slug: &str,
    selectable: &std::collections::HashSet<String>,
    route_providers: &std::collections::HashSet<String>,
) -> ProviderSearchMode {
    if selectable.contains(slug) || route_providers.contains(slug) {
        ProviderSearchMode::Selectable
    } else {
        ProviderSearchMode::CatalogOnly
    }
}

/// `GET /api/providers` returns database-backed Providerfile definitions.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<ProviderFilters>,
) -> Result<Json<Vec<Provider>>, AppError> {
    let searchable = state.p2p.searchable_sources();
    let route_providers = state.p2p.route_provider_names();
    crate::providers::list(&state.pool, filters)
        .await
        .map(|mut providers| {
            providers.iter_mut().for_each(|provider| {
                provider.search_mode = search_mode(&provider.slug, &searchable, &route_providers);
                provider.searchable = provider.search_mode != ProviderSearchMode::CatalogOnly;
            });
            providers
        })
        .map(Json)
        .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn route_providers_are_selectable_sources() {
        let selectable = HashSet::from(["cow-swap".to_string(), "binance".to_string()]);
        let route_providers = HashSet::from([
            "cow-swap".to_string(),
            "near-intents".to_string(),
            "id-pay".to_string(),
        ]);

        assert_eq!(
            search_mode("cow-swap", &selectable, &route_providers),
            ProviderSearchMode::Selectable
        );
        assert_eq!(
            search_mode("near-intents", &selectable, &route_providers),
            ProviderSearchMode::Selectable
        );
        assert_eq!(
            search_mode("id-pay", &selectable, &route_providers),
            ProviderSearchMode::Selectable
        );
        assert_eq!(
            search_mode("binance", &selectable, &route_providers),
            ProviderSearchMode::Selectable
        );
        assert_eq!(
            search_mode("whitebird", &selectable, &route_providers),
            ProviderSearchMode::CatalogOnly
        );
    }
}

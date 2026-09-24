use axum::extract::{Query, State};
use axum::Json;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::providers::{Provider, ProviderFilters};

/// `GET /api/providers` returns database-backed Providerfile definitions.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<ProviderFilters>,
) -> Result<Json<Vec<Provider>>, AppError> {
    let searchable = state.p2p.searchable_sources();
    crate::providers::list(&state.pool, filters)
        .await
        .map(|mut providers| {
            providers.iter_mut().for_each(|provider| {
                provider.searchable = searchable.contains(provider.slug.as_str());
            });
            providers
        })
        .map(Json)
        .map_err(AppError::from)
}

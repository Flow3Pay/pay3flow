use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::networks::{self, CryptoNetwork};
use crate::route_engine::PublicRouteProvider;

#[derive(Debug, Default, Deserialize)]
pub struct NetworkFilters {
    pub currency: Option<String>,
}

/// `GET /api/networks` — networks available for crypto assets.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<NetworkFilters>,
) -> Result<Json<Vec<CryptoNetwork>>, AppError> {
    let mut catalog = networks::list(&state.pool, None)
        .await
        .map_err(AppError::from)?;
    for asset in state.near_intents.supported_assets().await {
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

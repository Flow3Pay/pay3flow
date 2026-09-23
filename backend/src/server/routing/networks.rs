use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::networks::{self, CryptoNetwork};

#[derive(Debug, Default, Deserialize)]
pub struct NetworkFilters {
    pub currency: Option<String>,
}

/// `GET /api/networks` — networks available for crypto assets.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<NetworkFilters>,
) -> Result<Json<Vec<CryptoNetwork>>, AppError> {
    networks::list(&state.pool, filters.currency.as_deref())
        .await
        .map(Json)
        .map_err(AppError::from)
}

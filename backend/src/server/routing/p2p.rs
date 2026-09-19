use axum::extract::{Query, State};
use axum::Json;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::p2p::{P2pRouteSearchQuery, P2pRouteSearchResponse, P2pSearchQuery, P2pSearchResponse};

/// Read-only live search over configured public P2P advertisement sources.
/// This endpoint never contacts advertisers, creates exchange orders, or places P2P orders.
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<P2pSearchQuery>,
) -> Result<Json<P2pSearchResponse>, AppError> {
    state
        .p2p
        .search(query)
        .await
        .map(Json)
        .map_err(|error| AppError::BadRequest(error.to_string()))
}

/// Build complete `source fiat -> asset -> target fiat` P2P routes and rank them
/// by estimated target amount. Search only: no contact or trade is created.
pub async fn routes(
    State(state): State<AppState>,
    Query(query): Query<P2pRouteSearchQuery>,
) -> Result<Json<P2pRouteSearchResponse>, AppError> {
    state
        .p2p
        .search_routes(query)
        .await
        .map(Json)
        .map_err(|error| AppError::BadRequest(error.to_string()))
}

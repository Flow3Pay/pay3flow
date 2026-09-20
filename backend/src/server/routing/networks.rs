use axum::extract::Query;
use axum::Json;
use serde::Deserialize;

use crate::networks::{self, CryptoNetwork};

#[derive(Debug, Default, Deserialize)]
pub struct NetworkFilters {
    pub currency: Option<String>,
}

/// `GET /api/networks` — networks available for crypto assets.
pub async fn list(Query(filters): Query<NetworkFilters>) -> Json<Vec<CryptoNetwork>> {
    Json(networks::for_currency(filters.currency.as_deref()))
}

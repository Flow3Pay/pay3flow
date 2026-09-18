use axum::extract::{Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::pairs::{ExchangePair, NewPair, PairFilters, PairPatch};

/// `GET /api/exchange-pairs` — the public bank-pair router catalog (PLAN 46c).
/// Only `enabled` pairs are served; supported filters: `country`, `currency`
/// (either leg), `scheme` (either leg), `from_scheme`, `to_scheme`. Responses
/// are cached in memory for the configured TTL. No auth: the catalog is the
/// source of truth for the payment form's "from → to" picker.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<PairFilters>,
) -> Result<Json<Vec<ExchangePair>>, AppError> {
    let pairs = state.pairs.list(&filters).await.map_err(AppError::from)?;
    Ok(Json(pairs))
}

/// `POST /api/admin/exchange-pairs` — add (or upsert by route key) a pair
/// (PLAN 46e: "добавить связку"). Guarded by the admin bearer token.
pub async fn admin_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<NewPair>,
) -> Result<Json<ExchangePair>, AppError> {
    auth_admin(&state, &headers)?;
    Ok(Json(
        state.pairs.create(&body).await.map_err(AppError::from)?,
    ))
}

/// `POST /api/admin/exchange-pairs/:id` — toggle/fix an existing pair: `status`
/// ("enabled"/"disabled"), banks, icons, country, currencies or the daily
/// limit. Partial payload: only provided fields are written. Guarded by the
/// admin bearer token.
pub async fn admin_update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(patch): Json<PairPatch>,
) -> Result<Json<ExchangePair>, AppError> {
    auth_admin(&state, &headers)?;
    let pair = state
        .pairs
        .update(id, &patch)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange pair not found".into()))?;
    Ok(Json(pair))
}

pub(crate) fn auth_admin(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(raw) = headers.get(AUTHORIZATION) else {
        return Err(AppError::Unauthorized("admin bearer token required".into()));
    };
    let raw = raw.to_str().unwrap_or_default();
    let token = raw
        .strip_prefix("Bearer ")
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AppError::Unauthorized("invalid authorization header".into()))?;
    if token != state.admin_token {
        return Err(AppError::Unauthorized("invalid admin token".into()));
    }
    Ok(())
}
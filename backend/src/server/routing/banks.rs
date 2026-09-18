use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;

use crate::banks::{Bank, BankFilters, BankPage, NewBank};
use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::server::routing::pairs::auth_admin;

/// `GET /api/banks` — the public payment-method directory (PLAN 2△). Only `enabled`
/// banks are served; filters: `role` (`sender`/`receiver`/`both`), `country`,
/// `currency`, `scheme`, `q` (name search), plus `limit`/`offset` pagination.
/// No auth: it is the source of truth for the swap form's method pickers.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(flatten)]
    pub filters: BankFilters,
    /// Default 30, clamped to 100.
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// `POST /api/admin/banks/:name/status` — set `enabled`/`disabled`.
#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub status: String,
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<BankPage>, AppError> {
    let limit = query.limit.unwrap_or(30).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let page = state
        .banks
        .list(&query.filters, limit, offset)
        .await
        .map_err(AppError::from)?;
    Ok(Json(page))
}

/// `POST /api/admin/banks` — add (or upsert by `name`) a bank. Guarded by the
/// admin bearer token.
pub async fn admin_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<NewBank>,
) -> Result<Json<Bank>, AppError> {
    auth_admin(&state, &headers)?;
    Ok(Json(
        state.banks.create(&body).await.map_err(AppError::from)?,
    ))
}

/// `POST /api/admin/banks/:name/status` — toggle a bank's `status`
/// (`enabled`/`disabled`). Guarded by the admin bearer token.
pub async fn admin_set_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<Json<Bank>, AppError> {
    auth_admin(&state, &headers)?;
    if body.status != "enabled" && body.status != "disabled" {
        return Err(AppError::BadRequest("status must be 'enabled' or 'disabled'".into()));
    }
    let bank = state
        .banks
        .set_status(&name, &body.status)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("bank not found".into()))?;
    Ok(Json(bank))
}

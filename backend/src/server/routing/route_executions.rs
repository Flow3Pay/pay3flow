use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::db::DbPool;
use crate::route_execution::{
    CreateRouteExecution, RouteExecutionError, RouteExecutionView, SubmitRouteExecution,
};

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

pub async fn count_completed_last_hour(pool: &DbPool) -> anyhow::Result<i64> {
    let client = pool.get().await?;
    let row = client
        .query_one(
            "SELECT COUNT(*)::BIGINT FROM route_executions \
             WHERE status = 'completed' AND updated_at >= now() - INTERVAL '1 hour'",
            &[],
        )
        .await?;
    Ok(row.get(0))
}

#[derive(Debug, Deserialize)]
pub struct ExecutionOwner {
    anonymous_id: Uuid,
}

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRouteExecution>,
) -> Result<Json<RouteExecutionView>, AppError> {
    let key = headers
        .get(IDEMPOTENCY_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key header is required".into()))?;
    state
        .route_executions
        .create(request, key)
        .await
        .map(Json)
        .map_err(map_error)
}

pub async fn submit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<SubmitRouteExecution>,
) -> Result<Json<RouteExecutionView>, AppError> {
    state
        .route_executions
        .submit(id, request)
        .await
        .map(Json)
        .map_err(map_error)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(owner): Query<ExecutionOwner>,
) -> Result<Json<RouteExecutionView>, AppError> {
    state
        .route_executions
        .get(id, owner.anonymous_id)
        .await
        .map(Json)
        .map_err(map_error)
}

fn map_error(error: RouteExecutionError) -> AppError {
    match error {
        RouteExecutionError::NotFound => AppError::NotFound(error.to_string()),
        RouteExecutionError::Disabled => AppError::NotImplemented(error.to_string()),
        RouteExecutionError::InvalidToken
        | RouteExecutionError::UnsupportedRoute
        | RouteExecutionError::InvalidAmount
        | RouteExecutionError::InvalidSubmission => AppError::BadRequest(error.to_string()),
        RouteExecutionError::Internal(error) => AppError::Internal(error),
    }
}

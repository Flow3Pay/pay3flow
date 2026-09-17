use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::Json;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::payments::PaymentView;

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

/// `POST /api/payments` — create a payment (PLAN #33). The request body is the
/// same shape the quote pipeline consumes; the service routes, executes and
/// persists it in one step.
///
/// Idempotency (PLAN #36): pass `Idempotency-Key` and the same body twice —
/// the second call returns the first result instead of charging again.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<crate::payments::model::NewPayment>,
) -> Result<Json<PaymentView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let idem = headers
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let view = state
        .payments
        .create(user_id, body, idem)
        .await
        .map_err(AppError::from)?;
    Ok(Json(view))
}

/// `GET /api/payments` — list the caller's payments, newest first.
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PaymentView>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let views = state
        .payments
        .list_for_user(&user_id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(views))
}

/// `GET /api/payments/:id` — one payment by id (scoped to the caller).
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<PaymentView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let view = state.payments.view(&id).await.map_err(AppError::from)?;
    if view.transaction.user_id != user_id {
        return Err(AppError::NotFound("payment not found".into()));
    }
    Ok(Json(view))
}

/// Resolve the caller's user id from the Bearer token.
pub fn auth_user_id(state: &AppState, headers: &HeaderMap) -> Result<Uuid, AppError> {
    let token = bearer_token(headers)?;
    let sub = state.jwt.verify(token).map_err(|_| {
        AppError::Unauthorized("invalid or expired token".into())
    })?;
    Uuid::parse_str(&sub)
        .map_err(|_| AppError::Unauthorized("malformed token subject".into()))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, AppError> {
    let Some(raw) = headers.get(AUTHORIZATION) else {
        return Err(AppError::Unauthorized(
            "missing authorization header".into(),
        ));
    };
    let raw = raw.to_str().unwrap_or_default();
    raw.strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| AppError::Unauthorized("invalid authorization header".into()))
}

/// `POST /api/providers/:provider/webhooks` — ingest an async provider event
/// (PLAN #31). Storage is append-only: the raw payload is recorded with a
/// processing status, and a later worker can settle the linked transaction.
/// Providers currently deliver through the stub synchronously, so this is the
/// durable intake bucket rather than a live settlement path.
pub async fn webhook(
    State(state): State<AppState>,
    axum::extract::Path(provider): axum::extract::Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("webhook body must be JSON".into()))?;
    let event_id = payload
        .get("event_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let event_type = payload
        .get("event_type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let signature = headers
        .get("X-Webhook-Signature")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let row_id = crate::payments::repo::insert_webhook(
        &state.pool,
        &provider,
        event_id.as_deref(),
        event_type.as_deref(),
        &payload,
        signature.as_deref(),
    )
    .await
    .map_err(AppError::from)?;
    Ok(axum::Json(serde_json::json!({ "id": row_id, "status": "received" })))
}
//! HTTP surface for the B2B crypto routing API and the hosted payment page.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::b2b::model::{
    CreateCheckoutRequest, CreateTransferRequest, CreateWebhookRequest, QuoteRequest,
};
use crate::b2b::Error;
use crate::core::error::AppError;
use crate::core::state::AppState;

use super::payments::auth_user_id;

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

impl From<Error> for AppError {
    fn from(error: Error) -> Self {
        match error {
            Error::Invalid(message) => AppError::BadRequest(message),
            Error::Unauthorized(message) => AppError::Unauthorized(message),
            Error::NotFound => AppError::NotFound("transfer not found".into()),
            Error::Conflict(message) => AppError::Conflict(message),
            Error::Unavailable(message) => AppError::Unavailable(message),
            Error::Internal(error) => AppError::Internal(error),
        }
    }
}

fn idempotency_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get(IDEMPOTENCY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// `POST /api/v1/quotes`
pub async fn create_quote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<QuoteRequest>,
) -> Result<Json<crate::b2b::model::QuoteView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let quote = state
        .b2b
        .create_quote(user_id, &body, idempotency_key(&headers).as_deref())
        .await?;
    Ok(Json(quote))
}

/// `POST /api/v1/transfers`
pub async fn create_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateTransferRequest>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let key = idempotency_key(&headers)
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key header is required".into()))?;
    let transfer = state.b2b.create_transfer(user_id, &body, &key).await?;
    Ok(Json(transfer))
}

/// `GET /api/v1/transfers/:id`
pub async fn get_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let transfer = state.b2b.transfer(user_id, id).await?;
    Ok(Json(transfer))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `GET /api/v1/transfers`
pub async fn list_transfers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<crate::b2b::model::TransferView>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let transfers = state
        .b2b
        .transfers(user_id, query.limit.unwrap_or(20))
        .await?;
    Ok(Json(transfers))
}

/// `POST /api/v1/transfers/:id/cancel`
pub async fn cancel_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let transfer = state.b2b.cancel_transfer(user_id, id).await?;
    Ok(Json(transfer))
}

/// `POST /api/v1/payments` — quote plus a hosted checkout page.
pub async fn create_checkout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCheckoutRequest>,
) -> Result<Json<crate::b2b::model::CheckoutView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let checkout = state
        .b2b
        .create_checkout(user_id, &body, idempotency_key(&headers).as_deref())
        .await?;
    Ok(Json(checkout))
}

/// `GET /pay/:token` — the public hosted payment page.
pub async fn checkout_page(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, AppError> {
    let view = state.b2b.checkout(&token).await?;
    let qr = crate::b2b::qr::svg(&view.quote.deposit_address).unwrap_or_default();
    let html = crate::b2b::page::render(&token, &view, &qr);
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    Ok(response)
}

/// `GET /pay/:token/status` — polling endpoint used by the page.
pub async fn checkout_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<crate::b2b::model::CheckoutStatusView>, AppError> {
    Ok(Json(state.b2b.checkout(&token).await?))
}

/// `POST /pay/:token/quote` — re-quote an expired page with a new deposit
/// address.
pub async fn checkout_requote(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<crate::b2b::model::CheckoutStatusView>, AppError> {
    Ok(Json(state.b2b.requote_checkout(&token).await?))
}

/// `POST /api/b2b/webhooks`
pub async fn create_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<Json<crate::b2b::model::WebhookEndpointView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let endpoint = state.b2b.create_webhook(user_id, &body).await?;
    Ok(Json(endpoint))
}

/// `GET /api/b2b/orders` — dashboard view of the merchant's orders.
pub async fn list_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<crate::b2b::model::TransferView>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let orders = state
        .b2b
        .transfers(user_id, query.limit.unwrap_or(50))
        .await?;
    Ok(Json(orders))
}

/// `GET /api/b2b/analytics`
pub async fn analytics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::b2b::model::AnalyticsView>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    Ok(Json(state.b2b.analytics(user_id).await?))
}

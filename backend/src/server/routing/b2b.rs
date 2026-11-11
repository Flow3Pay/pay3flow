//! HTTP surface for the B2B crypto routing API and the hosted payment page.

use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::b2b::model::{
    CreateCheckoutRequest, CreateTransferRequest, CreateWebhookRequest, QuoteRequest,
};
use crate::b2b::Caller;
use crate::core::error::AppError;
use crate::core::state::AppState;

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

/// A project key is bound to one environment: a request may name that
/// environment or omit it, but it can never switch to the other one (§8.1).
/// The legacy user token has no environment, so the request keeps its own.
fn pin_env(env: &mut Option<String>, caller: &Caller) -> Result<(), AppError> {
    let Some(key_env) = caller.env else {
        return Ok(());
    };
    if let Some(requested) = env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !requested.eq_ignore_ascii_case(key_env.as_str()) {
            return Err(AppError::Forbidden(format!(
                "environment_mismatch: key is `{}`",
                key_env.as_str()
            )));
        }
    }
    *env = Some(key_env.as_str().to_string());
    Ok(())
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
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    Json(mut body): Json<QuoteRequest>,
) -> Result<Json<crate::b2b::model::QuoteView>, AppError> {
    pin_env(&mut body.env, &caller)?;
    let quote = state
        .b2b
        .create_quote(caller, &body, idempotency_key(&headers).as_deref())
        .await?;
    Ok(Json(quote))
}

/// `POST /api/v1/transfers`
pub async fn create_transfer(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    Json(mut body): Json<CreateTransferRequest>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    pin_env(&mut body.env, &caller)?;
    let key = idempotency_key(&headers)
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key header is required".into()))?;
    let transfer = state.b2b.create_transfer(caller, &body, &key).await?;
    Ok(Json(transfer))
}

/// `GET /api/v1/transfers/:id`
pub async fn get_transfer(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    let transfer = state.b2b.transfer(caller, id).await?;
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
    Extension(caller): Extension<Caller>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<crate::b2b::model::TransferView>>, AppError> {
    let transfers = state
        .b2b
        .transfers(caller, query.limit.unwrap_or(20))
        .await?;
    Ok(Json(transfers))
}

/// `POST /api/v1/transfers/:id/cancel`
pub async fn cancel_transfer(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::b2b::model::TransferView>, AppError> {
    let transfer = state.b2b.cancel_transfer(caller, id).await?;
    Ok(Json(transfer))
}

/// `POST /api/v1/payments` — quote plus a hosted checkout page.
pub async fn create_checkout(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    headers: HeaderMap,
    Json(mut body): Json<CreateCheckoutRequest>,
) -> Result<Json<crate::b2b::model::CheckoutView>, AppError> {
    pin_env(&mut body.quote.env, &caller)?;
    let checkout = state
        .b2b
        .create_checkout(caller, &body, idempotency_key(&headers).as_deref())
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
    Extension(caller): Extension<Caller>,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<Json<crate::b2b::model::WebhookEndpointView>, AppError> {
    let endpoint = state.b2b.create_webhook(caller, &body).await?;
    Ok(Json(endpoint))
}

/// `GET /api/b2b/orders` — dashboard view of the merchant's orders.
pub async fn list_orders(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<crate::b2b::model::TransferView>>, AppError> {
    let orders = state
        .b2b
        .transfers(caller, query.limit.unwrap_or(50))
        .await?;
    Ok(Json(orders))
}

/// `GET /api/b2b/analytics`
pub async fn analytics(
    State(state): State<AppState>,
    Extension(caller): Extension<Caller>,
) -> Result<Json<crate::b2b::model::AnalyticsView>, AppError> {
    Ok(Json(state.b2b.analytics(caller).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::b2b::keys::KeyEnv;

    fn caller(env: Option<KeyEnv>) -> Caller {
        Caller {
            merchant_id: Uuid::nil(),
            project_id: env.map(|_| Uuid::nil()),
            api_key_id: env.map(|_| Uuid::nil()),
            env,
        }
    }

    #[test]
    fn key_environment_fills_a_missing_request_env() {
        let mut env = None;
        pin_env(&mut env, &caller(Some(KeyEnv::Test))).unwrap();
        assert_eq!(env.as_deref(), Some("test"));
    }

    #[test]
    fn key_environment_must_match_when_requested() {
        let mut env = Some("live".into());
        assert!(pin_env(&mut env, &caller(Some(KeyEnv::Test))).is_err());

        let mut env = Some("TEST".into());
        pin_env(&mut env, &caller(Some(KeyEnv::Test))).unwrap();
        assert_eq!(env.as_deref(), Some("test"));
    }

    #[test]
    fn legacy_caller_keeps_the_request_environment() {
        let mut env = Some("live".into());
        pin_env(&mut env, &caller(None)).unwrap();
        assert_eq!(env.as_deref(), Some("live"));
    }
}

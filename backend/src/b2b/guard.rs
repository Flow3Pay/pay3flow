//! Router-wide merchant API key guard (§8.1).
//!
//! The layer classifies every request through [`key_coverage::classify`] and
//! authenticates business routes with `Authorization: Bearer sk_…` before the
//! handler runs. Allowlisted routes pass through untouched; `/api/v1/*` and
//! `/api/b2b/*` still accept the legacy user JWT during the deprecation window.
//!
//! The authenticated key is put into request extensions, so a handler can read
//! the project and key without looking at the token again. Usage is recorded
//! after the response, whatever its status, per §5.3.2.

use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::{header::AUTHORIZATION, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use serde_json::json;

use crate::core::error::AppError;
use crate::core::state::AppState;

use super::keys::{self, Scope};
use super::{key_coverage, Error};

/// Authenticate a business route with a project key.
pub async fn require_key(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }
    let path = request.uri().path().to_string();
    let Some(class) = key_coverage::classify(request.method().as_str(), &path) else {
        return next.run(request).await;
    };
    if key_coverage::allows_keyless(class) {
        return next.run(request).await;
    }
    let Some(scope) = required_scope(class) else {
        return next.run(request).await;
    };

    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| keys::bearer_token(Some(value)))
        .map(str::to_string);
    let Some(token) = token else {
        return unauthorized("API key required");
    };
    if !token.starts_with("sk_") {
        // The legacy user token stays valid only where the plan allows it; the
        // handler still verifies it through `auth_user_id`.
        return match class {
            key_coverage::RouteClass::LegacyUserOrKey(_) => next.run(request).await,
            _ => unauthorized("API key required"),
        };
    }

    let key = match keys::authenticate(&state.pool, &token).await {
        Ok(key) => key,
        Err(Error::Unauthorized(message)) => return unauthorized(&message),
        Err(error) => {
            tracing::error!(error = %error, "could not authenticate merchant API key");
            return AppError::Internal(anyhow::anyhow!("api key authentication failed"))
                .into_response();
        }
    };

    if !key.allows(scope) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "insufficient_scope",
                "required": scope.as_str(),
            })),
        )
            .into_response();
    }

    let now = Utc::now();
    match keys::over_daily_limit(&state.pool, &key, now).await {
        Ok(true) => {
            let (_, reset) = keys::utc_day_bounds(now);
            return AppError::TooManyRequests {
                message: "daily request limit reached".into(),
                limit: u32::try_from(key.daily_request_limit).unwrap_or(0),
                remaining: 0,
                reset_unix: reset.timestamp(),
            }
            .into_response();
        }
        Ok(false) => {}
        Err(error) => {
            tracing::error!(error = %error, "could not read merchant API key usage");
            return AppError::Internal(anyhow::anyhow!("api key usage lookup failed"))
                .into_response();
        }
    }

    let key_id = key.id;
    request.extensions_mut().insert(key);
    let started = Instant::now();
    let response = next.run(request).await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Err(error) = keys::record_usage(
        &state.pool,
        key_id,
        response.status().as_u16(),
        latency_ms,
        Utc::now(),
    )
    .await
    {
        tracing::warn!(error = %error, "could not record merchant API key usage");
    }
    response
}

fn required_scope(class: key_coverage::RouteClass) -> Option<Scope> {
    match class {
        key_coverage::RouteClass::Protected(scope)
        | key_coverage::RouteClass::LegacyUserOrKey(scope) => Some(scope),
        key_coverage::RouteClass::Keyless
        | key_coverage::RouteClass::Staff
        | key_coverage::RouteClass::TemporarySite { .. } => None,
    }
}

fn unauthorized(message: &str) -> Response {
    AppError::Unauthorized(message.into()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::b2b::key_coverage::RouteClass;
    use crate::b2b::keys::AuthenticatedKey;

    #[test]
    fn scope_is_required_only_for_business_classes() {
        assert_eq!(
            required_scope(RouteClass::LegacyUserOrKey(Scope::OrdersRead)),
            Some(Scope::OrdersRead)
        );
        assert_eq!(
            required_scope(RouteClass::Protected(Scope::OtcWrite)),
            Some(Scope::OtcWrite)
        );
        assert_eq!(required_scope(RouteClass::Keyless), None);
        assert_eq!(required_scope(RouteClass::Staff), None);
        assert_eq!(
            required_scope(RouteClass::TemporarySite {
                until: "2026-12-11"
            }),
            None
        );
    }

    #[test]
    fn key_without_scope_is_rejected() {
        let key = AuthenticatedKey {
            id: uuid::Uuid::nil(),
            project_id: uuid::Uuid::nil(),
            merchant_id: uuid::Uuid::nil(),
            env: keys::KeyEnv::Test,
            scopes: vec![Scope::RatesRead],
            daily_request_limit: 0,
            prefix: "sk_test_00000000".into(),
        };
        assert!(key.allows(Scope::RatesRead));
        assert!(!key.allows(Scope::OrdersWrite));
    }
}

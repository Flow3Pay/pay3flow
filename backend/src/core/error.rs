use axum::http::{header::HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Unauthorized(String),
    Forbidden(String),
    TooManyRequests {
        message: String,
        limit: u32,
        remaining: u32,
        reset_unix: i64,
    },
    NotImplemented(String),
    Unavailable(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::TooManyRequests {
                message,
                limit,
                remaining,
                reset_unix,
            } => {
                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ErrorBody { error: message }),
                )
                    .into_response();
                let headers = response.headers_mut();
                if let Ok(value) = HeaderValue::from_str(&limit.to_string()) {
                    headers.insert("X-RateLimit-Limit", value);
                }
                if let Ok(value) = HeaderValue::from_str(&remaining.to_string()) {
                    headers.insert("X-RateLimit-Remaining", value);
                }
                if let Ok(value) = HeaderValue::from_str(&reset_unix.to_string()) {
                    headers.insert("X-RateLimit-Reset", value);
                }
                let retry_after = (reset_unix - chrono::Utc::now().timestamp()).max(0);
                if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
                    headers.insert("Retry-After", value);
                }
                response
            }
            other => {
                let (status, msg) = match other {
                    AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
                    AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
                    AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
                    AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
                    AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
                    AppError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
                    AppError::Unavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
                    AppError::Internal(e) => {
                        tracing::error!("internal error: {e:?}");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "internal server error".into(),
                        )
                    }
                    AppError::TooManyRequests { .. } => unreachable!(),
                };
                (status, Json(ErrorBody { error: msg })).into_response()
            }
        }
    }
}

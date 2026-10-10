use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("OTC route is paused or not configured")]
    Disabled,
    #[error("OTC authentication required")]
    Unauthorized,
    #[error("OTC record not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error(transparent)]
    Http(reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
pub(crate) type Result<T> = std::result::Result<T, Error>;
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::Disabled => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            other => {
                tracing::error!(error = %other, "otc.operation.failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "OTC operation failed".into(),
                )
            }
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}
pub(crate) fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

use axum::extract::Path;
use axum::Json;
use serde::Serialize;

use crate::core::error::AppError;
use crate::service::user;

#[derive(Serialize)]
pub struct OauthRes {
    pub token: String,
}

pub async fn oauth(Path(provider): Path<String>) -> Result<Json<OauthRes>, AppError> {
    let token = user::oauth::authorize(&provider)
        .map_err(|_| AppError::NotImplemented("oauth is not implemented yet".into()))?;
    Ok(Json(OauthRes { token }))
}

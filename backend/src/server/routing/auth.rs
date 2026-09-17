use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::service::user::{self, UserError};

#[derive(Deserialize)]
pub struct RegisterReq {
    pub email: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub code: String,
}

#[derive(Serialize)]
pub struct AuthRes {
    pub token: String,
}

#[derive(Serialize)]
pub struct UserRes {
    pub id: String,
    pub email: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<AuthRes>, AppError> {
    let id = user::register_user(&state.pool, &req.email, &req.code)
        .await
        .map_err(map_user_err)?;
    Ok(Json(AuthRes {
        token: state.jwt.sign(&id.to_string())?,
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<AuthRes>, AppError> {
    let token = user::login(&state.pool, &state.jwt, &req.email, &req.code)
        .await
        .map_err(map_user_err)?;
    Ok(Json(AuthRes { token }))
}

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserRes>, AppError> {
    let token = bearer_token(&headers)?;
    let user = user::current_user(&state.pool, &state.jwt, token)
        .await
        .map_err(map_user_err)?;
    Ok(Json(UserRes {
        id: user.id.to_string(),
        email: user.email,
    }))
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

fn map_user_err(err: UserError) -> AppError {
    match err {
        UserError::InvalidCode => AppError::BadRequest("invalid auth code".into()),
        UserError::InvalidEmail => AppError::BadRequest("invalid email".into()),
        UserError::NotFound => AppError::NotFound("user not found".into()),
        UserError::AlreadyExists => AppError::Conflict("email already registered".into()),
        UserError::Other(inner) => AppError::Internal(inner),
    }
}

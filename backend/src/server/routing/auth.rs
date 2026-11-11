use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::service::user::{self, UserError};

use super::anonymous::client_ip;

#[derive(Deserialize)]
pub struct RegisterReq {
    pub email: String,
    pub code: String,
    pub referral_code: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct CodeReq {
    pub email: String,
}

#[derive(Deserialize)]
pub struct AuthenticateReq {
    pub email: String,
    pub code: String,
    pub referral_code: Option<String>,
}

#[derive(Serialize)]
pub struct AuthRes {
    pub token: String,
}

#[derive(Serialize)]
pub struct UserRes {
    pub id: String,
    pub email: String,
    pub referral_code: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<Json<AuthRes>, AppError> {
    let id = user::register_user(
        &state.pool,
        &req.email,
        &req.code,
        req.referral_code.as_deref(),
    )
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

/// `POST /api/auth/code` — send a sign-in code. The response is the same for a
/// registered and an unregistered email (§4.3), except for rate limiting.
pub async fn request_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CodeReq>,
) -> Result<StatusCode, AppError> {
    let ip = client_ip(&headers).map(|ip| ip.to_string());
    user::auth::request_code(&state.pool, &state.mail, &req.email, ip.as_deref())
        .await
        .map_err(map_user_err)?;
    Ok(StatusCode::ACCEPTED)
}

/// `POST /api/auth/authenticate` — verify the code and return a session for an
/// existing or a newly created account.
pub async fn authenticate(
    State(state): State<AppState>,
    Json(req): Json<AuthenticateReq>,
) -> Result<Json<AuthRes>, AppError> {
    let token = user::authenticate(
        &state.pool,
        &state.jwt,
        &req.email,
        &req.code,
        req.referral_code.as_deref(),
    )
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
        referral_code: user.referral_code,
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
        UserError::RateLimited { retry_after_secs } => AppError::TooManyRequests {
            message: "too many code requests".into(),
            limit: 0,
            remaining: 0,
            reset_unix: chrono::Utc::now().timestamp() + retry_after_secs,
        },
        UserError::InvalidEmail => AppError::BadRequest("invalid email".into()),
        UserError::NotFound => AppError::NotFound("user not found".into()),
        UserError::AlreadyExists => AppError::Conflict("email already registered".into()),
        UserError::InvalidReferralCode => AppError::BadRequest("invalid referral code".into()),
        UserError::Other(inner) => AppError::Internal(inner),
    }
}

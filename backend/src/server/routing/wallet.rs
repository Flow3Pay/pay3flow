use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::server::routing::pairs::auth_admin;
use crate::wallet::{
    NativeTransferRequest, TokenTransferRequest, TransferResult, WalletBalance, WalletInfo,
};

pub async fn info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WalletInfo>, AppError> {
    auth_admin(&state, &headers)?;
    let wallet = state
        .wallet
        .as_ref()
        .ok_or_else(|| AppError::NotImplemented("self-hosted wallet is not configured".into()))?;
    Ok(Json(wallet.info().map_err(AppError::from)?))
}

pub async fn balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<WalletBalance>, AppError> {
    auth_admin(&state, &headers)?;
    let wallet = state
        .wallet
        .as_ref()
        .ok_or_else(|| AppError::NotImplemented("self-hosted wallet is not configured".into()))?;
    Ok(Json(wallet.balance().await.map_err(AppError::from)?))
}

pub async fn transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NativeTransferRequest>,
) -> Result<Json<TransferResult>, AppError> {
    auth_admin(&state, &headers)?;
    let wallet = state
        .wallet
        .as_ref()
        .ok_or_else(|| AppError::NotImplemented("self-hosted wallet is not configured".into()))?;
    Ok(Json(
        wallet
            .transfer_native(request)
            .await
            .map_err(AppError::from)?,
    ))
}

pub async fn token_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TokenTransferRequest>,
) -> Result<Json<TransferResult>, AppError> {
    auth_admin(&state, &headers)?;
    let wallet = state
        .wallet
        .as_ref()
        .ok_or_else(|| AppError::NotImplemented("self-hosted wallet is not configured".into()))?;
    Ok(Json(
        wallet
            .transfer_token(request)
            .await
            .map_err(AppError::from)?,
    ))
}

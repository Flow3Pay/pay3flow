use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::referrals::ReferralProfile;
use crate::server::routing::payments::auth_user_id;

pub async fn profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ReferralProfile>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let profile = crate::referrals::profile(&state.pool, &user_id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(profile))
}

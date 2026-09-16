use axum::extract::State;
use axum::Json;

use crate::core::state::AppState;
use crate::routing::{PaymentRequest, RouteResolved};

/// POST /routing/fallback — resolve a payment request with local rules only
/// (the fmatch-unreachable path). Debug surface for the degradation story.
pub async fn fallback(
    State(state): State<AppState>,
    Json(req): Json<PaymentRequest>,
) -> Json<RouteResolved> {
    Json(state.picker.resolve(&req, None))
}

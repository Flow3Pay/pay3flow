use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::core::state::AppState;

/// `GET /api/debug/acquirers` — the full catalog of the 10 fictional solvers
/// fmatch routes to, each with its per-pair commission and invented API layer.
pub async fn list(State(_state): State<AppState>) -> Json<Value> {
    let items: Vec<Value> = crate::fake_acquirers::FAKE_ACQUIRERS
        .iter()
        .map(serialize_summary)
        .collect();
    Json(json!({ "count": items.len(), "acquirers": items }))
}

/// `GET /api/debug/acquirers/:slug` — one fictional solver in full detail:
/// per-pair commissions in the "Flinger Pay: USD/HKD 3.4%, USD/AUD 2.5%"
/// template, plus every fake endpoint of its API layer.
pub async fn by_slug(
    State(_state): State<AppState>,
    Path(slug): Path<String>,
) -> Response {
    let Some(acq) = crate::fake_acquirers::fake_acquirer_by_slug(&slug) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("no fake acquirer with slug '{slug}'")})),
        )
            .into_response();
    };
    Json(serde_json::to_value(acq).unwrap_or_default()).into_response()
}

fn serialize_summary(acq: &crate::fake_acquirers::FakeAcquirer) -> Value {
    let pairs: Vec<Value> = acq
        .pairs
        .iter()
        .map(|p| json!({"from": p.from, "to": p.to, "commission": p.commission}))
        .collect();
    json!({
        "slug": acq.slug,
        "name": acq.name,
        "geo": acq.geo,
        "limits": acq.limits,
        "quality": acq.quality,
        "latency_ms": acq.latency_ms,
        "pairs": pairs,
        "api": {
            "base_url": acq.api.base_url,
            "auth": acq.api.auth,
            "webhook_secret": acq.api.webhook_secret,
            "endpoints": acq.api.endpoints,
        },
    })
}
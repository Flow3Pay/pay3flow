use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::activitypub::model::AcquirerCandidate;
use crate::core::state::AppState;

pub async fn webfinger(
    State(state): State<AppState>,
    Query(params): Query<WebfingerQuery>,
) -> Json<Value> {
    let resource = params.resource.unwrap_or_default();
    // accept both acct:handle@domain and bare IRIs / actor id
    let resource_for_profile = if resource.starts_with("acct:") {
        resource.trim_start_matches("acct:").to_string()
    } else {
        resource.clone()
    };
    Json(state.ap.identity.webfinger(&resource_for_profile))
}

#[derive(serde::Deserialize)]
pub struct WebfingerQuery {
    pub resource: Option<String>,
}

pub async fn actor_collection(State(state): State<AppState>) -> Json<Value> {
    let id = &state.ap.identity.actor_id;
    Json(json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": id,
        "type": "OrderedCollection",
        "totalItems": 1,
        "orderedItems": [id],
    }))
}

pub async fn actor_document(State(state): State<AppState>) -> Json<Value> {
    Json(state.ap.identity.to_document())
}

/// `/actor/:handle` — serve the actor document for any handle (we have one).
pub async fn actor_document_by_handle(
    State(state): State<AppState>,
    Path(handle): Path<String>,
) -> Response {
    tracing::debug!(%handle, "actor_document_by_handle");
    if handle != state.ap.identity.handle {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "actor not found"})),
        )
            .into_response();
    }
    Json(state.ap.identity.to_document()).into_response()
}

/// `/candidates` — debug endpoint: the most recently received candidate list.
pub async fn candidates(State(state): State<AppState>) -> Json<Vec<AcquirerCandidate>> {
    Json(state.ap.latest_candidates().await)
}

/// POST /inbox and POST /inbox/:handle live in the activitypub inbox module.
pub use crate::activitypub::inbox::handle as inbox;
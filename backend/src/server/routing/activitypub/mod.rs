use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::activitypub::fep8fba;
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

pub async fn exchange_resource(State(state): State<AppState>) -> Json<Value> {
    Json(fep8fba::exchange_resource(&state.ap.origin))
}

pub async fn exchange_proposal(State(state): State<AppState>) -> Json<Value> {
    Json(fep8fba::exchange_proposal(
        &state.ap.origin,
        &state.ap.identity.actor_id,
        &state.ap.fmatch_actor_id,
    ))
}

pub async fn exchange_interface(State(state): State<AppState>) -> Json<Value> {
    Json(fep8fba::interface_collection(&state.ap.origin))
}

pub async fn exchange_input_shape(State(state): State<AppState>) -> Json<Value> {
    Json(fep8fba::input_shape(&state.ap.origin))
}

pub async fn exchange_output_shape(State(state): State<AppState>) -> Json<Value> {
    Json(fep8fba::output_shape(&state.ap.origin))
}

pub async fn exchange_preview(
    state: State<AppState>,
    payload: Json<fep8fba::PreviewInput>,
) -> Response {
    fep8fba::preview(state, payload).await
}

/// `/candidates` — debug endpoint: the most recently received candidate list.
pub async fn candidates(State(state): State<AppState>) -> Json<Vec<AcquirerCandidate>> {
    Json(state.ap.latest_candidates().await)
}

/// `POST /api/debug/task` — submit a payflow request into fmatch and return
/// the inline candidate list it answers with (punkt 22, "best vs suitable").
#[derive(serde::Deserialize)]
pub struct DebugTaskPayload {
    #[serde(default = "default_command")]
    pub command: String,
    #[serde(default = "default_content")]
    pub content: String,
}

fn default_command() -> String {
    "candidates".into()
}

fn default_content() -> String {
    "transfer 100 USD from PayPal to VISA of BelarusBank; currency=USD; from=paypal; to=visa-belarusbank".into()
}

pub async fn debug_task(
    State(state): State<AppState>,
    payload: Option<Json<DebugTaskPayload>>,
) -> Response {
    let payload = payload.map(|Json(p)| p).unwrap_or(DebugTaskPayload {
        command: default_command(),
        content: default_content(),
    });

    state.ap.clear_candidates().await;
    match state
        .ap
        .submit_request(&payload.command, &payload.content)
        .await
    {
        Ok((outcome, body)) => {
            let candidates = body
                .as_ref()
                .map(AcquirerCandidate::from_reply)
                .unwrap_or_default();
            state.ap.received_candidates(&candidates).await;
            (
                StatusCode::OK,
                Json(json!({
                    "command": payload.command,
                    "outcome": format!("{outcome:?}"),
                    "body": body,
                    "candidates": candidates,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /inbox and POST /inbox/:handle live in the activitypub inbox module.
pub use crate::activitypub::inbox::handle as inbox_shared;
pub use crate::activitypub::inbox::handle_named as inbox_named;

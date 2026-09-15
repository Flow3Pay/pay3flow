use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use tokio_postgres::types::ToSql;

use crate::activitypub::model::AcquirerCandidate;
use crate::core::state::AppState;
use crate::db::DbPool;

const DEFAULT_HANDLE: &str = "pay3flow";

/// Shared inbox entry-point: `POST /inbox[/:handle]`
pub async fn handle(
    State(state): State<AppState>,
    Path(handle): Path<Option<String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let handle_str = handle.as_deref().unwrap_or(DEFAULT_HANDLE);
    match inner(&state, handle_str, &headers, body.into_bytes()).await {
        Ok(resp) => resp,
        Err(status) => (status, Json(json!({"error": "bad request"}))).into_response(),
    }
}

async fn inner(
    state: &AppState,
    handle: &str,
    headers: &HeaderMap,
    raw_body: Vec<u8>,
) -> Result<Response, StatusCode> {
    if state.ap.require_signatures {
        verify_inbound_signature(state, headers, raw_body.as_slice()).await?;
    }

    let activity: Value = serde_json::from_slice(&raw_body).map_err(|_| StatusCode::BAD_REQUEST)?;

    if let Some(id) = activity.get("id").and_then(Value::as_str) {
        if was_received(&state.pool, id).await? {
            return Ok(Json(json!({
                "status": "exists",
                "activity": activity,
            }))
            .into_response());
        }
    }

    let atype = activity
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    let response = match atype {
        "Follow" => handle_follow(state, &activity).await?,
        _ => handle_generic(state, handle, &activity).await?,
    };

    if let Some(id) = activity.get("id").and_then(Value::as_str) {
        record_inbound(&state.pool, id).await?;
    }

    Ok(response)
}

async fn handle_follow(state: &AppState, activity: &Value) -> Result<Response, StatusCode> {
    let accept = crate::activitypub::model::accept_follow(activity, &state.ap.identity.actor_id);
    Ok(Json(accept).into_response())
}

async fn handle_generic(
    state: &AppState,
    _handle: &str,
    activity: &Value,
) -> Result<Response, StatusCode> {
    let atype = activity.get("type").and_then(Value::as_str).unwrap_or("");

    let candidates = AcquirerCandidate::from_reply(activity);
    if !candidates.is_empty() {
        state.ap.received_candidates(&candidates).await;
        tracing::info!(count = candidates.len(), "fmatch delivered candidate list");
    }

    match atype {
        "Offer" | "Accept" => {
            let activity_id = activity.get("id").and_then(Value::as_str).unwrap_or("?");
            tracing::info!(atype, activity_id, "received activity with candidate data");
        }
        "Reject" => {
            let detail = activity
                .get("summary")
                .or_else(|| activity.get("detail"))
                .and_then(Value::as_str)
                .unwrap_or("no detail");
            tracing::warn!(atype, detail, "fmatch rejected activity");
        }
        _ => {
            tracing::debug!(atype, "received unhandled activity type");
        }
    }

    Ok(Json(json!({"status": "accepted"})).into_response())
}

async fn verify_inbound_signature(
    state: &AppState,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(), StatusCode> {
    let sig_header = headers
        .get("signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let date_header = headers
        .get("date")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let key_id = crate::activitypub::signature::verify(
        &[("date", date_header), ("signature", sig_header)],
        "POST",
        "/inbox",
        body,
        &state.ap.identity.public_key_pem(),
        chrono::Utc::now(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if !state.ap.is_local_activitypub_actor(&key_id) {
        let _pubkey = state
            .ap
            .delivery
            .fetch_public_key_for_verify(&state.ap.identity, &key_id)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
    }

    Ok(())
}

async fn was_received(pool: &DbPool, id: &str) -> Result<bool, StatusCode> {
    let client = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let row = client
        .query_opt(
            "SELECT 1 FROM activitypub_inbox WHERE activity_id = $1",
            &[&id as &(dyn ToSql + Sync)],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(row.is_some())
}

async fn record_inbound(pool: &DbPool, id: &str) -> Result<(), StatusCode> {
    let client = pool
        .get()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    client
        .execute(
            "INSERT INTO activitypub_inbox (activity_id) VALUES ($1)
             ON CONFLICT (activity_id) DO NOTHING",
            &[&id as &(dyn ToSql + Sync)],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}
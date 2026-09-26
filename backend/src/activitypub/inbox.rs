use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use tokio_postgres::types::ToSql;

use crate::activitypub::model::AcquirerCandidate;
use crate::core::state::AppState;
use crate::db::DbPool;
use crate::route_engine::{Amount, Asset};

const DEFAULT_HANDLE: &str = "pay3flow";

/// Shared inbox entry-point: `POST /inbox`
pub async fn handle(State(state): State<AppState>, headers: HeaderMap, body: String) -> Response {
    handle_inner(&state, DEFAULT_HANDLE, &headers, body).await
}

/// Per-handle inbox entry-point: `POST /inbox/:handle`
pub async fn handle_named(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    handle_inner(&state, &handle, &headers, body).await
}

async fn handle_inner(
    state: &AppState,
    handle_str: &str,
    headers: &HeaderMap,
    body: String,
) -> Response {
    match inner(state, handle_str, headers, body.into_bytes()).await {
        Ok(resp) => resp,
        Err(status) => {
            tracing::warn!(code = %status, "inbox inner failed");
            (status, Json(json!({"error": "bad request"}))).into_response()
        }
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

    let activity: Value = serde_json::from_slice(&raw_body).map_err(|e| {
        tracing::warn!(error = %e, "inbox: body is not valid JSON");
        StatusCode::BAD_REQUEST
    })?;

    if let Some(id) = activity.get("id").and_then(Value::as_str) {
        match was_received(&state.pool, id).await {
            Ok(true) => {
                return Ok(Json(json!({"status": "exists", "activity": activity})).into_response())
            }
            Ok(false) => {}
            Err(code) => {
                tracing::warn!(activity_id = id, code = %code, "was_received failed");
                return Err(code);
            }
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
        if let Err(code) = record_inbound(&state.pool, id).await {
            tracing::warn!(activity_id = id, code = %code, "record_inbound failed");
            return Err(code);
        }
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

    if atype == "Proposal"
        && activity.get("purpose").and_then(Value::as_str) == Some("request")
        && activity.get("command").and_then(Value::as_str) == Some("quote")
    {
        return handle_quote_request(state, activity).await;
    }

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

async fn handle_quote_request(state: &AppState, activity: &Value) -> Result<Response, StatusCode> {
    let requester = activity
        .get("attributedTo")
        .or_else(|| activity.get("actor"))
        .and_then(Value::as_str);
    if requester != Some(state.ap.fmatch_actor_id.as_str()) {
        tracing::warn!(
            ?requester,
            "rejected route quote request from a non-fmatch actor"
        );
        return Err(StatusCode::FORBIDDEN);
    }
    let fields = activity
        .get("content")
        .and_then(Value::as_str)
        .map(parse_fields)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let route_id = fields.get("route_id").ok_or(StatusCode::BAD_REQUEST)?;
    let (amount_value, asset_value) =
        quote_amount_fields(&fields).ok_or(StatusCode::BAD_REQUEST)?;
    let asset = Asset::parse(asset_value).map_err(|_| StatusCode::BAD_REQUEST)?;
    let amount = Amount::new(amount_value, asset).map_err(|_| StatusCode::BAD_REQUEST)?;
    let quote = state
        .route_quotes
        .quote(route_id, amount)
        .await
        .map_err(|error| {
            tracing::warn!(%error, route_id, "fmatch route quote failed");
            StatusCode::NOT_FOUND
        })?;
    Ok(Json(json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://w3id.org/fep/0837"
        ],
        "type": "Offer",
        "id": format!("{}/route-quotes/{}", state.ap.origin, quote.quote_id),
        "actor": &state.ap.identity.actor_id,
        "object": {
            "type": "RouteQuote",
            "route_id": quote.route_id,
            "quote_id": quote.quote_id.clone(),
            "input": quote.input,
            "output": quote.output,
            "fees": quote.fees,
            "available": quote.available,
            "expires_at": quote.expires_at,
        }
    }))
    .into_response())
}

fn parse_fields(content: &str) -> std::collections::HashMap<String, String> {
    content
        .split(';')
        .filter_map(|part| part.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

fn quote_amount_fields(
    fields: &std::collections::HashMap<String, String>,
) -> Option<(String, &str)> {
    let amount = fields.get("amount")?.trim();
    if let Some(asset) = fields.get("asset") {
        return Some((amount.to_string(), asset.trim()));
    }
    let mut parts = amount.split_whitespace();
    let value = parts.next()?;
    let asset = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((value.to_string(), asset))
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
        state.ap.identity.public_key_pem(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_amount_accepts_explicit_or_compound_asset_fields() {
        let explicit = parse_fields("route_id=r; amount=500000; asset=AMD");
        assert_eq!(
            quote_amount_fields(&explicit),
            Some(("500000".to_string(), "AMD"))
        );

        let compound = parse_fields("route_id=r; amount=500000 AMD");
        assert_eq!(
            quote_amount_fields(&compound),
            Some(("500000".to_string(), "AMD"))
        );
    }

    #[test]
    fn quote_amount_rejects_ambiguous_compound_values() {
        let fields = parse_fields("route_id=r; amount=500000 AMD extra");
        assert!(quote_amount_fields(&fields).is_none());
    }
}

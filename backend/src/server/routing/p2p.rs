use std::collections::{HashMap, HashSet};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::p2p::{
    P2pRoute, P2pRouteSearchQuery, P2pRouteSearchResponse, P2pSearchQuery, P2pSearchResponse,
};
use crate::service_reputation::{
    average_reputation, ReputationError, RouteServiceStats, ServiceLink, ServiceLinkKind,
    ServiceStats, VoteChoice,
};

#[derive(Debug, Deserialize)]
pub struct RouteHttpQuery {
    #[serde(flatten)]
    query: P2pRouteSearchQuery,
    anonymous_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSocketRequest {
    anonymous_id: Uuid,
    query: P2pRouteSearchQuery,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenExecutionRequest {
    anonymous_id: Uuid,
    tracking_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoteRequest {
    anonymous_id: Uuid,
    vote: Option<VoteChoice>,
}

/// Read-only live search over configured public P2P advertisement sources.
/// This endpoint never contacts advertisers, creates exchange orders, or places P2P orders.
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<P2pSearchQuery>,
) -> Result<Json<P2pSearchResponse>, AppError> {
    state
        .p2p
        .search(query)
        .await
        .map(Json)
        .map_err(|error| AppError::BadRequest(error.to_string()))
}

/// Build and rank complete P2P routes. `routes_found` counts every valid
/// unique route before the response's display limit is applied.
pub async fn routes(
    State(state): State<AppState>,
    Query(request): Query<RouteHttpQuery>,
) -> Result<Json<P2pRouteSearchResponse>, AppError> {
    let search_id = Uuid::new_v4();
    state
        .reputation
        .start_search(search_id)
        .await
        .map_err(map_reputation_error)?;
    let response = state.p2p.search_routes(request.query).await;
    let mut response = match response {
        Ok(mut response) => {
            response.search_id = search_id;
            response
        }
        Err(error) => {
            state
                .reputation
                .update_search(search_id, 0, "failed")
                .await
                .map_err(map_reputation_error)?;
            return Err(AppError::BadRequest(error.to_string()));
        }
    };
    if let Err(error) = enrich_routes(&state, &mut response, request.anonymous_id).await {
        let _ = state
            .reputation
            .update_search(search_id, response.routes_found, "failed")
            .await;
        return Err(error);
    }
    state
        .reputation
        .update_search(search_id, response.routes_found, "finished")
        .await
        .map_err(map_reputation_error)?;
    Ok(Json(response))
}

pub async fn routes_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| route_socket(state, socket))
}

async fn route_socket(state: AppState, mut socket: WebSocket) {
    let Some(Ok(Message::Text(payload))) = socket.recv().await else {
        let _ = socket.close().await;
        return;
    };
    let request = match serde_json::from_str::<RouteSocketRequest>(&payload) {
        Ok(request) => request,
        Err(error) => {
            let _ = send_json(
                &mut socket,
                json!({ "type": "search_failed", "error": error.to_string() }),
            )
            .await;
            let _ = socket.close().await;
            return;
        }
    };
    let search_id = Uuid::new_v4();
    if let Err(error) = state.reputation.start_search(search_id).await {
        let _ = send_json(
            &mut socket,
            json!({ "type": "search_failed", "search_id": search_id, "error": error.to_string() }),
        )
        .await;
        return;
    }
    if send_json(
        &mut socket,
        json!({ "type": "search_started", "search_id": search_id, "routes_found": 0 }),
    )
    .await
    .is_err()
    {
        let _ = state.reputation.update_search(search_id, 0, "failed").await;
        return;
    }

    let (updates_tx, mut updates_rx) = mpsc::channel(4);
    let p2p = state.p2p.clone();
    let query = request.query;
    let search_task =
        tokio::spawn(async move { p2p.stream_routes(query, search_id, updates_tx).await });
    let mut last_routes_found = 0;

    loop {
        tokio::select! {
            message = socket.recv() => {
                if matches!(message, None | Some(Ok(Message::Close(_))) | Some(Err(_))) {
                    search_task.abort();
                    let _ = search_task.await;
                    let _ = state.reputation.update_search(search_id, last_routes_found, "failed").await;
                    return;
                }
            }
            update = updates_rx.recv() => {
                let Some(mut response) = update else { break };
                if let Err(error) = enrich_routes(&state, &mut response, Some(request.anonymous_id)).await {
                    search_task.abort();
                    let _ = state.reputation.update_search(search_id, last_routes_found, "failed").await;
                    let _ = send_json(&mut socket, json!({
                        "type": "search_failed",
                        "search_id": search_id,
                        "error": error_message(error),
                    })).await;
                    return;
                }
                last_routes_found = response.routes_found;
                if state.reputation.update_search(search_id, last_routes_found, "searching").await.is_err() {
                    search_task.abort();
                    return;
                }
                if send_search_response(&mut socket, "routes_updated", response).await.is_err() {
                    search_task.abort();
                    let _ = state.reputation.update_search(search_id, last_routes_found, "failed").await;
                    return;
                }
            }
        }
    }

    match search_task.await {
        Ok(Ok(mut response)) => {
            if let Err(error) =
                enrich_routes(&state, &mut response, Some(request.anonymous_id)).await
            {
                let _ = state
                    .reputation
                    .update_search(search_id, last_routes_found, "failed")
                    .await;
                let _ = send_json(
                    &mut socket,
                    json!({
                        "type": "search_failed",
                        "search_id": search_id,
                        "error": error_message(error),
                    }),
                )
                .await;
                return;
            }
            if state
                .reputation
                .update_search(search_id, response.routes_found, "finished")
                .await
                .is_ok()
            {
                let _ = send_search_response(&mut socket, "search_finished", response).await;
            }
        }
        Ok(Err(error)) => {
            let _ = state
                .reputation
                .update_search(search_id, last_routes_found, "failed")
                .await;
            let _ = send_json(
                &mut socket,
                json!({ "type": "search_failed", "search_id": search_id, "error": error.to_string() }),
            )
            .await;
        }
        Err(error) => {
            let _ = state
                .reputation
                .update_search(search_id, last_routes_found, "failed")
                .await;
            let _ = send_json(
                &mut socket,
                json!({ "type": "search_failed", "search_id": search_id, "error": error.to_string() }),
            )
            .await;
        }
    }
    let _ = socket.close().await;
}

pub async fn open_execution(
    State(state): State<AppState>,
    Json(request): Json<OpenExecutionRequest>,
) -> Result<Json<crate::service_reputation::ExecutionOpen>, AppError> {
    let execution = state
        .reputation
        .record_execution(&request.tracking_token, request.anonymous_id)
        .await
        .map_err(map_reputation_error)?;
    tracing::info!(
        execution_id = %execution.execution_id,
        service = %execution.service.slug,
        newly_recorded = execution.newly_recorded,
        "service.execution.opened"
    );
    Ok(Json(execution))
}

pub async fn set_vote(
    State(state): State<AppState>,
    Path(service_id): Path<Uuid>,
    Json(request): Json<VoteRequest>,
) -> Result<Json<ServiceStats>, AppError> {
    let service = state
        .reputation
        .set_vote(service_id, request.anonymous_id, request.vote)
        .await
        .map_err(map_reputation_error)?;
    tracing::info!(service = %service.slug, vote = ?service.viewer_vote, "service.vote.updated");
    Ok(Json(service))
}

async fn enrich_routes(
    state: &AppState,
    response: &mut P2pRouteSearchResponse,
    anonymous_id: Option<Uuid>,
) -> Result<(), AppError> {
    let services = response
        .routes
        .iter()
        .flat_map(route_service_slugs)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|slug| {
            let name = service_name(&slug).to_string();
            (slug, name)
        })
        .collect::<Vec<_>>();
    state
        .reputation
        .ensure_services(&services)
        .await
        .map_err(map_reputation_error)?;
    let slugs = services
        .iter()
        .map(|(slug, _)| slug.clone())
        .collect::<Vec<_>>();
    let stats = state
        .reputation
        .stats_for_slugs(&slugs, anonymous_id)
        .await
        .map_err(map_reputation_error)?;

    for route in &mut response.routes {
        let route_slugs = route_service_slugs(route);
        route.services = route_slugs
            .iter()
            .filter_map(|slug| stats.get(slug).cloned())
            .map(|stats| RouteServiceStats { stats })
            .collect();
        route.reputation = Some(average_reputation(&route.services));
        route.service_links = route_links(state, response.search_id, route, &stats)?;
    }
    Ok(())
}

fn route_service_slugs(route: &P2pRoute) -> Vec<String> {
    let mut slugs = Vec::new();
    if let Some(entry) = &route.entry_offer {
        slugs.push(entry.source.to_ascii_lowercase());
    }
    if let Some(exit) = &route.exit_offer {
        let slug = exit.source.to_ascii_lowercase();
        if !slugs.contains(&slug) {
            slugs.push(slug);
        }
    }
    if let Some(market) = &route.market_path {
        let slug = market.venue.to_ascii_lowercase();
        if !slugs.contains(&slug) {
            slugs.push(slug);
        }
    }
    slugs
}

fn route_links(
    state: &AppState,
    search_id: Uuid,
    route: &P2pRoute,
    stats: &HashMap<String, ServiceStats>,
) -> Result<Vec<ServiceLink>, AppError> {
    let mut links = Vec::new();
    if let Some(offer) = &route.entry_offer {
        push_link(
            state,
            &mut links,
            stats,
            search_id,
            route,
            &offer.source,
            ServiceLinkKind::Entry,
            offer
                .advertiser_profile_url
                .as_deref()
                .unwrap_or(&offer.source_url),
        )?;
    }
    if let Some(offer) = &route.exit_offer {
        push_link(
            state,
            &mut links,
            stats,
            search_id,
            route,
            &offer.source,
            ServiceLinkKind::Exit,
            offer
                .advertiser_profile_url
                .as_deref()
                .unwrap_or(&offer.source_url),
        )?;
    }
    if let Some(market) = &route.market_path {
        let first_target = route
            .bridge_currency
            .as_deref()
            .unwrap_or(&route.target_fiat);
        if let Some(url) = spot_url(
            &market.venue,
            &market.source_pair,
            &route.source_fiat,
            first_target,
        ) {
            push_link(
                state,
                &mut links,
                stats,
                search_id,
                route,
                &market.venue,
                ServiceLinkKind::MarketSource,
                &url,
            )?;
        }
        if route.bridge_currency.is_some() && market.target_pair != market.source_pair {
            if let Some(url) = spot_url(
                &market.venue,
                &market.target_pair,
                route.bridge_currency.as_deref().unwrap_or_default(),
                &route.target_fiat,
            ) {
                push_link(
                    state,
                    &mut links,
                    stats,
                    search_id,
                    route,
                    &market.venue,
                    ServiceLinkKind::MarketTarget,
                    &url,
                )?;
            }
        }
    }
    Ok(links)
}

#[allow(clippy::too_many_arguments)]
fn push_link(
    state: &AppState,
    links: &mut Vec<ServiceLink>,
    stats: &HashMap<String, ServiceStats>,
    search_id: Uuid,
    route: &P2pRoute,
    slug: &str,
    kind: ServiceLinkKind,
    destination_url: &str,
) -> Result<(), AppError> {
    let slug = slug.to_ascii_lowercase();
    let Some(service) = stats.get(&slug) else {
        return Ok(());
    };
    let tracking_token = state
        .reputation
        .tracking_token(service.id, search_id, &route.route_id, destination_url)
        .map_err(map_reputation_error)?;
    links.push(ServiceLink {
        service_id: service.id,
        service_slug: slug,
        kind,
        tracking_token,
    });
    Ok(())
}

fn spot_url(venue: &str, symbol: &str, first_asset: &str, second_asset: &str) -> Option<String> {
    let normalized = symbol.replace(['/', '-', '_'], "").to_ascii_uppercase();
    let first = first_asset.to_ascii_uppercase();
    let second = second_asset.to_ascii_uppercase();
    let (base, quote) = if normalized == format!("{first}{second}") {
        (first, second)
    } else if normalized == format!("{second}{first}") {
        (second, first)
    } else {
        return None;
    };
    match venue.to_ascii_lowercase().as_str() {
        "binance" => Some(format!(
            "https://www.binance.com/en/trade/{base}_{quote}?type=spot"
        )),
        "bybit" => Some(format!("https://www.bybit.com/trade/spot/{base}/{quote}")),
        "okx" => Some(format!(
            "https://www.okx.com/trade-spot/{}-{}",
            base.to_ascii_lowercase(),
            quote.to_ascii_lowercase()
        )),
        "bitget" => Some(format!("https://www.bitget.com/spot/{base}{quote}")),
        _ => None,
    }
}

fn service_name(slug: &str) -> &str {
    match slug {
        "binance" => "Binance",
        "bybit" => "Bybit",
        "okx" => "OKX",
        "bitget" => "Bitget",
        "rapira" => "Rapira",
        other => other,
    }
}

async fn send_search_response(
    socket: &mut WebSocket,
    event_type: &str,
    response: P2pRouteSearchResponse,
) -> Result<(), axum::Error> {
    let mut value = serde_json::to_value(response).unwrap_or_else(|_| json!({}));
    if let Value::Object(object) = &mut value {
        object.insert("type".into(), Value::String(event_type.into()));
    }
    send_json(socket, value).await
}

async fn send_json(socket: &mut WebSocket, value: Value) -> Result<(), axum::Error> {
    socket.send(Message::Text(value.to_string())).await
}

fn error_message(error: AppError) -> String {
    match error {
        AppError::BadRequest(message)
        | AppError::NotFound(message)
        | AppError::Conflict(message)
        | AppError::Unauthorized(message)
        | AppError::NotImplemented(message) => message,
        AppError::Internal(error) => error.to_string(),
    }
}

fn map_reputation_error(error: ReputationError) -> AppError {
    match error {
        ReputationError::ServiceNotFound => AppError::NotFound(error.to_string()),
        ReputationError::ExecutionRequired | ReputationError::InvalidTrackingToken => {
            AppError::BadRequest(error.to_string())
        }
        ReputationError::Internal(error) => AppError::Internal(error),
    }
}

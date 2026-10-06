use std::collections::{HashMap, HashSet};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::p2p::{
    ExchangeMode, P2pRoute, P2pRouteSearchQuery, P2pRouteSearchResponse, P2pSearchQuery,
    P2pSearchResponse,
};
use crate::route_engine::canonical_network_id;
use crate::service_reputation::{
    average_reputation, vote_quality_score, ReputationError, RouteServiceStats,
    SearchActivityPeriod, ServiceLink, ServiceLinkKind, ServiceStats, VoteChoice,
};

#[derive(Debug, Deserialize)]
pub struct RouteHttpMetadata {
    anonymous_id: Option<Uuid>,
    count_activity: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RouteActivityQuery {
    source_currency: String,
    target_currency: String,
    #[serde(default)]
    period: SearchActivityPeriod,
}

#[derive(Serialize)]
pub struct RouteActivityResponse {
    source_currency: String,
    target_currency: String,
    hours: Vec<crate::service_reputation::RouteSearchActivityHour>,
}

/// Hourly search counts for a currency direction during the last seven days.
/// When a browser identity is provided, its own searches are excluded.
pub async fn route_activity(
    State(state): State<AppState>,
    Query(query): Query<RouteActivityQuery>,
) -> Result<Json<RouteActivityResponse>, AppError> {
    let source_currency = query.source_currency.trim().to_ascii_uppercase();
    let target_currency = query.target_currency.trim().to_ascii_uppercase();
    let valid = |currency: &str| {
        (2..=12).contains(&currency.len())
            && currency.bytes().all(|byte| byte.is_ascii_alphanumeric())
    };
    if !valid(&source_currency) || !valid(&target_currency) {
        return Err(AppError::BadRequest("Invalid currency pair".into()));
    }
    let hours = state
        .reputation
        .search_activity(&source_currency, &target_currency, query.period)
        .await
        .map_err(map_reputation_error)?;
    Ok(Json(RouteActivityResponse {
        source_currency,
        target_currency,
        hours,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSocketRequest {
    anonymous_id: Uuid,
    count_activity: Option<bool>,
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
pub struct OpenInstructionRequest {
    anonymous_id: Uuid,
    tracking_tokens: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoteRequest {
    anonymous_id: Uuid,
    vote: VoteChoice,
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

/// Build and rank P2P routes. `routes_found` counts the bounded candidate set
/// before the display limit; `routes_exhaustive` states whether that count is
/// an exact enumeration of every valid combination.
pub async fn routes(
    State(state): State<AppState>,
    Query(query): Query<P2pRouteSearchQuery>,
    Query(metadata): Query<RouteHttpMetadata>,
) -> Result<Json<P2pRouteSearchResponse>, AppError> {
    let search_id = Uuid::new_v4();
    state
        .reputation
        .start_search(
            search_id,
            &query.source_fiat,
            &query.target_fiat,
            metadata.anonymous_id,
            metadata.count_activity.unwrap_or(false),
        )
        .await
        .map_err(map_reputation_error)?;
    let mut response = match cached_route_response(&state, &query).await {
        Some(mut response) => {
            response.search_id = search_id;
            response
        }
        None => match state.p2p.search_routes(query.clone()).await {
            Ok(mut response) => {
                response.search_id = search_id;
                cache_route_response(&state, &query, &response);
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
        },
    };
    if let Err(error) = enrich_routes(&state, &mut response, metadata.anonymous_id).await {
        let _ = state
            .reputation
            .update_search(search_id, response.routes_found, "failed")
            .await;
        return Err(error);
    }
    if let Err(error) = save_shared_response(&state, &response).await {
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

async fn save_shared_response(
    state: &AppState,
    response: &P2pRouteSearchResponse,
) -> Result<(), AppError> {
    let body = serde_json::to_string(response).map_err(|error| AppError::Internal(error.into()))?;
    let client = state
        .pool
        .get()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    client
        .execute(
            "UPDATE route_searches SET shared_response = $2::jsonb WHERE id = $1",
            &[&response.search_id, &body],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(())
}

/// Return the exact completed search so a shared link opens without searching again.
pub async fn shared_routes(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(metadata): Query<RouteHttpMetadata>,
) -> Result<Json<P2pRouteSearchResponse>, AppError> {
    let client = state
        .pool
        .get()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let row = client.query_opt(
        "SELECT shared_response::text FROM route_searches WHERE id = $1 AND status = 'finished'",
        &[&id],
    ).await.map_err(|error| AppError::Internal(error.into()))?;
    let body: Option<String> = row.and_then(|row| row.get(0));
    let body = body.ok_or_else(|| AppError::NotFound("Shared search not found".into()))?;
    let mut response: P2pRouteSearchResponse =
        serde_json::from_str(&body).map_err(|error| AppError::Internal(error.into()))?;
    enrich_routes(&state, &mut response, metadata.anonymous_id).await?;
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
    if let Err(error) = state
        .reputation
        .start_search(
            search_id,
            &request.query.source_fiat,
            &request.query.target_fiat,
            Some(request.anonymous_id),
            request.count_activity.unwrap_or(false),
        )
        .await
    {
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

    if let Some(mut response) = cached_route_response(&state, &request.query).await {
        response.search_id = search_id;
        if let Err(error) = enrich_routes(&state, &mut response, Some(request.anonymous_id)).await {
            let _ = state
                .reputation
                .update_search(search_id, response.routes_found, "failed")
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
        if let Err(error) = save_shared_response(&state, &response).await {
            tracing::warn!(%search_id, ?error, "could not save shared route search");
            let _ = state
                .reputation
                .update_search(search_id, response.routes_found, "failed")
                .await;
            return;
        }
        if state
            .reputation
            .update_search(search_id, response.routes_found, "finished")
            .await
            .is_err()
        {
            return;
        }
        if send_search_response(&mut socket, "routes_updated", response.clone())
            .await
            .is_ok()
        {
            let _ = send_search_response(&mut socket, "search_finished", response).await;
        }
        return;
    }

    let (updates_tx, mut updates_rx) = mpsc::channel(4);
    let p2p = state.p2p.clone();
    let cache_query = request.query.clone();
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
            cache_route_response(&state, &cache_query, &response);
            response.search_id = search_id;
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
            if let Err(error) = save_shared_response(&state, &response).await {
                tracing::warn!(%search_id, ?error, "could not save shared route search");
                let _ = state
                    .reputation
                    .update_search(search_id, response.routes_found, "failed")
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

pub async fn open_instruction(
    State(state): State<AppState>,
    Json(request): Json<OpenInstructionRequest>,
) -> Result<Json<crate::service_reputation::InstructionOpen>, AppError> {
    if request.tracking_tokens.len() > 8 {
        return Err(AppError::BadRequest(
            "too many route tracking tokens".into(),
        ));
    }
    let event = state
        .reputation
        .record_instruction_open(&request.tracking_tokens, request.anonymous_id)
        .await
        .map_err(map_reputation_error)?;
    Ok(Json(event))
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

pub async fn set_route_vote(
    State(state): State<AppState>,
    Path(route_id): Path<String>,
    Json(request): Json<VoteRequest>,
) -> Result<Json<crate::service_reputation::RouteFeedback>, AppError> {
    let feedback = state
        .reputation
        .set_route_vote(&route_id, request.anonymous_id, request.vote)
        .await
        .map_err(map_reputation_error)?;
    tracing::info!(route_id = %route_id, vote = ?feedback.viewer_vote, "route.vote.updated");
    Ok(Json(feedback))
}

async fn enrich_routes(
    state: &AppState,
    response: &mut P2pRouteSearchResponse,
    anonymous_id: Option<Uuid>,
) -> Result<(), AppError> {
    let slugs = response
        .routes
        .iter()
        .flat_map(route_service_slugs)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let route_ids = response
        .routes
        .iter()
        .map(|route| route.route_id.clone())
        .collect::<Vec<_>>();
    let (stats, feedback) = tokio::join!(
        state.reputation.stats_for_slugs(&slugs, anonymous_id),
        state
            .reputation
            .feedback_for_routes(&route_ids, anonymous_id),
    );
    let stats = stats.map_err(map_reputation_error)?;
    let feedback = feedback.map_err(map_reputation_error)?;

    for route in &mut response.routes {
        let route_slugs = route_service_slugs(route);
        route.services = route_slugs
            .iter()
            .filter_map(|slug| stats.get(slug).cloned())
            .map(|stats| RouteServiceStats { stats })
            .collect();
        route.reputation = Some(average_reputation(&route.services));
        route.feedback = feedback.get(&route.route_id).cloned();
        route.service_links = route_links(state, response.search_id, route, &stats)?;
        state.route_executions.attach_descriptor(route);
    }
    rank_close_routes_by_votes(&mut response.routes);
    Ok(())
}

fn rank_close_routes_by_votes(routes: &mut [P2pRoute]) {
    routes.sort_by(|left, right| {
        let left_amount = left.target_amount.parse::<f64>().unwrap_or_default();
        let right_amount = right.target_amount.parse::<f64>().unwrap_or_default();
        right_amount.total_cmp(&left_amount)
    });

    let mut start = 0;
    while start < routes.len() {
        let best_amount = routes[start]
            .target_amount
            .parse::<f64>()
            .unwrap_or_default();
        let mut end = start + 1;
        while end < routes.len() {
            let amount = routes[end].target_amount.parse::<f64>().unwrap_or_default();
            if best_amount <= 0.0 || (best_amount - amount) / best_amount > 0.01 {
                break;
            }
            end += 1;
        }

        let scores = routes[start..end]
            .iter()
            .map(|route| {
                let service_likes = route
                    .services
                    .iter()
                    .map(|service| service.stats.likes_total)
                    .fold(0_i64, i64::saturating_add);
                let service_dislikes = route
                    .services
                    .iter()
                    .map(|service| service.stats.dislikes_total)
                    .fold(0_i64, i64::saturating_add);
                let route_likes = route
                    .feedback
                    .as_ref()
                    .map_or(0, |feedback| feedback.likes_total);
                let route_dislikes = route
                    .feedback
                    .as_ref()
                    .map_or(0, |feedback| feedback.dislikes_total);
                vote_quality_score(
                    service_likes.saturating_add(route_likes),
                    service_dislikes.saturating_add(route_dislikes),
                )
            })
            .collect::<Vec<_>>();
        if scores.iter().all(Option::is_some) {
            let mut indices = (0..end - start).collect::<Vec<_>>();
            indices
                .sort_by(|left, right| scores[*right].unwrap().total_cmp(&scores[*left].unwrap()));
            let ordered = indices
                .into_iter()
                .map(|index| routes[start + index].clone())
                .collect::<Vec<_>>();
            routes[start..end].clone_from_slice(&ordered);
        }
        start = end;
    }

    for (rank, route) in routes.iter_mut().enumerate() {
        route.rank = rank + 1;
    }
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
        "mexc" => Some(format!("https://www.mexc.com/exchange/{base}_{quote}")),
        "cifra-broker" => Some("https://tradernet.by/authentication/signup".into()),
        _ => None,
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
        ReputationError::InvalidTrackingToken => AppError::BadRequest(error.to_string()),
        ReputationError::Internal(error) => AppError::Internal(error),
    }
}

const ROUTE_RESULT_CACHE_TTL_SECS: u64 = 15;

async fn cached_route_response(
    state: &AppState,
    query: &P2pRouteSearchQuery,
) -> Option<P2pRouteSearchResponse> {
    if query.exchange_mode != ExchangeMode::Exchanger {
        return None;
    }
    let redis = state.redis.as_ref()?;
    let key = route_result_cache_key(query)?;
    match tokio::time::timeout(
        Duration::from_millis(100),
        crate::core::redis::get_json(redis, &key),
    )
    .await
    {
        Ok(Ok(Some(response))) => Some(response),
        Ok(Ok(None)) => None,
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to read route result cache");
            None
        }
        Err(_) => {
            tracing::warn!("route result cache read timed out");
            None
        }
    }
}

fn cache_route_response(
    state: &AppState,
    query: &P2pRouteSearchQuery,
    response: &P2pRouteSearchResponse,
) {
    if query.exchange_mode != ExchangeMode::Exchanger
        || response.routes_found == 0
        || response.routes.is_empty()
        || response.stale
        || response.asset_statuses.iter().any(|asset| {
            asset
                .entry_sources
                .iter()
                .chain(&asset.exit_sources)
                .any(|source| !source.ok)
        })
    {
        return;
    }
    let (Some(redis), Some(key)) = (state.redis.clone(), route_result_cache_key(query)) else {
        return;
    };
    let response = response.clone();
    tokio::spawn(async move {
        if let Err(error) =
            crate::core::redis::set_json(&redis, &key, &response, ROUTE_RESULT_CACHE_TTL_SECS).await
        {
            tracing::warn!(%error, "failed to write route result cache");
        }
    });
}

fn route_result_cache_key(query: &P2pRouteSearchQuery) -> Option<String> {
    let mut query = query.clone();
    query.source_fiat = query.source_fiat.trim().to_ascii_uppercase();
    query.target_fiat = query.target_fiat.trim().to_ascii_uppercase();
    query.source_network = query
        .source_network
        .as_deref()
        .map(str::trim)
        .filter(|network| !network.is_empty())
        .map(canonical_network_id);
    query.target_network = query
        .target_network
        .as_deref()
        .map(str::trim)
        .filter(|network| !network.is_empty())
        .map(canonical_network_id);
    if query.intermediary_assets.is_none() {
        query.intermediary_assets = query.assets.take();
    } else {
        query.assets = None;
    }
    for assets in [&mut query.intermediary_assets, &mut query.bridge_fiat] {
        if let Some(value) = assets {
            *value = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_uppercase)
                .collect::<Vec<_>>()
                .join(",");
        }
    }
    query.source_payment_method = query
        .source_payment_method
        .take()
        .map(|method| method.trim().to_string())
        .filter(|method| !method.is_empty());
    query.target_payment_method = query
        .target_payment_method
        .take()
        .map(|method| method.trim().to_string())
        .filter(|method| !method.is_empty());
    query.sources = crate::p2p::normalize_sources(query.sources.take()).ok()?;
    query.limit = Some(query.limit.unwrap_or(20).clamp(1, 100));
    query.max_price_deviation_bps = Some(query.max_price_deviation_bps.unwrap_or(1_000));
    query.merchant_only = Some(query.merchant_only.unwrap_or(false));
    query.allow_cross_venue = Some(query.allow_cross_venue.unwrap_or(false));
    query.min_orders = query.min_orders.filter(|orders| *orders > 0);
    query.source_payment_fee_percent = query
        .source_payment_fee_percent
        .map(|fee| (fee * 100.0).round() / 100.0);
    query.target_payment_fee_percent = query
        .target_payment_fee_percent
        .map(|fee| (fee * 100.0).round() / 100.0);
    let encoded = serde_json::to_vec(&query).ok()?;
    let digest = Sha256::digest(encoded);
    Some(format!("pay3flow:routes:v1:{digest:x}"))
}

#[cfg(test)]
mod tests {
    use axum::extract::Query;
    use axum::http::Uri;

    use super::{
        route_result_cache_key, P2pRouteSearchQuery, RouteHttpMetadata, VoteChoice, VoteRequest,
    };

    #[test]
    fn route_http_query_parses_numeric_and_boolean_url_values() {
        let uri: Uri = concat!(
            "/api/p2p/routes?source_fiat=USDC&target_fiat=RUB&source_amount=100",
            "&target_payment_method=Sberbank&sources=whitebird&allow_cross_venue=true",
            "&source_payment_fee_percent=0.75&target_payment_fee_percent=1.25",
            "&min_orders=20&min_completion_rate=0.9&limit=40",
            "&anonymous_id=aa1f91d5-410f-404d-85a5-de7438a29eb9"
        )
        .parse()
        .unwrap();

        let Query(query) = Query::<P2pRouteSearchQuery>::try_from_uri(&uri).unwrap();
        let Query(metadata) = Query::<RouteHttpMetadata>::try_from_uri(&uri).unwrap();

        assert_eq!(query.source_amount, 100.0);
        assert_eq!(query.min_orders, Some(20));
        assert_eq!(query.min_completion_rate, Some(0.9));
        assert_eq!(query.allow_cross_venue, Some(true));
        assert_eq!(query.source_payment_fee_percent, Some(0.75));
        assert_eq!(query.target_payment_fee_percent, Some(1.25));
        assert_eq!(query.limit, Some(40));
        assert_eq!(query.sources.as_deref(), Some("whitebird"));
        assert_eq!(
            metadata.anonymous_id.unwrap().to_string(),
            "aa1f91d5-410f-404d-85a5-de7438a29eb9"
        );
    }

    #[test]
    fn vote_request_accepts_an_anonymous_browser_vote() {
        let request: VoteRequest = serde_json::from_value(serde_json::json!({
            "anonymous_id": "aa1f91d5-410f-404d-85a5-de7438a29eb9",
            "vote": "dislike"
        }))
        .unwrap();

        assert_eq!(
            request.anonymous_id.to_string(),
            "aa1f91d5-410f-404d-85a5-de7438a29eb9"
        );
        assert_eq!(request.vote, VoteChoice::Dislike);
    }

    #[test]
    fn vote_request_requires_a_choice() {
        for vote in [serde_json::Value::Null, serde_json::json!("unknown")] {
            let payload = serde_json::json!({
                "anonymous_id": "aa1f91d5-410f-404d-85a5-de7438a29eb9",
                "vote": vote
            });

            assert!(serde_json::from_value::<VoteRequest>(payload).is_err());
        }
    }

    #[test]
    fn route_result_cache_key_uses_normalized_query_without_user_identity() {
        let mut first: P2pRouteSearchQuery = serde_json::from_value(serde_json::json!({
            "source_fiat": "AMD",
            "target_fiat": "RUB",
            "source_amount": 100000.0,
            "assets": "USDT, USDC",
            "sources": "binance,okx"
        }))
        .unwrap();
        let mut second = first.clone();
        first.source_fiat = " amd ".into();
        first.target_fiat = " rub ".into();
        first.assets = Some(" USDT , USDC ".into());
        second.assets = None;
        second.intermediary_assets = Some("USDT,USDC".into());
        second.sources = Some("OKX,BINANCE".into());

        assert_eq!(
            route_result_cache_key(&first),
            route_result_cache_key(&second)
        );
        second.sources = Some("not a valid source".into());
        assert!(route_result_cache_key(&second).is_none());
    }
}

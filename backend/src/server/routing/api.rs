use std::time::Duration;

use axum::extract::State;
use axum::http::{header::CONTENT_TYPE, HeaderValue, Request, Response};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::Router;
use scalar_api_reference::axum::scalar_response;
use serde_json::json;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::Level;

use crate::core::state::AppState;
use crate::external_reviews;
use crate::market_prices;
use crate::server::openapi;
use crate::server::routing::{
    activitypub, anonymous, auth, banks, exchange, matcher, networks, oauth, p2p, pairs, payments,
    payments_ws, providers, rates, referrals, route_executions, share_links, solver, ws,
};

pub fn router(state: AppState) -> Router {
    let scalar_configuration = json!({
        "url": "/openapi.json",
        "pageTitle": "Pay3Flow API Reference",
        "hideClientButton": true,
        "hideModels": false
    });

    // `scalar_api_reference` 0.1.x does not include its `scalar.js` asset in
    // the published crate. Its router therefore serves an HTML page that
    // points at a guaranteed 404 (`/scalar/scalar.js`). Use the crate's CDN
    // fallback instead, which keeps the documentation page functional.
    let scalar_routes = Router::new()
        .route(
            "/scalar",
            get(move || {
                let configuration = scalar_configuration.clone();
                async move { scalar_response(&configuration, None) }
            }),
        )
        .with_state(());

    let test_otc = crate::otc::TestOtc::default();
    Router::new()
        .route(
            "/ws/otc",
            get(move |ws| crate::server::routing::otc::ws(ws, test_otc.clone())),
        )
        .merge(scalar_routes)
        .route("/openapi.json", get(openapi::document))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/api/share-links", post(share_links::create))
        .route("/api/share-links/:id", get(share_links::get))
        .route("/api/anonymous/register", post(anonymous::register))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/referrals/me", get(referrals::profile))
        .route("/api/auth/oauth/{provider}", post(oauth::oauth))
        .route("/ws", get(ws::ws_handler))
        .route("/ws/rates", get(rates::rates_ws))
        .route("/ws/payments", get(payments_ws::payments_ws))
        .route("/ws/p2p/routes", get(p2p::routes_ws))
        .route("/api/payments", post(payments::create))
        .route("/api/payments", get(payments::list))
        .route("/api/payments/:id", get(payments::get))
        .route("/api/exchange/orders", post(exchange::create_order))
        .route("/api/exchange/orders", get(exchange::list_orders))
        .route("/api/exchange/corridors", get(exchange::list_corridors))
        .route("/api/exchange/orders/:id", get(exchange::get_order))
        .route("/api/exchange/orders/:id/quotes", get(exchange::get_quotes))
        .route("/api/exchange/orders/:id/live", get(exchange::live_routes))
        .route(
            "/api/exchange/orders/:id/discover",
            post(exchange::discover_solvers),
        )
        .route(
            "/api/exchange/orders/:id/auction",
            post(exchange::run_auction),
        )
        .route(
            "/api/exchange/orders/:id/confirm",
            post(exchange::confirm_order),
        )
        .route(
            "/api/exchange/orders/:id/funding/confirm",
            post(exchange::confirm_funding),
        )
        .route(
            "/api/exchange/orders/:id/manual-review",
            get(exchange::get_manual_review),
        )
        .route(
            "/api/exchange/orders/:id/settlement",
            get(exchange::get_settlement),
        )
        .route(
            "/api/exchange/orders/:id/ledger",
            get(exchange::get_ledger_operations),
        )
        .route("/api/exchange/orders/:id/proofs", get(exchange::get_proofs))
        .route(
            "/api/exchange/orders/:id/proof",
            post(exchange::submit_proof),
        )
        .route(
            "/api/debug/exchange/orders/:id/audit",
            get(exchange::get_audit_events),
        )
        .route(
            "/api/exchange/orders/:id/cancel",
            post(exchange::cancel_order),
        )
        .route("/api/solver/orders/open", get(solver::open_orders))
        .route("/api/solver/orders/:id/quotes", post(solver::submit_quote))
        .route("/api/exchange-pairs", get(pairs::list))
        .route("/api/admin/exchange-pairs", post(pairs::admin_create))
        .route("/api/admin/exchange-pairs/:id", post(pairs::admin_update))
        .route(
            "/api/admin/exchange/controls",
            get(exchange::admin_get_controls).post(exchange::admin_update_controls),
        )
        .route(
            "/api/admin/exchange/corridors/:id",
            post(exchange::admin_set_corridor_enabled),
        )
        .route(
            "/api/admin/exchange/solvers/:id",
            post(exchange::admin_set_solver_status),
        )
        .route(
            "/api/admin/exchange/orders/:id/manual-review",
            post(exchange::admin_resolve_manual_review),
        )
        .route(
            "/api/admin/exchange/orders/:id/dispute",
            post(exchange::admin_resolve_dispute),
        )
        .route("/api/banks", get(banks::list))
        .route("/api/networks", get(networks::list))
        .route("/api/market-prices", get(market_prices::market_prices))
        .route(
            "/api/reviews/providers/:slug",
            get(external_reviews::provider_reviews),
        )
        .route(
            "/api/reviews/profile",
            get(external_reviews::profile_reviews),
        )
        .route("/api/market-values", post(market_prices::market_values))
        .route("/api/p2p/search", get(p2p::search))
        .route("/api/p2p/routes", get(p2p::routes))
        .route("/api/p2p/route-activity", get(p2p::route_activity))
        .route("/api/p2p/route-executions", post(route_executions::create))
        .route("/api/p2p/route-executions/:id", get(route_executions::get))
        .route(
            "/api/p2p/route-executions/:id/submissions",
            post(route_executions::submit),
        )
        .route("/api/service-executions/open", post(p2p::open_execution))
        .route("/api/route-instructions/open", post(p2p::open_instruction))
        .route("/api/services/:id/vote", put(p2p::set_vote))
        .route("/api/routes/:route_id/vote", put(p2p::set_route_vote))
        .route("/api/admin/banks", post(banks::admin_create))
        .route(
            "/api/admin/banks/:name/status",
            post(banks::admin_set_status),
        )
        .route("/api/providers", get(providers::list))
        .route(
            "/api/providers/:slug/statistics",
            get(crate::provider_profile::statistics),
        )
        .route("/api/providers/:provider/webhooks", post(payments::webhook))
        .route("/api/debug/quote", post(rates::debug_quote))
        .route("/routing/fallback", post(matcher::fallback))
        .route("/.well-known/webfinger", get(activitypub::webfinger))
        .route("/actor", get(activitypub::actor_collection))
        .route("/actor/:handle", get(activitypub::actor_document_by_handle))
        .route(
            "/marketplace/resources/exchange",
            get(activitypub::exchange_resource),
        )
        .route(
            "/marketplace/proposals/exchange",
            get(activitypub::exchange_proposal),
        )
        .route(
            "/marketplace/interfaces/exchange",
            get(activitypub::exchange_interface),
        )
        .route(
            "/marketplace/shapes/exchange-input",
            get(activitypub::exchange_input_shape),
        )
        .route(
            "/marketplace/shapes/exchange-output",
            get(activitypub::exchange_output_shape),
        )
        .route("/marketplace/preview", post(activitypub::exchange_preview))
        .route("/candidates", get(activitypub::candidates))
        .route("/api/debug/task", post(activitypub::debug_task))
        .route("/inbox", post(activitypub::inbox_shared))
        .route("/inbox/:handle", post(activitypub::inbox_named))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(observe_request)
                .on_response(
                    |response: &Response<_>, latency: Duration, _span: &tracing::Span| {
                        let status = response.status();
                        crate::observability::request_finished(status.as_u16(), latency);
                        if status.is_client_error() || status.is_server_error() {
                            tracing::error!(%status, ?latency, "request failed");
                        } else {
                            tracing::info!(%status, ?latency, "request ok");
                        }
                    },
                )
                .on_failure(
                    |class: ServerErrorsFailureClass, latency: Duration, _span: &tracing::Span| {
                        tracing::error!(class = ?class, ?latency, "request error");
                    },
                ),
        )
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            anonymous::track_request,
        ))
        .with_state(state)
}

pub async fn health() -> &'static str {
    "ok"
}

fn observe_request<B>(_: &Request<B>, _: &tracing::Span) {
    crate::observability::request_started();
}

pub async fn metrics(State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let (anonymous_users, registered_users) = anonymous::count_users(&state.pool)
        .await
        .unwrap_or_default();
    let completed_swaps_last_hour = route_executions::count_completed_last_hour(&state.pool)
        .await
        .unwrap_or_default();
    let swap_searches_last_hour = state
        .reputation
        .count_searches_last_hour()
        .await
        .unwrap_or_default();
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        crate::observability::render(
            anonymous_users,
            registered_users,
            completed_swaps_last_hour,
            swap_searches_last_hour,
        ),
    )
}

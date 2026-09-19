use std::time::Duration;

use axum::http::Response;
use axum::routing::{get, post};
use axum::Router;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::Level;

use crate::core::state::AppState;
use crate::server::routing::{
    activitypub, auth, banks, exchange, matcher, oauth, p2p, pairs, payments, payments_ws, rates,
    solver, wallet, ws,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/oauth/{provider}", post(oauth::oauth))
        .route("/ws", get(ws::ws_handler))
        .route("/ws/rates", get(rates::rates_ws))
        .route("/ws/payments", get(payments_ws::payments_ws))
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
        .route("/api/p2p/search", get(p2p::search))
        .route("/api/p2p/routes", get(p2p::routes))
        .route("/api/admin/banks", post(banks::admin_create))
        .route("/api/admin/wallet", get(wallet::info))
        .route("/api/admin/wallet/balance", get(wallet::balance))
        .route("/api/admin/wallet/transfer", post(wallet::transfer))
        .route(
            "/api/admin/wallet/token-transfer",
            post(wallet::token_transfer),
        )
        .route(
            "/api/admin/banks/:name/status",
            post(banks::admin_set_status),
        )
        .route("/api/providers/:provider/webhooks", post(payments::webhook))
        .route("/api/debug/quote", post(rates::debug_quote))
        .route("/routing/fallback", post(matcher::fallback))
        .route("/.well-known/webfinger", get(activitypub::webfinger))
        .route("/actor", get(activitypub::actor_collection))
        .route("/actor/:handle", get(activitypub::actor_document_by_handle))
        .route("/candidates", get(activitypub::candidates))
        .route("/api/debug/task", post(activitypub::debug_task))
        .route("/inbox", post(activitypub::inbox_shared))
        .route("/inbox/:handle", post(activitypub::inbox_named))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(
                    |response: &Response<_>, latency: Duration, _span: &tracing::Span| {
                        let status = response.status();
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
        .with_state(state)
}

pub async fn health() -> &'static str {
    "ok"
}

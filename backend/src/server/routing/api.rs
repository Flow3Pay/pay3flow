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
    activitypub, auth, fake, matcher, oauth, payments, payments_ws, rates, ws,
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
        .route("/api/providers/:provider/webhooks", post(payments::webhook))
        .route("/api/debug/quote", post(rates::debug_quote))
        .route("/routing/fallback", post(matcher::fallback))
        .route("/api/debug/acquirers", get(fake::list))
        .route("/api/debug/acquirers/:slug", get(fake::by_slug))
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

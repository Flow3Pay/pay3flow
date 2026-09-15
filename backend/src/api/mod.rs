pub mod http;
pub mod ws;

use axum::Router;

pub fn router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(http::health))
        .route("/ws", axum::routing::get(ws::ws_handler))
}
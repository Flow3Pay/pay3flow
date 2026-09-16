use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::Json;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::core::state::AppState;
use crate::quotes::{self, Quote};
use crate::routing::PaymentRequest;

/// `GET /ws/rates` — live quoting over WebSocket.
///
/// The client decides the cadence (5/10/15/20s…): it keeps the socket open and
/// pushes a `{"type":"quote",...}` message whenever it wants a fresh offer.
/// Each message is answered with the ranked acquirer list fmatch produced for
/// the exchange pair (fallback when fmatch is down). fmatch is read-only.
pub async fn rates_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RateIn {
    /// Ask for a live quote. `id` is an optional client correlation tag,
    /// echoed back so the caller can match responses to requests.
    Quote {
        request: PaymentRequest,
        id: Option<String>,
    },
    Ping,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RateOut {
    Quote {
        id: Option<String>,
        quote: Box<Quote>,
    },
    Pong,
    Error {
        message: String,
    },
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut tx, mut rx) = socket.split();
    // Outbound writes happen from spawned quote tasks; a small channel fans
    // their replies back into the single socket writer.
    let (send_tx, mut send_rx) = tokio::sync::mpsc::channel::<Message>(16);
    tokio::spawn(async move {
        while let Some(msg) = send_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(msg)) = rx.next().await {
        match msg {
            Message::Text(text) => {
                let send_tx = send_tx.clone();
                let state = state.clone();
                tokio::spawn(async move {
                    let reply = match serde_json::from_str::<RateIn>(&text) {
                        Ok(RateIn::Quote { request, id }) => {
                            if request.amount <= 0.0 {
                                RateOut::Error {
                                    message: "amount must be positive".into(),
                                }
                            } else {
                                RateOut::Quote {
                                    id,
                                    quote: Box::new(quotes::compute_quote(&state, request).await),
                                }
                            }
                        }
                        Ok(RateIn::Ping) => RateOut::Pong,
                        Err(err) => RateOut::Error {
                            message: format!("invalid message: {err}"),
                        },
                    };
                    if let Ok(text) = serde_json::to_string(&reply) {
                        let _ = send_tx.send(Message::Text(text)).await;
                    }
                });
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

/// `POST /api/debug/quote` — the same live-quote pipeline over plain HTTP, for
/// curl smoke tests (parallel to `/api/debug/task`).
pub async fn debug_quote(
    State(state): State<AppState>,
    Json(request): Json<PaymentRequest>,
) -> Json<Quote> {
    Json(quotes::compute_quote(&state, request).await)
}

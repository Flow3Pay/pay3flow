use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

use crate::core::state::AppState;
use crate::payments::service::PaymentEvent;

/// `GET /ws/payments` — live payment status events (PLAN #44).
///
/// Unlike `/ws/rates` (pull), this stream is push: the client subscribes and
/// the server broadcasts every status transition of every payment through a
/// tokio broadcast channel. The optional leading `{"type":"subscribe"}` is
/// accepted for symmetry; the stream starts immediately.
pub async fn payments_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WsIn {
    Subscribe,
    Ping,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WsOut<'a> {
    Subscribed,
    Event(&'a PaymentEvent),
    Pong,
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut tx, mut rx) = socket.split();
    let (send_tx, mut send_rx) = tokio::sync::mpsc::channel::<Message>(32);
    tokio::spawn(async move {
        while let Some(msg) = send_rx.recv().await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut events = state.payments.subscribe();
    let events_tx = send_tx.clone();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            let msg = WsOut::Event(&event);
            if let Ok(text) = serde_json::to_string(&msg) {
                if events_tx.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    let _ = send_tx.send(Message::Text(
        serde_json::to_string(&WsOut::Subscribed).unwrap_or_default(),
    )).await;

    while let Some(Ok(msg)) = rx.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(WsIn::Ping) = serde_json::from_str::<WsIn>(&text) {
                    if let Ok(out) = serde_json::to_string(&WsOut::Pong) {
                        let _ = send_tx.send(Message::Text(out)).await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
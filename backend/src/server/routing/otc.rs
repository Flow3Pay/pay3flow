//! WebSocket transport for the test OTC workspace.
use crate::otc::{Draft, TestOtc};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    #[serde(rename_all = "camelCase")]
    Subscribe {
        session_id: Uuid,
        market_id: String,
    },
    Create {
        id: Uuid,
        order: Draft,
    },
    #[serde(rename_all = "camelCase")]
    Cancel {
        id: Uuid,
        order_id: Uuid,
    },
    Ping,
}
pub async fn ws(ws: WebSocketUpgrade, otc: TestOtc) -> Response {
    ws.max_message_size(16 * 1024)
        .on_upgrade(move |socket| handle(socket, otc))
}
async fn send(socket: &mut WebSocket, value: Value) -> Result<(), axum::Error> {
    tokio::time::timeout(
        Duration::from_secs(10),
        socket.send(Message::Text(value.to_string())),
    )
    .await
    .map_err(axum::Error::new)?
}
async fn handle(mut socket: WebSocket, otc: TestOtc) {
    let mut subscription: Option<(Uuid, String)> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let (id, response) = tokio::select! {
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<Request>(&text) {
                        Ok(Request::Subscribe { session_id, market_id }) => {
                            // A connection cannot change its owner after subscribing.
                            if subscription.as_ref().is_some_and(|(owner, _)| *owner != session_id) {
                                (None, Err("session_conflict"))
                            } else {
                                let snapshot = otc.snapshot(session_id, &market_id).await;
                                if snapshot.is_ok() { subscription = Some((session_id, market_id)); }
                                (None, snapshot)
                            }
                        },
                        Ok(Request::Create { id, order }) => {
                            let reply = match &subscription {
                                Some((session, _)) => otc.create(*session, id, order).await,
                                None => Err("subscribe_first"),
                            };
                            (Some(id), reply)
                        },
                        Ok(Request::Cancel { id, order_id }) => {
                            let reply = match &subscription {
                                Some((session, _)) => otc.cancel(*session, id, order_id).await,
                                None => Err("subscribe_first"),
                            };
                            (Some(id), reply)
                        },
                        Ok(Request::Ping) => (None, Ok(json!({"type": "pong"}))),
                        Err(_) => (None, Err("invalid_message")),
                    },
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() { break; }
                        continue;
                    },
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => continue,
                }
            },
            _ = tick.tick() => {
                let Some((session, market)) = &subscription else { continue; };
                (None, otc.snapshot(*session, market).await)
            }
        };
        let value =
            response.unwrap_or_else(|code| json!({"type": "error", "id": id, "code": code}));
        if send(&mut socket, value).await.is_err() {
            break;
        }
    }
}

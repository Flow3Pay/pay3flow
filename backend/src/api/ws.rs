use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures::{SinkExt, StreamExt};

pub async fn ws_handler(ws: WebSocketUpgrade) -> WebSocketUpgrade {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            Message::Text(text) => {
                let _ = socket.send(Message::Text(text)).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
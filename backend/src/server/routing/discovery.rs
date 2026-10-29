use std::time::Duration;
use axum::extract::{Query, State, WebSocketUpgrade, ws::{Message, WebSocket}};
use axum::response::Response;
use axum::Json;
use serde_json::json;
use crate::core::{error::AppError, state::AppState};
use crate::p2p::discovery::{self, DiscoveryQuery, DiscoveryResponse};

pub async fn search(State(state): State<AppState>, Query(query): Query<DiscoveryQuery>) -> Result<Json<DiscoveryResponse>, AppError> {
    discovery::run(&state,query,None).await.map(Json).map_err(|e|AppError::BadRequest(e.to_string()))
}
pub async fn socket(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.max_message_size(16384).on_upgrade(move |socket| stream(state,socket))
}
async fn stream(state: AppState, mut socket: WebSocket) {
    let payload=match tokio::time::timeout(Duration::from_secs(15),socket.recv()).await {
        Ok(Some(Ok(Message::Text(payload)))) => payload,
        _ => return,
    };
    let query=match serde_json::from_str::<DiscoveryQuery>(&payload) {
        Ok(query)=>query,
        Err(error)=>{let _=socket.send(Message::Text(json!({"type":"search_failed","error":error.to_string()}).to_string())).await;return;}
    };
    let (tx,mut rx)=tokio::sync::mpsc::channel(4);
    let search=discovery::run(&state,query,Some(tx)); tokio::pin!(search);
    loop {
        tokio::select! {
            result=&mut search => {
                let event=match result {Ok(response)=>json!({"type":"search_finished","response":response}),Err(error)=>json!({"type":"search_failed","error":error.to_string()})};
                let _=socket.send(Message::Text(event.to_string())).await;break;
            },
            update=rx.recv()=>{ if let Some(response)=update {if socket.send(Message::Text(json!({"type":"routes_updated","response":response}).to_string())).await.is_err(){break;}} },
            incoming=socket.recv()=>match incoming {Some(Ok(Message::Ping(bytes)))=>{if socket.send(Message::Pong(bytes)).await.is_err(){break;}},Some(Ok(Message::Pong(_)))=>{},_=>break},
        }
    }
    let _=socket.close().await;
}

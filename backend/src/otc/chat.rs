//! Authenticated P2P deal chat; message bodies are encrypted at rest.
use super::{
    error::{invalid, Error, Result},
    repo, Service,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

const HISTORY_LIMIT: i64 = 100;
const MESSAGE_LIMIT: usize = 2_000;

#[derive(Debug, Serialize)]
struct ChatMessage {
    sequence: i64,
    id: Uuid,
    sender: String,
    content: String,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Authenticate {
    Authenticate { token: String, trade_id: Uuid },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Send { content: String },
    Ping,
}

const HISTORY: &str = "SELECT id,message_id,sender_actor,ciphertext,created_at FROM (SELECT id,message_id,sender_actor,ciphertext,created_at FROM otc_p2p_messages WHERE trade_id=$1 ORDER BY id DESC LIMIT $2) recent ORDER BY id";
const AFTER: &str = "SELECT id,message_id,sender_actor,ciphertext,created_at FROM otc_p2p_messages WHERE trade_id=$1 AND id>$2 ORDER BY id LIMIT $3";
const INSERT: &str = "INSERT INTO otc_p2p_messages(message_id,trade_id,sender_actor,ciphertext) VALUES($1,$2,$3,$4) RETURNING id,message_id,sender_actor,ciphertext,created_at";

fn message(service: &Service, row: &tokio_postgres::Row) -> Result<ChatMessage> {
    Ok(ChatMessage {
        sequence: row.get("id"),
        id: row.get("message_id"),
        sender: row.get("sender_actor"),
        content: service
            .secrets
            .decrypt(&row.get::<_, String>("ciphertext"))?,
        created_at: row.get("created_at"),
    })
}

fn validate_content(content: &str) -> Result<&str> {
    let content = content.trim();
    if content.is_empty() || content.chars().count() > MESSAGE_LIMIT {
        return Err(invalid("message must contain 1 to 2000 characters"));
    }
    Ok(content)
}

impl Service {
    async fn participant_trade(
        &self,
        client: &(impl GenericClient + Sync),
        actor: &str,
        trade_id: Uuid,
    ) -> Result<()> {
        repo::p2p::trade_by_actor(client, &trade_id, actor)
            .await?
            .ok_or(Error::NotFound)?;
        Ok(())
    }

    async fn chat_messages(&self, trade_id: Uuid, after: Option<i64>) -> Result<Vec<ChatMessage>> {
        let client = self.pool.get().await?;
        let rows = if let Some(after) = after {
            client
                .query(AFTER, &[&trade_id, &after, &HISTORY_LIMIT])
                .await?
        } else {
            client.query(HISTORY, &[&trade_id, &HISTORY_LIMIT]).await?
        };
        rows.iter().map(|row| message(self, row)).collect()
    }

    async fn insert_chat_message(
        &self,
        actor: &str,
        trade_id: Uuid,
        content: &str,
    ) -> Result<ChatMessage> {
        let content = validate_content(content)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let lock_key = format!("otc-p2p-chat:{trade_id}");
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&lock_key],
        )
        .await?;
        self.participant_trade(&tx, actor, trade_id).await?;
        let encrypted = self.secrets.encrypt(content)?;
        let id = Uuid::new_v4();
        let row = tx
            .query_one(INSERT, &[&id, &trade_id, &actor, &encrypted])
            .await?;
        tx.commit().await?;
        message(self, &row)
    }
}

pub(crate) async fn ws(
    ws: WebSocketUpgrade,
    State(state): State<crate::core::state::AppState>,
) -> Response {
    ws.max_message_size(16 * 1024)
        .on_upgrade(move |socket| handle(socket, state.otc))
}

async fn send(socket: &mut WebSocket, value: &Value) -> std::result::Result<(), axum::Error> {
    socket.send(Message::Text(value.to_string())).await
}

async fn handle(mut socket: WebSocket, service: Service) {
    let auth = tokio::time::timeout(Duration::from_secs(10), socket.recv()).await;
    let Some(Ok(Message::Text(text))) = auth.ok().flatten() else {
        return;
    };
    let Ok(Authenticate::Authenticate { token, trade_id }) =
        serde_json::from_str::<Authenticate>(&text)
    else {
        let _ = send(
            &mut socket,
            &json!({"type":"error","code":"authenticate_first"}),
        )
        .await;
        return;
    };
    let Ok((_, actor)) = service.session(&token).await else {
        let _ = send(&mut socket, &json!({"type":"error","code":"unauthorized"})).await;
        return;
    };
    let client = match service.pool.get().await {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "otc.chat.database_unavailable");
            let _ = send(&mut socket, &json!({"type":"error","code":"unavailable"})).await;
            return;
        }
    };
    if let Err(error) = service.participant_trade(&client, &actor, trade_id).await {
        let _ = send(
            &mut socket,
            &json!({"type":"error","code":"trade_not_found"}),
        )
        .await;
        if !matches!(&error, Error::NotFound) {
            tracing::warn!(%error, "otc.chat.authorization_failed");
        }
        return;
    }

    let history = match service.chat_messages(trade_id, None).await {
        Ok(messages) => messages,
        Err(error) => {
            tracing::warn!(%error, "otc.chat.history_failed");
            let _ = send(&mut socket, &json!({"type":"error","code":"unavailable"})).await;
            return;
        }
    };
    let mut cursor = history.last().map_or(0, |item| item.sequence);
    let Ok(history) = serde_json::to_value(history) else {
        return;
    };
    if send(&mut socket, &json!({"type":"history","messages":history}))
        .await
        .is_err()
    {
        return;
    }

    let mut poll = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(700),
        Duration::from_millis(700),
    );
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<Request>(&text) {
                    Ok(Request::Send { content }) => match service.insert_chat_message(&actor, trade_id, &content).await {
                        Ok(message) => {
                            cursor = cursor.max(message.sequence);
                            if send(&mut socket, &json!({"type":"message","message":message})).await.is_err() { break; }
                        }
                        Err(Error::Invalid(_)) => {
                            if send(&mut socket, &json!({"type":"error","code":"invalid_message"})).await.is_err() { break; }
                        }
                        Err(error) => {
                            tracing::warn!(%error, trade=%trade_id, "otc.chat.send_failed");
                            if send(&mut socket, &json!({"type":"error","code":"unavailable"})).await.is_err() { break; }
                        }
                    },
                    Ok(Request::Ping) => {
                        if send(&mut socket, &json!({"type":"pong"})).await.is_err() { break; }
                    }
                    Err(_) => {
                        if send(&mut socket, &json!({"type":"error","code":"invalid_message"})).await.is_err() { break; }
                    }
                },
                Some(Ok(Message::Ping(payload))) => {
                    if socket.send(Message::Pong(payload)).await.is_err() { break; }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                _ => {}
            },
            _ = poll.tick() => match service.chat_messages(trade_id, Some(cursor)).await {
                Ok(messages) => {
                    for message in messages {
                        cursor = cursor.max(message.sequence);
                        if send(&mut socket, &json!({"type":"message","message":message})).await.is_err() { return; }
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, trade=%trade_id, "otc.chat.poll_failed");
                    if send(&mut socket, &json!({"type":"error","code":"unavailable"})).await.is_err() { break; }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_content;

    #[test]
    fn trims_messages_and_rejects_empty_or_oversized_content() {
        assert_eq!(validate_content("  hello  ").unwrap(), "hello");
        assert!(validate_content(" \n\t ").is_err());
        assert!(validate_content(&"x".repeat(2001)).is_err());
        assert!(validate_content(&"я".repeat(2000)).is_ok());
    }
}

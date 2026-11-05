use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::db::DbPool;

const ANONYMOUS_USER_HEADER: &str = "x-anonymous-user-id";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    pub anonymous_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub anonymous_id: Uuid,
}

/// Register the browser-generated pseudonymous identifier without collecting
/// an account, contact details, IP address, or device fingerprint.
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    let is_new = record_request(&state.pool, request.anonymous_id)
        .await
        .map_err(AppError::from)?;
    if is_new {
        notify_new_user(&state, request.anonymous_id);
    }
    Ok(Json(RegisterResponse {
        anonymous_id: request.anonymous_id,
    }))
}

/// Count one API request for a pseudonymous browser. Unknown IDs are accepted
/// so a lost registration request cannot make the application request fail.
pub async fn record_request(pool: &DbPool, anonymous_id: Uuid) -> anyhow::Result<bool> {
    let mut client = pool.get().await?;
    let transaction = client.transaction().await?;
    let is_new = transaction
        .execute(
            r#"
INSERT INTO anonymous_users (id, request_count)
VALUES ($1, 0)
ON CONFLICT (id) DO NOTHING
"#,
            &[&anonymous_id],
        )
        .await?
        == 1;
    let starts_session = transaction
        .query_one(
            r#"
SELECT last_session_at IS NULL
    OR last_seen_at <= now() - INTERVAL '30 minutes'
FROM anonymous_users
WHERE id = $1
FOR UPDATE
"#,
            &[&anonymous_id],
        )
        .await?
        .get::<_, bool>(0);
    transaction
        .execute(
            r#"
UPDATE anonymous_users
SET request_count = request_count + 1,
    last_seen_at = now(),
    last_session_at = CASE WHEN $2 THEN now() ELSE last_session_at END
WHERE id = $1
"#,
            &[&anonymous_id, &starts_session],
        )
        .await?;
    if starts_session {
        transaction
            .execute(
                "INSERT INTO anonymous_user_sessions (anonymous_id) VALUES ($1)",
                &[&anonymous_id],
            )
            .await?;
    }
    transaction.commit().await?;
    Ok(is_new)
}

fn notify_new_user(state: &AppState, anonymous_id: Uuid) {
    let Some(bot) = state.telegram.clone() else {
        return;
    };

    tokio::spawn(async move {
        if let Err(error) = bot.notify_new_user(anonymous_id).await {
            tracing::warn!(%error, "Telegram new-user notification failed");
        }
    });
}

pub async fn count_users(pool: &DbPool) -> anyhow::Result<(i64, i64)> {
    let client = pool.get().await?;
    let row = client
        .query_one(
            "SELECT (SELECT COUNT(*)::BIGINT FROM anonymous_users), \
                    (SELECT COUNT(*)::BIGINT FROM users)",
            &[],
        )
        .await?;
    Ok((row.get(0), row.get(1)))
}

pub async fn activity_counts(pool: &DbPool) -> anyhow::Result<(i64, i64, i64, i64)> {
    let client = pool.get().await?;
    let row = client
        .query_one(
            r#"
SELECT
    COUNT(DISTINCT anonymous_id) FILTER (
        WHERE started_at >= now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(DISTINCT anonymous_id) FILTER (
        WHERE started_at >= now() - INTERVAL '14 days'
          AND started_at < now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(*) FILTER (
        WHERE started_at >= now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(*) FILTER (
        WHERE started_at >= now() - INTERVAL '14 days'
          AND started_at < now() - INTERVAL '7 days'
    )::BIGINT
FROM anonymous_user_sessions
"#,
            &[],
        )
        .await?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

/// Attach usage accounting to every request carrying the anonymous ID. The
/// ID is deliberately read only from a client-provided pseudonymous header;
/// no identifying request metadata is stored.
pub async fn track_request(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() != "/api/anonymous/register" {
        if let Some(anonymous_id) = request_anonymous_id(&request) {
            match record_request(&state.pool, anonymous_id).await {
                Ok(true) => notify_new_user(&state, anonymous_id),
                Ok(false) => {}
                Err(error) => tracing::warn!(error = %error, "anonymous request accounting failed"),
            }
        }
    }

    next.run(request).await
}

fn request_anonymous_id(request: &Request) -> Option<Uuid> {
    request
        .headers()
        .get(ANONYMOUS_USER_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Uuid>().ok())
        .or_else(|| {
            request
                .uri()
                .query()?
                .split('&')
                .find_map(|part| part.strip_prefix("anonymous_id=")?.parse().ok())
        })
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::extract::Request;

    use super::{request_anonymous_id, RegisterRequest};

    #[test]
    fn register_request_rejects_extra_data() {
        let payload = serde_json::json!({
            "anonymous_id": "aa1f91d5-410f-404d-85a5-de7438a29eb9",
            "email": "not-collected@example.com"
        });

        assert!(serde_json::from_value::<RegisterRequest>(payload).is_err());
    }

    #[test]
    fn request_id_prefers_header_and_supports_websocket_query() {
        let request = Request::builder()
            .uri("/ws/p2p/routes?anonymous_id=aa1f91d5-410f-404d-85a5-de7438a29eb9")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            request_anonymous_id(&request).unwrap().to_string(),
            "aa1f91d5-410f-404d-85a5-de7438a29eb9"
        );

        let request = Request::builder()
            .uri("/api/providers?anonymous_id=00000000-0000-0000-0000-000000000000")
            .header(
                "x-anonymous-user-id",
                "aa1f91d5-410f-404d-85a5-de7438a29eb9",
            )
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            request_anonymous_id(&request).unwrap().to_string(),
            "aa1f91d5-410f-404d-85a5-de7438a29eb9"
        );
    }
}

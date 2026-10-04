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
    record_request(&state.pool, request.anonymous_id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(RegisterResponse {
        anonymous_id: request.anonymous_id,
    }))
}

/// Count one API request for a pseudonymous browser. Unknown IDs are accepted
/// so a lost registration request cannot make the application request fail.
pub async fn record_request(pool: &DbPool, anonymous_id: Uuid) -> anyhow::Result<()> {
    let client = pool.get().await?;
    client
        .execute(
            r#"
INSERT INTO anonymous_users (id, request_count)
VALUES ($1, 1)
ON CONFLICT (id) DO UPDATE SET
    request_count = anonymous_users.request_count + 1,
    last_seen_at = now()
"#,
            &[&anonymous_id],
        )
        .await?;
    Ok(())
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
            if let Err(error) = record_request(&state.pool, anonymous_id).await {
                tracing::warn!(error = %error, "anonymous request accounting failed");
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

use std::net::IpAddr;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::HeaderMap;
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
/// an account, contact details, or device fingerprint. The request IP is only
/// used transiently to derive a coarse region for the operator notification;
/// it is never stored.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    let is_new = record_request(&state.pool, request.anonymous_id)
        .await
        .map_err(AppError::from)?;
    if is_new {
        notify_new_user(&state, request.anonymous_id, client_ip(&headers));
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

fn notify_new_user(state: &AppState, anonymous_id: Uuid, ip: Option<IpAddr>) {
    let Some(bot) = state.telegram.clone() else {
        return;
    };

    tokio::spawn(async move {
        let region = resolve_region(ip).await;
        if let Err(error) = bot.notify_new_user(anonymous_id, &region).await {
            tracing::warn!(%error, "Telegram new-user notification failed");
        }
    });
}

/// Best-effort country/region lookup for the new-user notification. Runs off
/// the request path and falls back to a placeholder on any failure, so a slow
/// or blocked geo service can never delay or fail registration.
async fn resolve_region(ip: Option<IpAddr>) -> String {
    let Some(ip) = ip else {
        return "неизвестен".to_string();
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(client) => client,
        Err(_) => return "неизвестен".to_string(),
    };
    let response = client
        .get(format!(
            "https://ipwho.is/{ip}?fields=success,country,region,city"
        ))
        .send()
        .await;
    let Ok(response) = response else {
        return "неизвестен".to_string();
    };
    let Ok(body) = response.json::<serde_json::Value>().await else {
        return "неизвестен".to_string();
    };
    if body.get("success").and_then(|value| value.as_bool()) != Some(true) {
        return "неизвестен".to_string();
    }

    let mut parts: Vec<&str> = Vec::new();
    for key in ["city", "region", "country"] {
        let Some(value) = body.get(key).and_then(|value| value.as_str()) else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() && !parts.contains(&value) {
            parts.push(value);
        }
    }
    if parts.is_empty() {
        "неизвестен".to_string()
    } else {
        parts.join(", ")
    }
}

/// Trust the first hop of `X-Forwarded-For`, which Caddy fills with the real
/// client address, and fall back to `X-Real-IP`.
pub(crate) fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<IpAddr>().ok())
        })
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
            let ip = client_ip(request.headers());
            match record_request(&state.pool, anonymous_id).await {
                Ok(true) => notify_new_user(&state, anonymous_id, ip),
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
    use axum::http::HeaderMap;

    use super::{client_ip, request_anonymous_id, RegisterRequest};

    #[test]
    fn register_request_rejects_extra_data() {
        let payload = serde_json::json!({
            "anonymous_id": "aa1f91d5-410f-404d-85a5-de7438a29eb9",
            "email": "not-collected@example.com"
        });

        assert!(serde_json::from_value::<RegisterRequest>(payload).is_err());
    }

    #[test]
    fn client_ip_uses_first_forwarded_hop_then_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        headers.insert("x-real-ip", "198.51.100.9".parse().unwrap());
        assert_eq!(client_ip(&headers).unwrap().to_string(), "203.0.113.7");

        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.9".parse().unwrap());
        assert_eq!(client_ip(&headers).unwrap().to_string(), "198.51.100.9");

        assert!(client_ip(&HeaderMap::new()).is_none());
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

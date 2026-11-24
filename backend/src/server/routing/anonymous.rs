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
use crate::db::{repo, DbPool};

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
        let ua = user_agent(&headers);
        notify_new_user(&state, request.anonymous_id, client_ip(&headers), ua);
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
    let is_new = repo::anonymous::insert_user(&transaction, &anonymous_id).await?;
    let starts_session = repo::anonymous::lock_user(&transaction, &anonymous_id).await?;
    repo::anonymous::bump_request(&transaction, &anonymous_id, starts_session).await?;
    if starts_session {
        repo::anonymous::insert_session(&transaction, &anonymous_id).await?;
    }
    transaction.commit().await?;
    Ok(is_new)
}

fn notify_new_user(
    state: &AppState,
    anonymous_id: Uuid,
    ip: Option<IpAddr>,
    user_agent: Option<String>,
) {
    let Some(bot) = state.telegram.clone() else {
        return;
    };

    tokio::spawn(async move {
        let region = resolve_region(ip).await;
        if let Err(error) = bot
            .notify_new_user(anonymous_id, &region, user_agent.as_deref())
            .await
        {
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

fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Prefer headers commonly set to the original client address by a trusted
/// edge proxy; use the first forwarded hop only as a fallback.
fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    for name in [
        "cf-connecting-ip",
        "true-client-ip",
        "x-real-ip",
        "x-forwarded-for",
    ] {
        let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) else {
            continue;
        };
        let Some(ip) = value
            .split(',')
            .next()
            .and_then(|value| value.trim().parse::<IpAddr>().ok())
        else {
            continue;
        };
        return Some(ip);
    }
    None
}

pub async fn count_users(pool: &DbPool) -> anyhow::Result<(i64, i64)> {
    let client = pool.get().await?;
    repo::anonymous::count_users(&client).await
}

pub async fn activity_counts(pool: &DbPool) -> anyhow::Result<(i64, i64, i64, i64)> {
    let client = pool.get().await?;
    repo::anonymous::activity_counts(&client).await
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
            let ua = user_agent(request.headers());
            match record_request(&state.pool, anonymous_id).await {
                Ok(true) => notify_new_user(&state, anonymous_id, ip, ua),
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
    fn client_ip_prefers_origin_headers_over_forwarded_proxy_hops() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        headers.insert("x-real-ip", "198.51.100.9".parse().unwrap());
        assert_eq!(client_ip(&headers).unwrap().to_string(), "198.51.100.9");

        headers.insert("true-client-ip", "192.0.2.10".parse().unwrap());
        headers.insert("cf-connecting-ip", "192.0.2.20".parse().unwrap());
        assert_eq!(client_ip(&headers).unwrap().to_string(), "192.0.2.20");

        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
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

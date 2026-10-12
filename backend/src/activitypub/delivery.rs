use std::time::Duration;

use serde_json::Value;
use tokio_postgres::types::ToSql;

use crate::activitypub::actor::ActorIdentity;
use crate::activitypub::error::ActivityPubError;
use crate::activitypub::signature::sign_headers;
use crate::activitypub::ACTIVITY_JSON;
use crate::db::DbPool;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RETRIES: u32 = 3;
const BASE_BACKOFF_MS: u64 = 500;

/// Outbound ActivityPub delivery: sign, retry transient failures, dedupe by
/// activity id so a submission is never applied twice.
///
/// `pool` is optional purely so the failure path (unreachable/erroring inbox)
/// is unit-testable without a Postgres instance; production code always passes
/// `Some`. When `None`, dedupe is skipped (nothing is read or recorded).
#[derive(Clone)]
pub struct DeliveryClient {
    http: reqwest::Client,
    pool: Option<DbPool>,
    max_retries: u32,
    base_backoff_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeliveryOutcome {
    Delivered,
    AlreadyDelivered,
    Rejected { status: u16 },
}

impl DeliveryClient {
    pub fn new(pool: DbPool) -> Self {
        let http = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("reqwest client");
        Self {
            http,
            pool: Some(pool),
            max_retries: MAX_RETRIES,
            base_backoff_ms: BASE_BACKOFF_MS,
        }
    }

    /// Test-only client: no dedupe ledger (the DB table is never touched) and
    /// a short backoff so the "fmatch is down" retry loop finishes fast.
    #[cfg(test)]
    pub fn for_test(max_retries: u32, base_backoff_ms: u64) -> Self {
        let http = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("reqwest client");
        Self {
            http,
            pool: None,
            max_retries,
            base_backoff_ms,
        }
    }

    /// Deliver a signed activity to `inbox`; idempotent per activity `id`.
    pub async fn deliver(
        &self,
        identity: &ActorIdentity,
        inbox: &str,
        activity: &Value,
    ) -> Result<DeliveryOutcome, ActivityPubError> {
        self.post(identity, inbox, activity, false, true)
            .await
            .map(|(outcome, _)| outcome)
    }

    /// Deliver a signed activity and return the response JSON body (when the
    /// peer answered inline — e.g. fmatch's candidate list for a discovery
    /// request).
    pub async fn deliver_with_response(
        &self,
        identity: &ActorIdentity,
        inbox: &str,
        activity: &Value,
    ) -> Result<(DeliveryOutcome, Option<Value>), ActivityPubError> {
        self.post(identity, inbox, activity, true, true).await
    }

    /// Deliver a one-shot request and return its inline response without
    /// touching the delivery ledger.
    ///
    /// Request activities use a fresh id and are never replayed after the
    /// response has been returned, so the ledger cannot deduplicate useful
    /// work. Skipping it also keeps two database round trips out of latency-
    /// sensitive discovery requests. Retries inside this call still reuse the
    /// same activity id and remain idempotent at the receiver.
    pub async fn deliver_ephemeral_with_response(
        &self,
        identity: &ActorIdentity,
        inbox: &str,
        activity: &Value,
    ) -> Result<(DeliveryOutcome, Option<Value>), ActivityPubError> {
        self.post(identity, inbox, activity, true, false).await
    }

    async fn post(
        &self,
        identity: &ActorIdentity,
        inbox: &str,
        activity: &Value,
        capture_body: bool,
        track_delivery: bool,
    ) -> Result<(DeliveryOutcome, Option<Value>), ActivityPubError> {
        let activity_id = activity.get("id").and_then(Value::as_str).ok_or_else(|| {
            ActivityPubError::InvalidActivity {
                type_name: activity
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                detail: "activity is missing an id (idempotency key)".into(),
            }
        })?;

        if track_delivery && self.was_delivered(activity_id, inbox).await? {
            return Ok((DeliveryOutcome::AlreadyDelivered, None));
        }

        let parsed: reqwest::Url = inbox
            .parse()
            .map_err(|e| ActivityPubError::Delivery(format!("invalid inbox url {inbox}: {e}")))?;
        let host = parsed
            .host_str()
            .map(|h| format!("{h}{}", port_suffix(&parsed)))
            .unwrap_or_default();
        let uri = match parsed.query() {
            Some(q) => format!("{}?{q}", parsed.path()),
            None => parsed.path().to_string(),
        };
        let body = serde_json::to_vec(activity)
            .map_err(|e| ActivityPubError::Other(format!("serialize activity: {e}")))?;

        let mut attempt: u32 = 0;
        loop {
            let hdrs = sign_headers(identity, "POST", &uri, &host, &body, chrono::Utc::now())?;
            let builder = self.http.post(inbox).body(body.clone());
            let request = hdrs
                .into_iter()
                .fold(builder, |b, (k, v)| b.header(k, v))
                .header(axum::http::header::CONTENT_TYPE, ACTIVITY_JSON);

            match request.send().await {
                Ok(res) => {
                    let status = res.status();
                    if status.is_success() {
                        let value = if capture_body {
                            serde_json::from_str(&res.text().await.unwrap_or_default()).ok()
                        } else {
                            None
                        };
                        if track_delivery {
                            self.record_delivery(activity_id, inbox).await?;
                        }
                        return Ok((DeliveryOutcome::Delivered, value));
                    }
                    let response_body = res.text().await.unwrap_or_default();
                    if is_no_candidates_rejection(status.as_u16(), &response_body)
                        || !retryable(status.as_u16())
                        || attempt >= self.max_retries
                    {
                        return Err(delivery_response_error(
                            status.as_u16(),
                            activity_id,
                            &response_body,
                        ));
                    }
                }
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(ActivityPubError::Delivery(format!("network error: {e}")));
                    }
                }
            }
            attempt += 1;
            tokio::time::sleep(Duration::from_millis(self.base_backoff_ms * (1 << attempt))).await;
        }
    }

    /// Verify an inbound signature against the signing actor's public key,
    /// fetched live from its actor document (cached per keyId for 5m).
    pub async fn fetch_public_key_for_verify(
        &self,
        identity: &ActorIdentity,
        key_id: &str,
    ) -> Result<String, ActivityPubError> {
        let actor_iri = key_id.split('#').next().unwrap_or(key_id);
        // keyId is usually <actor>#main-key; key owner equals actor id.
        if identity.public_key_id() == key_id {
            return Ok(identity.public_key_pem().to_string());
        }
        let doc = identity.fetch_remote_actor(&self.http, actor_iri).await?;
        doc.public_key
            .filter(|pk| pk.id == key_id && pk.owner == actor_iri)
            .map(|pk| pk.public_key_pem)
            .ok_or_else(|| {
                ActivityPubError::Signature(format!("actor {actor_iri} has no publicKey"))
            })
    }

    async fn was_delivered(
        &self,
        activity_id: &str,
        inbox: &str,
    ) -> Result<bool, ActivityPubError> {
        let Some(pool) = &self.pool else {
            // test-only client without a ledger: dedupe is skipped
            return Ok(false);
        };
        let client = pool
            .get()
            .await
            .map_err(|e| ActivityPubError::Other(format!("db: {e}")))?;
        let row = client
            .query_opt(
                "SELECT 1 FROM activitypub_deliveries WHERE activity_id = $1 AND target = $2",
                &[&activity_id, &inbox],
            )
            .await
            .map_err(|e| ActivityPubError::Other(format!("db: {e}")))?;
        Ok(row.is_some())
    }

    async fn record_delivery(
        &self,
        activity_id: &str,
        inbox: &str,
    ) -> Result<(), ActivityPubError> {
        let Some(pool) = &self.pool else {
            // test-only client: nothing is recorded
            return Ok(());
        };
        let client = pool
            .get()
            .await
            .map_err(|e| ActivityPubError::Other(format!("db: {e}")))?;
        client
            .execute(
                "INSERT INTO activitypub_deliveries (activity_id, target) VALUES ($1, $2)
                 ON CONFLICT (activity_id, target) DO NOTHING",
                &[
                    &activity_id as &(dyn ToSql + Sync),
                    &inbox as &(dyn ToSql + Sync),
                ],
            )
            .await
            .map_err(|e| ActivityPubError::Other(format!("db: {e}")))?;
        Ok(())
    }
}

fn retryable(status: u16) -> bool {
    status == 429 || status >= 500
}

fn is_no_candidates_rejection(status: u16, body: &str) -> bool {
    status == 503
        && serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .is_some_and(|status| status == "no-candidates")
}

fn delivery_response_error(status: u16, activity_id: &str, body: &str) -> ActivityPubError {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|value| {
        let workflow_status = value.get("status").and_then(Value::as_str);
        let summary = value.get("summary").and_then(Value::as_str);
        match (workflow_status, summary) {
            (Some(workflow_status), Some(summary)) => Some(format!("{workflow_status}: {summary}")),
            (Some(workflow_status), None) => Some(workflow_status.to_string()),
            (None, Some(summary)) => Some(summary.to_string()),
            (None, None) => None,
        }
    });
    let detail = detail
        .map(|detail| detail.chars().take(512).collect::<String>())
        .map(|detail| format!(": {detail}"))
        .unwrap_or_default();
    ActivityPubError::Delivery(format!("http {status} for {activity_id}{detail}"))
}

fn port_suffix(url: &reqwest::Url) -> String {
    match url.port() {
        Some(port) => format!(":{port}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod port_tests {
    #[test]
    fn path_with_query() {
        let url: reqwest::Url = "http://localhost:7277/inbox/actra?v=1".parse().unwrap();
        assert_eq!(url.path(), "/inbox/actra");
        assert_eq!(url.query(), Some("v=1"));
    }
}

#[cfg(test)]
mod fmatch_down_tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{http::StatusCode, routing::post, Json, Router};
    use serde_json::json;

    use super::*;
    use crate::activitypub::actor::ActorIdentity;

    /// "fmatch упал" — the fmatch inbox is unreachable (connection refused on a
    /// closed local port). `deliver` must exhaust its retries and surface a
    /// `Delivery` error instead of panicking, and it must never fake success
    /// (nothing is deduped/recorded when `pool: None`).
    #[tokio::test]
    async fn fmatch_unreachable_surfaces_delivery_error_after_retries() {
        let key_path = std::env::temp_dir()
            .join("pay3flow-fmatch-down-test")
            .join("id.pem");
        if let Some(parent) = key_path.parent() {
            std::fs::create_dir_all(parent).expect("test key dir");
        }
        let identity = ActorIdentity::load_or_create(
            key_path.to_str().expect("valid utf8 key path"),
            "https://pay3flow.local",
            "pay3flow",
        )
        .expect("actor identity");

        let client = DeliveryClient::for_test(2, 5);
        let inbox = "http://127.0.0.1:1/inbox/fmatch";
        let activity = json!({
            "id": "https://pay3flow.local/activities/fmatch-down-unit",
            "type": "Follow",
            "actor": "https://pay3flow.local/actor/pay3flow",
            "object": "https://fmatch.local/actor/fmatch"
        });

        let err = client
            .deliver(&identity, inbox, &activity)
            .await
            .expect_err("fmatch is down => delivery must fail");

        assert!(
            matches!(err, ActivityPubError::Delivery(_)),
            "expected ActivityPubError::Delivery, got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn no_candidates_rejection_is_not_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let handler_attempts = attempts.clone();
        let app = Router::new().route(
            "/",
            post(move || {
                let handler_attempts = handler_attempts.clone();
                async move {
                    handler_attempts.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({
                            "type": "Reject",
                            "status": "no-candidates",
                            "summary": "no active P2P offers"
                        })),
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let server =
            tokio::spawn(async move { axum::serve(listener, app).await.expect("serve rejection") });
        let key_path = std::env::temp_dir()
            .join(format!("pay3flow-no-candidates-{}", uuid::Uuid::new_v4()))
            .join("id.pem");
        let identity = ActorIdentity::load_or_create(
            key_path.to_str().expect("valid utf8 key path"),
            "https://pay3flow.local",
            "pay3flow",
        )
        .expect("actor identity");
        let client = DeliveryClient::for_test(3, 5);
        let activity = json!({
            "id": "https://pay3flow.local/activities/no-candidates",
            "type": "Proposal"
        });

        let error = client
            .deliver(&identity, &format!("http://{address}/"), &activity)
            .await
            .expect_err("semantic rejection must surface");
        server.abort();

        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert!(error.to_string().contains("no active P2P offers"));
    }
}

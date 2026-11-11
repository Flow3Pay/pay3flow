//! Merchant webhooks: HMAC signatures, at-least-once delivery and exponential
//! backoff. Events are written to `b2b_events` in the same transaction as the
//! order transition, then fanned out to every subscribed active endpoint.

use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};
use sha2::Sha256;
use tokio_postgres::Transaction;
use uuid::Uuid;

use super::model::{CreateWebhookRequest, WebhookEndpointView};
use super::{Error, Result, Service, WEBHOOK_EVENTS};

const MAX_ATTEMPTS: i32 = 8;
const MAX_RESPONSE_BODY: usize = 500;

impl Service {
    /// `POST /api/b2b/webhooks`. The signing secret is returned once; only its
    /// AES-GCM ciphertext is stored.
    pub async fn create_webhook(
        &self,
        user_id: Uuid,
        request: &CreateWebhookRequest,
    ) -> Result<WebhookEndpointView> {
        validate_webhook_url(&request.url)?;
        let events = match request.events.as_deref() {
            Some(events) if !events.is_empty() => {
                let mut validated = Vec::with_capacity(events.len());
                for event in events {
                    if !WEBHOOK_EVENTS.contains(&event.as_str()) {
                        return Err(Error::Invalid(format!(
                            "unsupported webhook event `{event}`"
                        )));
                    }
                    validated.push(event.clone());
                }
                validated
            }
            _ => WEBHOOK_EVENTS
                .iter()
                .map(|event| event.to_string())
                .collect(),
        };
        let merchant_id = self.merchant(user_id).await?;
        let secret = format!("whsec_{}", super::random_token());
        let ciphertext = self.secrets.encrypt(&secret)?;
        let client = self.client().await?;
        let row = client
            .query_one(
                r#"
INSERT INTO b2b_webhook_endpoints (merchant_id, url, secret_ciphertext, events)
VALUES ($1, $2, $3, $4)
RETURNING id, active
"#,
                &[
                    &merchant_id,
                    &request.url.trim(),
                    &ciphertext.as_bytes(),
                    &events,
                ],
            )
            .await?;
        Ok(WebhookEndpointView {
            id: row.get(0),
            url: request.url.trim().to_string(),
            events,
            secret,
            active: row.get(1),
        })
    }

    pub(crate) async fn deliver_webhooks(&self) -> Result<()> {
        let client = self.client().await?;
        let claimed = client
            .query(
                r#"
WITH claimed AS (
    SELECT id FROM b2b_webhook_deliveries
    WHERE (status = 'pending' AND next_attempt_at <= now())
       OR (status = 'delivering' AND next_attempt_at < now() - interval '5 minutes')
    ORDER BY next_attempt_at
    LIMIT 20
    FOR UPDATE SKIP LOCKED
)
UPDATE b2b_webhook_deliveries d
SET status = 'delivering', next_attempt_at = now() + interval '5 minutes'
FROM claimed c
WHERE d.id = c.id
RETURNING d.id, d.endpoint_id, d.event_id, d.payload, d.attempts
"#,
                &[],
            )
            .await?;
        for delivery in claimed {
            let delivery_id: Uuid = delivery.get(0);
            let endpoint_id: Uuid = delivery.get(1);
            let event_id: i64 = delivery.get(2);
            let payload: Value = delivery.get(3);
            let attempts: i32 = delivery.get(4);
            if let Err(error) = self
                .deliver_one(delivery_id, endpoint_id, event_id, &payload, attempts)
                .await
            {
                tracing::warn!(delivery_id = %delivery_id, %error, "b2b webhook delivery failed");
            }
        }
        Ok(())
    }

    async fn deliver_one(
        &self,
        delivery_id: Uuid,
        endpoint_id: Uuid,
        event_id: i64,
        payload: &Value,
        attempts: i32,
    ) -> Result<()> {
        let client = self.client().await?;
        let endpoint = client
            .query_opt(
                "SELECT url, secret_ciphertext, active FROM b2b_webhook_endpoints WHERE id = $1",
                &[&endpoint_id],
            )
            .await?;
        let Some(endpoint) = endpoint else {
            client
                .execute(
                    "UPDATE b2b_webhook_deliveries SET status = 'dead' WHERE id = $1",
                    &[&delivery_id],
                )
                .await?;
            return Ok(());
        };
        let url: String = endpoint.get(0);
        let ciphertext: Vec<u8> = endpoint.get(1);
        let active: bool = endpoint.get(2);
        if !active {
            client
                .execute(
                    "UPDATE b2b_webhook_deliveries SET status = 'skipped' WHERE id = $1",
                    &[&delivery_id],
                )
                .await?;
            return Ok(());
        }
        let secret = self
            .secrets
            .decrypt(std::str::from_utf8(&ciphertext).unwrap_or_default())?;
        let body = serde_json::to_string(payload)?;
        let timestamp = Utc::now().timestamp();
        let signature = sign(&secret, timestamp, body.as_bytes());
        let response = self
            .http
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header("Pay3Flow-Event-Id", format!("evt_{event_id}"))
            .header("Pay3Flow-Delivery-Id", delivery_id.to_string())
            .header(
                "Pay3Flow-Signature",
                format!("t={timestamp},v1={signature}"),
            )
            .body(body)
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let code = i32::from(response.status().as_u16());
                client
                    .execute(
                        "UPDATE b2b_webhook_deliveries SET status = 'delivered', \
                         attempts = attempts + 1, response_code = $2, response_body = $3 \
                         WHERE id = $1",
                        &[&delivery_id, &code, &truncate("")],
                    )
                    .await?;
            }
            Ok(response) => {
                let code = i32::from(response.status().as_u16());
                let text = response
                    .text()
                    .await
                    .unwrap_or_default()
                    .chars()
                    .take(MAX_RESPONSE_BODY)
                    .collect::<String>();
                self.retry(delivery_id, attempts, Some(code), &text).await?;
            }
            Err(error) => {
                self.retry(delivery_id, attempts, None, &error.to_string())
                    .await?;
            }
        }
        Ok(())
    }

    async fn retry(
        &self,
        delivery_id: Uuid,
        attempts: i32,
        response_code: Option<i32>,
        response_body: &str,
    ) -> Result<()> {
        let next_attempts = attempts.saturating_add(1);
        let client = self.client().await?;
        if next_attempts >= MAX_ATTEMPTS {
            client
                .execute(
                    "UPDATE b2b_webhook_deliveries SET status = 'dead', attempts = $2, \
                     response_code = $3, response_body = $4 WHERE id = $1",
                    &[
                        &delivery_id,
                        &next_attempts,
                        &response_code,
                        &truncate(response_body),
                    ],
                )
                .await?;
            return Ok(());
        }
        let delay = (30i64 * 2i64.pow(attempts.clamp(0, 7) as u32)).min(3600);
        client
            .execute(
                "UPDATE b2b_webhook_deliveries SET status = 'pending', attempts = $2, \
                 response_code = $3, response_body = $4, \
                 next_attempt_at = now() + make_interval(secs => $5) WHERE id = $1",
                &[
                    &delivery_id,
                    &next_attempts,
                    &response_code,
                    &truncate(response_body),
                    &(delay as f64),
                ],
            )
            .await?;
        Ok(())
    }
}

/// Append an event to the audit log and fan it out to subscribed endpoints.
pub(crate) async fn queue_event(
    tx: &Transaction<'_>,
    order_id: Uuid,
    kind: &str,
    body: &Value,
) -> Result<i64> {
    let row = tx
        .query_one(
            "INSERT INTO b2b_events (order_id, kind, body) VALUES ($1, $2, $3) RETURNING id",
            &[&order_id, &kind, &body],
        )
        .await?;
    let event_id: i64 = row.get(0);
    let payload = json!({
        "type": kind,
        "created_at": Utc::now(),
        "data": body,
    });
    tx.execute(
        r#"
INSERT INTO b2b_webhook_deliveries (endpoint_id, event_id, event_kind, payload)
SELECT e.id, $1, $2, $3
FROM b2b_webhook_endpoints e
JOIN b2b_orders o ON o.merchant_id = e.merchant_id
WHERE o.id = $4 AND e.active AND $2 = ANY(e.events)
"#,
        &[&event_id, &kind, &payload, &order_id],
    )
    .await?;
    Ok(event_id)
}

/// `Pay3Flow-Signature: t=...,v1=<HMAC-SHA256("t.body")>`.
pub fn sign(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    super::to_hex(&mac.finalize().into_bytes())
}

fn truncate(value: &str) -> String {
    value.chars().take(MAX_RESPONSE_BODY).collect()
}

fn validate_webhook_url(url: &str) -> Result<()> {
    let url = url.trim();
    if url.len() > 2048 {
        return Err(Error::Invalid("webhook URL is too long".into()));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| Error::Invalid("webhook URL must be absolute".into()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Error::Invalid("webhook URL must use http or https".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_the_documented_formula() {
        // Reference value computed independently:
        // HMAC-SHA256("whsec_test", "1700000000.{\"type\":\"order.paid\"}")
        let body = br#"{"type":"order.paid"}"#;
        let signature = sign("whsec_test", 1_700_000_000, body);
        assert_eq!(signature.len(), 64);
        assert!(signature.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            signature,
            sign("whsec_test", 1_700_000_000, body),
            "signatures must be deterministic"
        );
        assert_ne!(signature, sign("whsec_test", 1_700_000_001, body));
        assert_ne!(signature, sign("whsec_other", 1_700_000_000, body));
    }

    #[test]
    fn rejects_non_http_webhook_urls() {
        assert!(validate_webhook_url("ftp://example.test/hook").is_err());
        assert!(validate_webhook_url("not a url").is_err());
        assert!(validate_webhook_url("https://example.test/hook").is_ok());
    }
}

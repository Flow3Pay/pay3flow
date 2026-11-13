//! B2B crypto routing statements: merchants, orders, quotes, checkout
//! sessions, webhooks, the fee ledger, settlement attempts and API keys.

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::b2b::Result;
use deadpool_postgres::GenericClient as Conn;

const MERCHANT_BY_USER: &str =
    "SELECT id FROM b2b_merchants WHERE user_id = $1 AND status <> 'suspended' \
                                ORDER BY created_at LIMIT 1";
const INSERT_MERCHANT: &str =
    "INSERT INTO b2b_merchants (user_id, name, status) VALUES ($1, $2, 'active') \
                               ON CONFLICT (user_id) DO NOTHING RETURNING id";
const MERCHANT_STATUS: &str = "SELECT id, status FROM b2b_merchants WHERE user_id = $1";

const INSERT_QUOTE: &str = r#"
INSERT INTO b2b_quotes
    (order_id, provider, provider_quote_id, exact, input_amount, output_amount,
     best_output, captured_bps, app_fee_bps, deposit_address, refund_address,
     expires_at, raw)
VALUES ($1, 'near-intents', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
RETURNING id
"#;
const INSERT_QUOTED_ORDER: &str = r#"
INSERT INTO b2b_orders
    (merchant_id, env, kind, external_id, input_asset, input_network,
     output_asset, output_network, recipient, refund_address, status,
     metadata, idempotency_key)
VALUES ($1, $2, 'transfer', $3, $4, $5, $6, $7, $8, $9, 'quoted', $10, $11)
RETURNING id
"#;
const ORDER_BY_IDEMPOTENCY_KEY: &str = "SELECT id, metadata->>'request_hash', status FROM b2b_orders \
                                        WHERE merchant_id = $1 AND env = $2 AND idempotency_key = $3";
const LATEST_QUOTE_FOR_ORDER: &str =
    "SELECT id FROM b2b_quotes WHERE order_id = $1 ORDER BY created_at DESC LIMIT 1";
const QUOTE_OWNED_BY_MERCHANT: &str = r#"
SELECT o.id, o.status, o.env, o.accepted_quote_id, o.transfer_key, q.expires_at
FROM b2b_orders o
JOIN b2b_quotes q ON q.order_id = o.id
WHERE q.id = $1 AND o.merchant_id = $2
"#;
const ORDER_BY_TRANSFER_KEY: &str = "SELECT id, accepted_quote_id FROM b2b_orders \
                                    WHERE merchant_id = $1 AND env = $2 AND transfer_key = $3";
const ACCEPT_QUOTE: &str = r#"
UPDATE b2b_orders
SET status = 'awaiting_deposit', accepted_quote_id = $2, transfer_key = $3,
    external_id = COALESCE($4, external_id), updated_at = now()
WHERE id = $1 AND merchant_id = $5 AND status = 'quoted'
"#;
const CANCEL_TRANSFER: &str = "UPDATE b2b_orders SET status = 'cancelled', updated_at = now() \
                               WHERE id = $1 AND merchant_id = $2 AND status IN ('quoted','awaiting_deposit')";
const ORDER_STATUS: &str = "SELECT status FROM b2b_orders WHERE id = $1 AND merchant_id = $2";
const MERCHANT_ORDER_IDS: &str = "SELECT id FROM b2b_orders WHERE merchant_id = $1 \
                                  ORDER BY created_at DESC LIMIT $2";
const ORDER_FOR_MERCHANT: &str = r#"
SELECT id, merchant_id, env, kind, external_id, status, input_asset,
       input_network, output_asset, output_network, recipient, refund_address,
       accepted_quote_id, created_at, updated_at
FROM b2b_orders WHERE id = $1 AND merchant_id = $2
"#;
const ORDER_EVENTS: &str = "SELECT id, kind, body, created_at FROM b2b_events \
                            WHERE order_id = $1 ORDER BY id LIMIT 200";
const ORDER_BODY: &str = r#"
SELECT id, env, kind, external_id, status, input_asset, input_network,
       output_asset, output_network, recipient, refund_address, updated_at
FROM b2b_orders WHERE id = $1 AND merchant_id = $2
"#;
const ORDER_ACCEPTED_QUOTE: &str = "SELECT accepted_quote_id FROM b2b_orders WHERE id = $1";
const QUOTE_FOR_MERCHANT: &str = r#"
SELECT q.id, q.order_id, q.provider, q.exact, q.input_amount, q.output_amount,
       q.best_output, q.captured_bps, q.app_fee_bps, q.deposit_address,
       q.refund_address, q.expires_at, q.created_at, q.raw,
       o.status, o.input_asset, o.input_network, o.output_asset, o.output_network,
       o.recipient
FROM b2b_quotes q
JOIN b2b_orders o ON o.id = q.order_id
WHERE q.id = $1 AND o.merchant_id = $2
"#;
const INSERT_CHECKOUT_ORDER: &str = r#"
INSERT INTO b2b_orders
    (merchant_id, env, kind, external_id, input_asset, input_network,
     output_asset, output_network, recipient, refund_address, status,
     metadata, idempotency_key)
VALUES ($1, $2, 'checkout', $3, $4, $5, $6, $7, $8, $9, 'awaiting_deposit', $10, $11)
RETURNING id
"#;
const SET_ACCEPTED_QUOTE: &str = "UPDATE b2b_orders SET accepted_quote_id = $2 WHERE id = $1";
const INSERT_CHECKOUT_SESSION: &str =
    "INSERT INTO b2b_checkout_sessions (token, order_id, success_url, expires_at) \
                                       VALUES ($1, $2, $3, $4)";
const LATEST_CHECKOUT_SESSION: &str =
    "SELECT token, success_url FROM b2b_checkout_sessions WHERE order_id = $1 \
                                       ORDER BY created_at DESC LIMIT 1";
const CHECKOUT_SESSION: &str =
    "SELECT order_id, success_url FROM b2b_checkout_sessions WHERE token = $1";
const ORDER_MERCHANT: &str = "SELECT merchant_id FROM b2b_orders WHERE id = $1";
const ORDER_FOR_REQUOTE: &str = "SELECT status, input_asset, input_network, output_asset, output_network, \
                                 recipient, refund_address, external_id, accepted_quote_id FROM b2b_orders \
                                 WHERE id = $1 AND merchant_id = $2";
const LATEST_QUOTE_SIDES: &str = "SELECT exact, input_amount, output_amount FROM b2b_quotes \
                                  WHERE order_id = $1 ORDER BY created_at DESC LIMIT 1";
const REOPEN_CHECKOUT_ORDER: &str =
    "UPDATE b2b_orders SET status = 'awaiting_deposit', accepted_quote_id = $2, \
                                     updated_at = now() WHERE id = $1 AND status = $3";
const REFRESH_CHECKOUT_SESSION: &str =
    "UPDATE b2b_checkout_sessions SET expires_at = $2 WHERE token = $1";
const ANALYTICS_TOTALS: &str = r#"
SELECT COUNT(*)::BIGINT,
       COUNT(*) FILTER (WHERE status IN ('paid','overpaid'))::BIGINT,
       COALESCE(ROUND(AVG(captured_bps) FILTER (
           WHERE status IN ('paid','overpaid'))), 0)::INT
FROM b2b_orders o
LEFT JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1
"#;
const ANALYTICS_AVERAGE_TICKET: &str = r#"
SELECT input_asset, ROUND(COALESCE(AVG(input_amount::NUMERIC), 0), 8)::TEXT
FROM b2b_orders o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1
GROUP BY input_asset
ORDER BY input_asset
"#;
const ANALYTICS_VOLUME: &str = r#"
SELECT input_asset, COALESCE(SUM(input_amount::NUMERIC), 0)::TEXT
FROM b2b_orders o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1 AND o.status IN ('paid','overpaid')
GROUP BY input_asset
ORDER BY input_asset
"#;
const ANALYTICS_CAPTURED: &str =
    "SELECT asset, COALESCE(SUM(amount::NUMERIC), 0)::TEXT FROM b2b_fee_ledger \
                                  WHERE merchant_id = $1 GROUP BY asset ORDER BY asset";
const ANALYTICS_FAILURE_REASONS: &str = r#"
SELECT status, COUNT(*)::BIGINT FROM b2b_orders
WHERE merchant_id = $1
  AND status IN ('failed','expired','underpaid','cancelled')
GROUP BY status ORDER BY status
"#;
const ACCEPTED_ORDERS: &str = r#"
WITH selected AS (
    SELECT id FROM b2b_orders
    WHERE accepted_quote_id IS NOT NULL
      AND status NOT IN ('paid','overpaid','refunded')
    ORDER BY last_polled_at ASC NULLS FIRST, created_at ASC, id ASC
    LIMIT 50
    FOR UPDATE SKIP LOCKED
), polled AS (
    UPDATE b2b_orders o SET last_polled_at = now()
    FROM selected s WHERE o.id = s.id RETURNING o.*
)
SELECT o.id, o.merchant_id, o.status, o.input_network, o.output_network,
       q.id, q.input_amount, q.output_amount, q.captured_bps, q.deposit_address,
       q.expires_at, q.raw
FROM polled o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
"#;
const EXPIRE_UNACCEPTED: &str = r#"
UPDATE b2b_orders o
SET status = 'expired', updated_at = now()
FROM b2b_quotes q
WHERE o.accepted_quote_id IS NULL AND o.status = 'quoted'
  AND q.order_id = o.id AND q.expires_at < now()
  AND q.created_at = (SELECT max(created_at) FROM b2b_quotes WHERE order_id = o.id)
"#;

const WORKER_LOCK: &str = "SELECT pg_try_advisory_lock($1)";
const WORKER_UNLOCK: &str = "SELECT pg_advisory_unlock($1)";
const APPLY_ORDER_STATUS: &str = "UPDATE b2b_orders SET status = $3, updated_at = now() \
                                  WHERE id = $1 AND status = $2";
const INSERT_FEE: &str = r#"
INSERT INTO b2b_fee_ledger
    (merchant_id, order_id, asset, amount, captured_bps, recipient)
SELECT $1, $2, input_asset, $3, $4, $5 FROM b2b_orders WHERE id = $2
"#;
const INSERT_ATTEMPT: &str = r#"
INSERT INTO b2b_attempts
    (order_id, quote_id, kind, chain, tx_hash, amount, status)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (chain, tx_hash, kind) DO NOTHING
"#;

const INSERT_WEBHOOK_ENDPOINT: &str = r#"
INSERT INTO b2b_webhook_endpoints (merchant_id, url, secret_ciphertext, events)
VALUES ($1, $2, $3, $4)
RETURNING id, active
"#;
const CLAIM_DELIVERIES: &str = r#"
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
"#;
const WEBHOOK_ENDPOINT: &str =
    "SELECT url, secret_ciphertext, active FROM b2b_webhook_endpoints WHERE id = $1";
const MARK_DELIVERY_DEAD: &str = "UPDATE b2b_webhook_deliveries SET status = 'dead' WHERE id = $1";
const MARK_DELIVERY_SKIPPED: &str =
    "UPDATE b2b_webhook_deliveries SET status = 'skipped' WHERE id = $1";
const MARK_DELIVERY_DELIVERED: &str = "UPDATE b2b_webhook_deliveries SET status = 'delivered', \
                                       attempts = attempts + 1, response_code = $2, response_body = $3 \
                                       WHERE id = $1";
const GIVE_UP_DELIVERY: &str = "UPDATE b2b_webhook_deliveries SET status = 'dead', attempts = $2, \
                                response_code = $3, response_body = $4 WHERE id = $1";
const RESCHEDULE_DELIVERY: &str = "UPDATE b2b_webhook_deliveries SET status = 'pending', attempts = $2, \
                                   response_code = $3, response_body = $4, \
                                   next_attempt_at = now() + make_interval(secs => $5) WHERE id = $1";
const INSERT_EVENT: &str =
    "INSERT INTO b2b_events (order_id, kind, body) VALUES ($1, $2, $3) RETURNING id";
const FAN_OUT_EVENT: &str = r#"
INSERT INTO b2b_webhook_deliveries (endpoint_id, event_id, event_kind, payload)
SELECT e.id, $1, $2, $3
FROM b2b_webhook_endpoints e
JOIN b2b_orders o ON o.merchant_id = e.merchant_id
WHERE o.id = $4 AND e.active AND $2 = ANY(e.events)
"#;

const API_KEY_BY_PREFIX: &str = "SELECT k.id, k.project_id, p.merchant_id, k.env, k.scopes, \
                                 k.daily_request_limit, k.prefix, k.secret_hash, k.status \
                                 FROM b2b_api_keys k \
                                 JOIN b2b_projects p ON p.id = k.project_id \
                                 WHERE k.prefix = $1";
const REQUESTS_TODAY: &str = "SELECT COALESCE(SUM(requests), 0)::bigint \
                              FROM b2b_api_key_usage \
                              WHERE key_id = $1 AND bucket_start >= $2 AND bucket_start < $3";
const UPSERT_USAGE: &str = "INSERT INTO b2b_api_key_usage \
                            (key_id, bucket_start, requests, errors_4xx, errors_5xx, latency_sum_ms) \
                            VALUES ($1, $2, 1, $3, $4, $5) \
                            ON CONFLICT (key_id, bucket_start) DO UPDATE SET \
                            requests = b2b_api_key_usage.requests + 1, \
                            errors_4xx = b2b_api_key_usage.errors_4xx + EXCLUDED.errors_4xx, \
                            errors_5xx = b2b_api_key_usage.errors_5xx + EXCLUDED.errors_5xx, \
                            latency_sum_ms = b2b_api_key_usage.latency_sum_ms + EXCLUDED.latency_sum_ms";
const TOUCH_API_KEY: &str = "UPDATE b2b_api_keys SET last_used_at = $2 WHERE id = $1";

/// `row.get(0)` of a one-column query.
fn first<T: for<'a> tokio_postgres::types::FromSql<'a>>(row: tokio_postgres::Row) -> T {
    row.get(0)
}

pub(crate) async fn merchant_by_user<C>(client: &C, user_id: Uuid) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(MERCHANT_BY_USER, &[&user_id])
        .await?
        .map(first))
}

pub(crate) async fn insert_merchant<C>(
    client: &C,
    user_id: Uuid,
    name: &str,
) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(INSERT_MERCHANT, &[&user_id, &name])
        .await?
        .map(first))
}

/// `(id, status)` of the caller's merchant row.
pub(crate) async fn merchant_status<C>(client: &C, user_id: Uuid) -> Result<(Uuid, String)>
where
    C: Conn + Sync + ?Sized,
{
    let row = client.query_one(MERCHANT_STATUS, &[&user_id]).await?;
    Ok((row.get(0), row.get(1)))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_quote<C>(
    client: &C,
    order_id: Uuid,
    provider_quote_id: &str,
    exact: &str,
    input_amount: &str,
    output_amount: &str,
    best_output: &str,
    captured_bps: i32,
    app_fee_bps: i32,
    deposit_address: &str,
    refund_address: &str,
    expires_at: DateTime<Utc>,
    raw: &Value,
) -> Result<Uuid>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(
            INSERT_QUOTE,
            &[
                &order_id,
                &provider_quote_id,
                &exact,
                &input_amount,
                &output_amount,
                &best_output,
                &captured_bps,
                &app_fee_bps,
                &deposit_address,
                &refund_address,
                &expires_at,
                &raw,
            ],
        )
        .await?;
    Ok(row.get(0))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_quoted_order<C>(
    client: &C,
    merchant_id: Uuid,
    env: &str,
    external_id: Option<&str>,
    input_asset: &str,
    input_network: Option<&str>,
    output_asset: &str,
    output_network: Option<&str>,
    recipient: &str,
    refund_address: &str,
    metadata: &Value,
    idempotency_key: Option<&str>,
) -> Result<Uuid>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(
            INSERT_QUOTED_ORDER,
            &[
                &merchant_id,
                &env,
                &external_id,
                &input_asset,
                &input_network,
                &output_asset,
                &output_network,
                &recipient,
                &refund_address,
                &metadata,
                &idempotency_key,
            ],
        )
        .await?;
    Ok(row.get(0))
}

/// `(order_id, metadata request_hash, status)` of an idempotent replay.
pub(crate) async fn order_by_idempotency_key<C>(
    client: &C,
    merchant_id: Uuid,
    env: &str,
    key: &str,
) -> Result<Option<(Uuid, Option<String>, String)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(ORDER_BY_IDEMPOTENCY_KEY, &[&merchant_id, &env, &key])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn latest_quote_for_order<C>(client: &C, order_id: Uuid) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(LATEST_QUOTE_FOR_ORDER, &[&order_id])
        .await?
        .map(first))
}

/// `(order_id, status, env, accepted_quote_id, transfer_key, expires_at)`.
pub(crate) async fn quote_owned_by_merchant<C>(
    client: &C,
    quote_id: Uuid,
    merchant_id: Uuid,
) -> Result<
    Option<(
        Uuid,
        String,
        String,
        Option<Uuid>,
        Option<String>,
        DateTime<Utc>,
    )>,
>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(QUOTE_OWNED_BY_MERCHANT, &[&quote_id, &merchant_id])
        .await?;
    Ok(row.map(|row| {
        (
            row.get(0),
            row.get(1),
            row.get(2),
            row.get(3),
            row.get(4),
            row.get(5),
        )
    }))
}

/// `(order_id, accepted_quote_id)` of an idempotent transfer replay.
pub(crate) async fn order_by_transfer_key<C>(
    client: &C,
    merchant_id: Uuid,
    env: &str,
    key: &str,
) -> Result<Option<(Uuid, Option<Uuid>)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(ORDER_BY_TRANSFER_KEY, &[&merchant_id, &env, &key])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

pub(crate) async fn accept_quote<C>(
    client: &C,
    order_id: Uuid,
    quote_id: Uuid,
    key: &str,
    external_id: Option<&str>,
    merchant_id: Uuid,
) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(
            ACCEPT_QUOTE,
            &[&order_id, &quote_id, &key, &external_id, &merchant_id],
        )
        .await?)
}

pub(crate) async fn cancel_transfer<C>(client: &C, order_id: Uuid, merchant_id: Uuid) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(CANCEL_TRANSFER, &[&order_id, &merchant_id])
        .await?)
}

pub(crate) async fn order_status<C>(
    client: &C,
    order_id: Uuid,
    merchant_id: Uuid,
) -> Result<Option<String>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_STATUS, &[&order_id, &merchant_id])
        .await?
        .map(first))
}

pub(crate) async fn merchant_order_ids<C>(
    client: &C,
    merchant_id: Uuid,
    limit: i64,
) -> Result<Vec<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(MERCHANT_ORDER_IDS, &[&merchant_id, &limit])
        .await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub(crate) async fn order_for_merchant<C>(
    client: &C,
    order_id: Uuid,
    merchant_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_FOR_MERCHANT, &[&order_id, &merchant_id])
        .await?)
}

/// `(id, kind, body, created_at)` per order event.
pub(crate) async fn order_events<C>(
    client: &C,
    order_id: Uuid,
) -> Result<Vec<(i64, String, Value, DateTime<Utc>)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(ORDER_EVENTS, &[&order_id]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect())
}

pub(crate) async fn order_body<C>(
    client: &C,
    order_id: Uuid,
    merchant_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_BODY, &[&order_id, &merchant_id])
        .await?)
}

pub(crate) async fn order_accepted_quote<C>(client: &C, order_id: Uuid) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_ACCEPTED_QUOTE, &[&order_id])
        .await?
        .and_then(|row| row.get(0)))
}

pub(crate) async fn quote_for_merchant<C>(
    client: &C,
    quote_id: Uuid,
    merchant_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(QUOTE_FOR_MERCHANT, &[&quote_id, &merchant_id])
        .await?)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_checkout_order<C>(
    client: &C,
    merchant_id: Uuid,
    env: &str,
    external_id: Option<&str>,
    input_asset: &str,
    input_network: Option<&str>,
    output_asset: &str,
    output_network: Option<&str>,
    recipient: &str,
    refund_address: &str,
    metadata: &Value,
    idempotency_key: Option<&str>,
) -> Result<Uuid>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(
            INSERT_CHECKOUT_ORDER,
            &[
                &merchant_id,
                &env,
                &external_id,
                &input_asset,
                &input_network,
                &output_asset,
                &output_network,
                &recipient,
                &refund_address,
                &metadata,
                &idempotency_key,
            ],
        )
        .await?;
    Ok(row.get(0))
}

pub(crate) async fn set_accepted_quote<C>(client: &C, order_id: Uuid, quote_id: Uuid) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(SET_ACCEPTED_QUOTE, &[&order_id, &quote_id])
        .await?;
    Ok(())
}

pub(crate) async fn insert_checkout_session<C>(
    client: &C,
    token: &str,
    order_id: Uuid,
    success_url: Option<&String>,
    expires_at: DateTime<Utc>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_CHECKOUT_SESSION,
            &[&token, &order_id, &success_url, &expires_at],
        )
        .await?;
    Ok(())
}

/// `(token, success_url)` of the newest page session for the order.
pub(crate) async fn latest_checkout_session<C>(
    client: &C,
    order_id: Uuid,
) -> Result<Option<(String, Option<String>)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(LATEST_CHECKOUT_SESSION, &[&order_id])
        .await?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

/// `(order_id, success_url)` for a page token.
pub(crate) async fn checkout_session<C>(
    client: &C,
    token: &str,
) -> Result<Option<(Uuid, Option<String>)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client.query_opt(CHECKOUT_SESSION, &[&token]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

pub(crate) async fn order_merchant<C>(client: &C, order_id: Uuid) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_MERCHANT, &[&order_id])
        .await?
        .map(first))
}

pub(crate) async fn order_for_requote<C>(
    client: &C,
    order_id: Uuid,
    merchant_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ORDER_FOR_REQUOTE, &[&order_id, &merchant_id])
        .await?)
}

/// `(exact, input_amount, output_amount)` of the newest quote.
pub(crate) async fn latest_quote_sides<C>(
    client: &C,
    order_id: Uuid,
) -> Result<(String, String, String)>
where
    C: Conn + Sync + ?Sized,
{
    let row = client.query_one(LATEST_QUOTE_SIDES, &[&order_id]).await?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

pub(crate) async fn reopen_checkout_order<C>(
    client: &C,
    order_id: Uuid,
    quote_id: Uuid,
    status: &str,
) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(REOPEN_CHECKOUT_ORDER, &[&order_id, &quote_id, &status])
        .await?)
}

pub(crate) async fn refresh_checkout_session<C>(
    client: &C,
    token: &str,
    expires_at: DateTime<Utc>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(REFRESH_CHECKOUT_SESSION, &[&token, &expires_at])
        .await?;
    Ok(())
}

/// `(orders, paid_orders, margin_bps)`.
pub(crate) async fn analytics_totals<C>(client: &C, merchant_id: Uuid) -> Result<(i64, i64, i32)>
where
    C: Conn + Sync + ?Sized,
{
    let row = client.query_one(ANALYTICS_TOTALS, &[&merchant_id]).await?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

/// `(asset, amount)` per input asset.
pub(crate) async fn analytics_average_ticket<C>(
    client: &C,
    merchant_id: Uuid,
) -> Result<Vec<(String, String)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(ANALYTICS_AVERAGE_TICKET, &[&merchant_id])
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// `(asset, amount)` per input asset.
pub(crate) async fn analytics_volume<C>(
    client: &C,
    merchant_id: Uuid,
) -> Result<Vec<(String, String)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(ANALYTICS_VOLUME, &[&merchant_id]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub(crate) async fn analytics_captured<C>(
    client: &C,
    merchant_id: Uuid,
) -> Result<Vec<(String, String)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(ANALYTICS_CAPTURED, &[&merchant_id]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// `(reason, orders)` per failed status.
pub(crate) async fn analytics_failure_reasons<C>(
    client: &C,
    merchant_id: Uuid,
) -> Result<Vec<(String, i64)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(ANALYTICS_FAILURE_REASONS, &[&merchant_id])
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// `(order_id, merchant_id, status, input_network, output_network, quote_id,
/// input_amount, output_amount, captured_bps, deposit_address, expires_at,
/// deposit_memo)` per order ready to poll.
#[allow(clippy::type_complexity)]
pub(crate) async fn accepted_orders<C>(
    client: &C,
) -> Result<
    Vec<(
        Uuid,
        Uuid,
        String,
        String,
        String,
        Uuid,
        String,
        String,
        i32,
        String,
        DateTime<Utc>,
        Option<String>,
    )>,
>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(ACCEPTED_ORDERS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            let raw: Value = row.get(11);
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
                row.get(6),
                row.get(7),
                row.get(8),
                row.get(9),
                row.get(10),
                crate::b2b::deposit_memo(&raw),
            )
        })
        .collect())
}

pub(crate) async fn expire_unaccepted<C>(client: &C) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(EXPIRE_UNACCEPTED, &[]).await?;
    Ok(())
}

/// Single-flight worker lock; `false` when another replica holds it.
pub(crate) async fn try_worker_lock<C>(client: &C, key: i64) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(WORKER_LOCK, &[&key])
        .await
        .map(|row| row.get(0))
        .unwrap_or(false))
}

pub(crate) async fn unlock_worker<C>(client: &C, key: i64) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(WORKER_UNLOCK, &[&key]).await?;
    Ok(())
}

pub(crate) async fn apply_order_status<C>(
    client: &C,
    order_id: Uuid,
    current: &str,
    new_status: &str,
) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(APPLY_ORDER_STATUS, &[&order_id, &current, &new_status])
        .await?)
}

pub(crate) async fn insert_fee<C>(
    client: &C,
    merchant_id: Uuid,
    order_id: Uuid,
    amount: &str,
    captured_bps: i32,
    recipient: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_FEE,
            &[&merchant_id, &order_id, &amount, &captured_bps, &recipient],
        )
        .await?;
    Ok(())
}

pub(crate) async fn insert_attempt<C>(
    client: &C,
    order_id: Uuid,
    quote_id: Uuid,
    kind: &str,
    chain: &str,
    tx_hash: &str,
    amount: &str,
    status: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_ATTEMPT,
            &[
                &order_id, &quote_id, &kind, &chain, &tx_hash, &amount, &status,
            ],
        )
        .await?;
    Ok(())
}

/// `(id, active)` of the created endpoint.
pub(crate) async fn insert_webhook_endpoint<C>(
    client: &C,
    merchant_id: Uuid,
    url: &str,
    secret_ciphertext: &[u8],
    events: &[String],
) -> Result<(Uuid, bool)>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(
            INSERT_WEBHOOK_ENDPOINT,
            &[&merchant_id, &url, &secret_ciphertext, &events],
        )
        .await?;
    Ok((row.get(0), row.get(1)))
}

/// `(delivery_id, endpoint_id, event_id, payload, attempts)` per claimed
/// delivery.
pub(crate) async fn claim_deliveries<C>(client: &C) -> Result<Vec<(Uuid, Uuid, i64, Value, i32)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(CLAIM_DELIVERIES, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect())
}

/// `(url, secret_ciphertext, active)` of a delivery endpoint.
pub(crate) async fn webhook_endpoint<C>(
    client: &C,
    endpoint_id: Uuid,
) -> Result<Option<(String, Vec<u8>, bool)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client.query_opt(WEBHOOK_ENDPOINT, &[&endpoint_id]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn mark_delivery_dead<C>(client: &C, delivery_id: Uuid) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(MARK_DELIVERY_DEAD, &[&delivery_id]).await?;
    Ok(())
}

pub(crate) async fn mark_delivery_skipped<C>(client: &C, delivery_id: Uuid) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(MARK_DELIVERY_SKIPPED, &[&delivery_id])
        .await?;
    Ok(())
}

pub(crate) async fn mark_delivery_delivered<C>(
    client: &C,
    delivery_id: Uuid,
    response_code: i32,
    response_body: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            MARK_DELIVERY_DELIVERED,
            &[&delivery_id, &response_code, &response_body],
        )
        .await?;
    Ok(())
}

pub(crate) async fn give_up_delivery<C>(
    client: &C,
    delivery_id: Uuid,
    attempts: i32,
    response_code: Option<i32>,
    response_body: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            GIVE_UP_DELIVERY,
            &[&delivery_id, &attempts, &response_code, &response_body],
        )
        .await?;
    Ok(())
}

pub(crate) async fn reschedule_delivery<C>(
    client: &C,
    delivery_id: Uuid,
    attempts: i32,
    response_code: Option<i32>,
    response_body: &str,
    delay_secs: f64,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            RESCHEDULE_DELIVERY,
            &[
                &delivery_id,
                &attempts,
                &response_code,
                &response_body,
                &delay_secs,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn insert_event<C>(
    client: &C,
    order_id: Uuid,
    kind: &str,
    body: &Value,
) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(INSERT_EVENT, &[&order_id, &kind, &body])
        .await?;
    Ok(row.get(0))
}

pub(crate) async fn fan_out_event<C>(
    client: &C,
    event_id: i64,
    kind: &str,
    payload: &Value,
    order_id: Uuid,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(FAN_OUT_EVENT, &[&event_id, &kind, &payload, &order_id])
        .await?;
    Ok(())
}

pub(crate) async fn api_key_by_prefix<C>(
    client: &C,
    prefix: &str,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client.query_opt(API_KEY_BY_PREFIX, &[&prefix]).await?)
}

pub(crate) async fn requests_today<C>(
    client: &C,
    key_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(REQUESTS_TODAY, &[&key_id, &start, &end])
        .await?
        .get(0))
}

pub(crate) async fn upsert_usage<C>(
    client: &C,
    key_id: Uuid,
    bucket_start: DateTime<Utc>,
    errors_4xx: i64,
    errors_5xx: i64,
    latency_sum_ms: i64,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            UPSERT_USAGE,
            &[
                &key_id,
                &bucket_start,
                &errors_4xx,
                &errors_5xx,
                &latency_sum_ms,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn touch_api_key<C>(client: &C, key_id: Uuid, now: DateTime<Utc>) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(TOUCH_API_KEY, &[&key_id, &now]).await?;
    Ok(())
}

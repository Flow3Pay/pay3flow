use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

const QUOTE_RESPONSE: &str = "WITH response AS (SELECT r.id,min(extract(epoch FROM i.processed_at-r.created_at))::double precision AS seconds FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE r.created_at>now()-interval '24 hours' AND i.processed_at<=(t.terms->>'quote_by')::timestamptz GROUP BY r.id) SELECT percentile_cont(0.95) WITHIN GROUP(ORDER BY seconds),count(*) FROM response";
const OLDEST_WAITING_RFQ: &str = "SELECT min(r.created_at) FROM otc_rfq r WHERE r.state='waiting' AND NOT EXISTS(SELECT 1 FROM otc_trades t JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE t.rfq_id=r.id AND i.processed_at<=(t.terms->>'quote_by')::timestamptz)";
const OLDEST_PENDING_PAYMENT: &str = "SELECT min(created_at) FROM otc_attempts WHERE kind='payment' AND state IN ('unknown','submitted','confirming')";
const OLDEST_UNANSWERED_BOOKING: &str =
    "SELECT min(updated_at) FROM otc_trades WHERE state='booking_pending'";
const OUTSTANDING_OBLIGATIONS: &str = "WITH open AS (SELECT t.*,e.kind,e.evidence,EXISTS(SELECT 1 FROM otc_attempts a WHERE a.trade_id=t.id AND a.kind='refund' AND a.state!='failed') AS refund_selected FROM otc_trades t JOIN otc_evidence e ON e.trade_id=t.id AND e.kind IN ('payment','review_payment') WHERE t.state NOT IN ('completed','refunded')) SELECT CASE WHEN kind='review_payment' OR refund_selected THEN evidence->'leg'->>'chain' WHEN terms->>'direction'='buy' THEN 'everscale' ELSE 'ethereum' END AS chain,sum(CASE WHEN kind='review_payment' THEN (evidence->'leg'->>'amount')::numeric ELSE (terms->>'output_units')::numeric END)::text,count(*) FROM open GROUP BY chain";
const AGREED_NOTIONAL: &str = "SELECT COALESCE(sum((terms->'fee_policy'->>'basis_units')::numeric),0)::text FROM otc_trades t WHERE state NOT IN ('completed','refunded') AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind IN ('payment','review_payment'))";
const INCIDENT_TIMINGS: &str = "SELECT incident_key,severity,extract(epoch FROM acknowledged_at-detected_at)::bigint,extract(epoch FROM service_restored_at-detected_at)::bigint,extract(epoch FROM financially_resolved_at-detected_at)::bigint,CASE WHEN closed_at IS NULL THEN extract(epoch FROM now()-detected_at)::bigint END FROM otc_incidents";
const PAYMENT_TO_PAYOUT: &str = "SELECT p.trade_id,extract(epoch FROM o.included_at-p.included_at)::bigint FROM otc_evidence p JOIN otc_evidence o ON o.trade_id=p.trade_id AND o.kind='payout' AND o.valid WHERE p.kind='payment' AND p.valid AND o.included_at>now()-interval '24 hours'";

/// Returns `(p95 seconds, observations)` for quote responses in the last 24 hours.
pub(crate) async fn quote_response(
    client: &(impl GenericClient + Sync),
) -> Result<(Option<f64>, i64)> {
    let row = client.query_one(QUOTE_RESPONSE, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

pub(crate) async fn oldest_waiting_rfq(
    client: &(impl GenericClient + Sync),
) -> Result<Option<DateTime<Utc>>> {
    Ok(client.query_one(OLDEST_WAITING_RFQ, &[]).await?.get(0))
}

pub(crate) async fn oldest_pending_payment(
    client: &(impl GenericClient + Sync),
) -> Result<Option<DateTime<Utc>>> {
    Ok(client.query_one(OLDEST_PENDING_PAYMENT, &[]).await?.get(0))
}

pub(crate) async fn oldest_unanswered_booking(
    client: &(impl GenericClient + Sync),
) -> Result<Option<DateTime<Utc>>> {
    Ok(client
        .query_one(OLDEST_UNANSWERED_BOOKING, &[])
        .await?
        .get(0))
}

/// Returns `(chain, units, cases)` of outstanding obligations per asset.
pub(crate) async fn outstanding_obligations(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, String, i64)>> {
    let rows = client.query(OUTSTANDING_OBLIGATIONS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

pub(crate) async fn agreed_notional(client: &(impl GenericClient + Sync)) -> Result<String> {
    Ok(client.query_one(AGREED_NOTIONAL, &[]).await?.get(0))
}

/// Returns `(key, severity, acknowledgement, restoration, financial resolution,
/// open age)` in seconds per incident.
#[allow(clippy::type_complexity)]
pub(crate) async fn incident_timings(
    client: &(impl GenericClient + Sync),
) -> Result<
    Vec<(
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    )>,
> {
    let rows = client.query(INCIDENT_TIMINGS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get(0),
                row.get(1),
                row.get(2),
                row.get(3),
                row.get(4),
                row.get(5),
            )
        })
        .collect())
}

/// Returns `(trade_id, payment_to_payout seconds)` per payout in the last 24 hours.
pub(crate) async fn payment_to_payout(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, i64)>> {
    let rows = client.query(PAYMENT_TO_PAYOUT, &[]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

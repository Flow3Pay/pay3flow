use super::{error::Result, Service};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

impl Service {
    pub(crate) async fn reporting(&self) -> Result<Value> {
        let client = self.pool.get().await?;
        let quote = client.query_one(
            "WITH response AS (SELECT r.id,min(extract(epoch FROM i.processed_at-r.created_at))::double precision AS seconds FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE r.created_at>now()-interval '24 hours' AND i.processed_at<=(t.terms->>'quote_by')::timestamptz GROUP BY r.id) SELECT percentile_cont(0.95) WITHIN GROUP(ORDER BY seconds),count(*) FROM response", &[]).await?;
        let waiting: Option<DateTime<Utc>> = client.query_one(
            "SELECT min(r.created_at) FROM otc_rfq r WHERE r.state='waiting' AND NOT EXISTS(SELECT 1 FROM otc_trades t JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE t.rfq_id=r.id AND i.processed_at<=(t.terms->>'quote_by')::timestamptz)", &[]).await?.get(0);
        let pending: Option<DateTime<Utc>> = client.query_one(
            "SELECT min(created_at) FROM otc_attempts WHERE kind='payment' AND state IN ('unknown','submitted','confirming')", &[]).await?.get(0);
        let booking: Option<DateTime<Utc>> = client
            .query_one(
                "SELECT min(updated_at) FROM otc_trades WHERE state='booking_pending'",
                &[],
            )
            .await?
            .get(0);
        let liabilities: Vec<Value> = client.query(
            "WITH open AS (SELECT t.*,e.kind,e.evidence,EXISTS(SELECT 1 FROM otc_attempts a WHERE a.trade_id=t.id AND a.kind='refund' AND a.state!='failed') AS refund_selected FROM otc_trades t JOIN otc_evidence e ON e.trade_id=t.id AND e.kind IN ('payment','review_payment') WHERE t.state NOT IN ('completed','refunded')) SELECT CASE WHEN kind='review_payment' OR refund_selected THEN evidence->'leg'->>'chain' WHEN terms->>'direction'='buy' THEN 'everscale' ELSE 'ethereum' END AS chain,sum(CASE WHEN kind='review_payment' THEN (evidence->'leg'->>'amount')::numeric ELSE (terms->>'output_units')::numeric END)::text,count(*) FROM open GROUP BY chain", &[]).await?.iter().map(|r|json!({"chain":r.get::<_,String>(0),"units":r.get::<_,String>(1),"cases":r.get::<_,i64>(2)})).collect();
        let notional: String = client.query_one(
            "SELECT COALESCE(sum((terms->'fee_policy'->>'basis_units')::numeric),0)::text FROM otc_trades t WHERE state NOT IN ('completed','refunded') AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind IN ('payment','review_payment'))", &[]).await?.get(0);
        let durations: Vec<Value> = client.query(
            "SELECT incident_key,severity,extract(epoch FROM acknowledged_at-detected_at)::bigint,extract(epoch FROM service_restored_at-detected_at)::bigint,extract(epoch FROM financially_resolved_at-detected_at)::bigint,CASE WHEN closed_at IS NULL THEN extract(epoch FROM now()-detected_at)::bigint END FROM otc_incidents", &[]).await?.iter().map(|r|json!({"key":r.get::<_,String>(0),"severity":r.get::<_,String>(1),"acknowledgement_seconds":r.get::<_,Option<i64>>(2),"restoration_seconds":r.get::<_,Option<i64>>(3),"financial_resolution_seconds":r.get::<_,Option<i64>>(4),"open_age_seconds":r.get::<_,Option<i64>>(5)})).collect();
        let payout_times: Vec<Value> = client.query(
            "SELECT p.trade_id,extract(epoch FROM o.included_at-p.included_at)::bigint FROM otc_evidence p JOIN otc_evidence o ON o.trade_id=p.trade_id AND o.kind='payout' AND o.valid WHERE p.kind='payment' AND p.valid AND o.included_at>now()-interval '24 hours'", &[]).await?.iter().map(|r|json!({"trade_id":r.get::<_,uuid::Uuid>(0),"payment_to_payout_seconds":r.get::<_,i64>(1)})).collect();
        Ok(json!({
            "quote_response_p95_seconds":quote.get::<_,Option<f64>>(0),
            "quote_response_observations":quote.get::<_,i64>(1),
            "oldest_waiting_rfq_seconds":age(waiting),
            "oldest_pending_payment_seconds":age(pending),
            "oldest_unanswered_booking_seconds":age(booking),
            "outstanding_obligations_by_asset":liabilities,
            "agreed_usdt_leg_notional_units":notional,
            "incident_timings":durations,
            "payment_to_payout":payout_times
        }))
    }
}
fn age(at: Option<DateTime<Utc>>) -> Option<i64> {
    at.map(|at| (Utc::now() - at).num_seconds().max(0))
}

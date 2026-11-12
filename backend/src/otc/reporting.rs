use super::{error::Result, repo, Service};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

impl Service {
    pub(crate) async fn reporting(&self) -> Result<Value> {
        let client = self.pool.get().await?;
        let (quote_p95, quote_observations) = repo::reporting::quote_response(&client).await?;
        let waiting = repo::reporting::oldest_waiting_rfq(&client).await?;
        let pending = repo::reporting::oldest_pending_payment(&client).await?;
        let booking = repo::reporting::oldest_unanswered_booking(&client).await?;
        let liabilities: Vec<Value> = repo::reporting::outstanding_obligations(&client)
            .await?
            .into_iter()
            .map(|(chain, units, cases)| json!({"chain":chain,"units":units,"cases":cases}))
            .collect();
        let notional = repo::reporting::agreed_notional(&client).await?;
        let durations: Vec<Value> = repo::reporting::incident_timings(&client).await?.into_iter().map(|(key,severity,acknowledgement_seconds,restoration_seconds,financial_resolution_seconds,open_age_seconds)|json!({"key":key,"severity":severity,"acknowledgement_seconds":acknowledgement_seconds,"restoration_seconds":restoration_seconds,"financial_resolution_seconds":financial_resolution_seconds,"open_age_seconds":open_age_seconds})).collect();
        let payout_times: Vec<Value> = repo::reporting::payment_to_payout(&client).await?.into_iter().map(|(trade_id,payment_to_payout_seconds)|json!({"trade_id":trade_id,"payment_to_payout_seconds":payment_to_payout_seconds})).collect();
        Ok(json!({
            "quote_response_p95_seconds":quote_p95,
            "quote_response_observations":quote_observations,
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

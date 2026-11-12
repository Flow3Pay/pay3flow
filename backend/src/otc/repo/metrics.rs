use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;

const METRIC_AGGREGATE_PREFIX: &str = "SELECT {aggregate} FROM otc_events e LEFT JOIN otc_trades t ON t.id=e.trade_id LEFT JOIN otc_rfq r ON r.id=COALESCE(e.rfq_id,t.rfq_id) WHERE e.kind=$1 AND r.direction=$2";
const TRADE_STATES: &str = "SELECT state,count(*) FROM otc_trades GROUP BY state";
const OBSERVERS: &str = "SELECT chain,last_scan,paused,failures,lag FROM otc_observers";
const QUOTE_RESPONSE_BUCKET: &str = "SELECT count(*) FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE extract(epoch FROM i.processed_at-r.created_at)<=$1::int";
const QUOTE_RESPONSE_TOTALS: &str = "SELECT count(*),COALESCE(sum(extract(epoch FROM i.processed_at-r.created_at)),0)::double precision FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id'";
const OPEN_INCIDENTS: &str = "SELECT severity,count(*),min(detected_at) FROM otc_incidents WHERE closed_at IS NULL GROUP BY severity";

/// One event counter labelled by event kind and RFQ direction.
pub(crate) async fn event_count(
    client: &(impl GenericClient + Sync),
    aggregate: &str,
    kind: &str,
    direction: &str,
) -> Result<i64> {
    let query = METRIC_AGGREGATE_PREFIX.replace("{aggregate}", aggregate);
    Ok(client.query_one(&query, &[&kind, &direction]).await?.get(0))
}

/// Returns `(state, count)` per trade state.
pub(crate) async fn trade_states(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, i64)>> {
    let rows = client.query(TRADE_STATES, &[]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// Returns `(chain, last_scan, paused, failures, lag)` per observer.
#[allow(clippy::type_complexity)]
pub(crate) async fn observers(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, Option<DateTime<Utc>>, bool, i32, Option<i64>)>> {
    let rows = client.query(OBSERVERS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect())
}

pub(crate) async fn quote_response_bucket(
    client: &(impl GenericClient + Sync),
    bucket: i32,
) -> Result<i64> {
    Ok(client
        .query_one(QUOTE_RESPONSE_BUCKET, &[&bucket])
        .await?
        .get(0))
}

/// Returns `(observations, total seconds)` of RFQ quote responses.
pub(crate) async fn quote_response_totals(
    client: &(impl GenericClient + Sync),
) -> Result<(i64, f64)> {
    let row = client.query_one(QUOTE_RESPONSE_TOTALS, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

/// Returns `(severity, count, oldest detected_at)` per open incident severity.
pub(crate) async fn open_incidents(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, i64, DateTime<Utc>)>> {
    let rows = client.query(OPEN_INCIDENTS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

const CONTROL_DEMO: &str = "SELECT demo FROM otc_controls WHERE singleton";
const CONTROL_PAUSED: &str = "SELECT paused FROM otc_controls WHERE singleton";
const UNHEALTHY_OBSERVERS: &str = "SELECT count(*) FROM otc_observers WHERE paused OR last_scan IS NULL OR last_scan < now()-interval '60 seconds'";

pub(crate) async fn control_demo(client: &(impl GenericClient + Sync)) -> Result<Option<bool>> {
    Ok(client.query_one(CONTROL_DEMO, &[]).await?.get(0))
}

pub(crate) async fn control_paused(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client.query_one(CONTROL_PAUSED, &[]).await?.get(0))
}

pub(crate) async fn unhealthy_observers(client: &(impl GenericClient + Sync)) -> Result<i64> {
    Ok(client.query_one(UNHEALTHY_OBSERVERS, &[]).await?.get(0))
}

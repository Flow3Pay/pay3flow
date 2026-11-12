use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const UPSERT_INCIDENT: &str = "INSERT INTO otc_incidents(incident_key,severity,owner,backup,trade_id,runbook,evidence,next_update_at) VALUES($1,$2,$3,$4,$5,$6,$7,now()+interval '30 minutes') ON CONFLICT(incident_key) DO UPDATE SET evidence=$7,severity=CASE WHEN otc_incidents.severity='P0' THEN 'P0' WHEN $2='P0' THEN 'P0' WHEN otc_incidents.severity='P1' THEN 'P1' ELSE $2 END";
const PAUSE_CONTROLS: &str =
    "UPDATE otc_controls SET paused=TRUE,signing_paused=TRUE WHERE singleton";
const FUNNEL: &str = "WITH first_event AS (SELECT COALESCE(trade_id,rfq_id) AS entity,min(created_at) AS at FROM otc_events WHERE kind=$1 GROUP BY COALESCE(trade_id,rfq_id)) SELECT count(*),count(*) FROM first_event WHERE at>now()-interval '24 hours'";
const QUOTE_COVERAGE: &str = "SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_trades t JOIN otc_inbox i ON i.activity_id=t.terms->'proposal'->>'id' AND i.actor=t.customer_actor WHERE t.rfq_id=r.id AND i.processed_at<=r.created_at+interval '120 seconds' AND i.processed_at<=(t.terms->>'quote_by')::timestamptz)) FROM otc_rfq r WHERE created_at<=now()-interval '120 seconds' AND created_at>now()-interval '24 hours'";
const ON_TIME_PAYOUT: &str = "WITH due AS (SELECT t.*,CASE WHEN t.terms->'escrow' IS NULL THEN (t.terms->>'payout_by')::timestamptz ELSE x.expires_at END AS deadline FROM otc_trades t LEFT JOIN otc_escrows x ON x.trade_id=t.id WHERE EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind='payment') OR EXISTS(SELECT 1 FROM otc_escrow_evidence e WHERE e.trade_id=t.id AND e.kind='funding' AND valid)) SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_events e WHERE e.trade_id=t.id AND e.kind='completed' AND e.created_at<=t.deadline)) FROM due t WHERE t.deadline BETWEEN now()-interval '24 hours' AND now()";
const TRADE_STATES: &str = "SELECT state,count(*),min(created_at) FROM otc_trades GROUP BY state";
const OBSERVERS: &str =
    "SELECT chain,checkpoint,last_scan,failures,lag,paused FROM otc_observers ORDER BY chain";
const LEDGER_FEES: &str =
    "SELECT kind,currency,sum(amount)::text FROM otc_ledger GROUP BY kind,currency";
const INVENTORY: &str = "SELECT i.chain,i.balance::text,i.gas_reserve::text,i.observed_at,COALESCE(sum(r.amount) FILTER(WHERE r.status!='released'),0)::text FROM otc_inventory i LEFT JOIN otc_reservations r USING(chain) GROUP BY i.chain";
const INCIDENTS: &str = "SELECT incident_key,severity,owner,backup,trade_id,runbook,evidence,detected_at,acknowledged_at,service_restored_at,financially_resolved_at,closed_at,next_update_at,actions,customer_updates FROM otc_incidents ORDER BY detected_at DESC LIMIT 100";
const RFQ_COHORT: &str = "SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_trades t WHERE t.rfq_id=r.id AND t.state='completed')) FROM otc_rfq r WHERE r.created_at>now()-interval '7 days'";
const COMPLETED_VOLUME_24H: &str = "SELECT COALESCE(sum((terms->'fee_policy'->>'basis_units')::numeric),0)::text FROM otc_trades t WHERE state='completed' AND (SELECT min(created_at) FROM otc_events e WHERE e.trade_id=t.id AND e.kind='completed')>now()-interval '24 hours'";
const OLDEST_UNDELIVERED: &str =
    "SELECT min(created_at) FROM otc_outbox WHERE delivered_at IS NULL";
const PAUSED: &str = "SELECT paused FROM otc_controls WHERE singleton";
const LEDGER_TOTALS: &str = "SELECT COALESCE(sum(amount) FILTER(WHERE kind='fee_accrued'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_collected'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_compensation'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind IN ('support_cost','monitoring_cost','exception_cost')),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='operator_minutes'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_refunded'),0)::text FROM otc_ledger";
const OPEN_OBLIGATIONS: &str = "SELECT t.id,t.state,t.terms,min(e.included_at) AS funded_at FROM otc_trades t JOIN (SELECT trade_id,included_at FROM otc_evidence WHERE kind IN ('payment','review_payment') UNION ALL SELECT trade_id,included_at FROM otc_escrow_evidence WHERE kind='funding' AND valid) e ON e.trade_id=t.id WHERE t.state NOT IN ('completed','refunded') GROUP BY t.id ORDER BY min(e.included_at)";
const UNSAFE_INCIDENTS: &str = "SELECT count(*) FROM otc_incidents WHERE severity='P0' AND service_restored_at IS NULL AND closed_at IS NULL";
const STALE_OBSERVERS: &str = "SELECT count(*) FROM otc_observers WHERE paused OR last_scan IS NULL OR last_scan<now()-interval '60 seconds'";
const SET_PAUSED: &str = "UPDATE otc_controls SET paused=$1,signing_paused=CASE WHEN NOT $1 THEN FALSE ELSE signing_paused END WHERE singleton";
const LOCK_INCIDENT: &str =
    "SELECT trade_id,severity FROM otc_incidents WHERE incident_key=$1 FOR UPDATE";
const TRADE_STATE: &str = "SELECT state FROM otc_trades WHERE id=$1";
const INCIDENT_RESOLVED: &str = "SELECT service_restored_at IS NOT NULL AND financially_resolved_at IS NOT NULL FROM otc_incidents WHERE incident_key=$1";
const TRADE_CUSTOMER: &str = "SELECT customer_actor FROM otc_trades WHERE id=$1";
const INCIDENT_ACTIONS_PREFIX: &str = "UPDATE otc_incidents SET ";
const INCIDENT_ACTIONS_SUFFIX: &str = "actions=actions||$2::jsonb,customer_updates=customer_updates||$3::jsonb,next_update_at=now()+interval '30 minutes' WHERE incident_key=$1";
const DUE_NOTIFICATIONS: &str = "SELECT incident_key,severity,owner,backup,runbook,detected_at,acknowledged_at FROM otc_incidents WHERE closed_at IS NULL AND next_notification_at<=now() ORDER BY detected_at LIMIT 20";
const INCIDENT_NOTIFIED: &str = "UPDATE otc_incidents SET notifications=notifications+1,next_notification_at=now()+interval '15 minutes' WHERE incident_key=$1";

pub(crate) async fn upsert_incident(
    client: &(impl GenericClient + Sync),
    key: &str,
    severity: &str,
    owner: &str,
    backup: &str,
    trade: &Option<Uuid>,
    runbook: &str,
    evidence: &Value,
) -> Result<()> {
    client
        .execute(
            UPSERT_INCIDENT,
            &[&key, &severity, &owner, &backup, trade, &runbook, evidence],
        )
        .await?;
    Ok(())
}

pub(crate) async fn pause_controls(client: &(impl GenericClient + Sync)) -> Result<()> {
    client.execute(PAUSE_CONTROLS, &[]).await?;
    Ok(())
}

/// Returns the number of distinct entities that reached the first funnel event
/// in the last 24 hours.
pub(crate) async fn funnel_entities(
    client: &(impl GenericClient + Sync),
    event: &str,
) -> Result<i64> {
    let row = client.query_one(FUNNEL, &[&event]).await?;
    Ok(row.get(0))
}

/// Returns `(denominator, numerator)` of RFQs quoted inside the SLA window.
pub(crate) async fn quote_coverage(client: &(impl GenericClient + Sync)) -> Result<(i64, i64)> {
    let row = client.query_one(QUOTE_COVERAGE, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

/// Returns `(denominator, numerator)` of funded trades paid out on time.
pub(crate) async fn on_time_payout(client: &(impl GenericClient + Sync)) -> Result<(i64, i64)> {
    let row = client.query_one(ON_TIME_PAYOUT, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

/// Returns `(state, count, oldest)` per trade state.
pub(crate) async fn trade_states(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, i64, Option<DateTime<Utc>>)>> {
    let rows = client.query(TRADE_STATES, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

/// Returns `(chain, checkpoint, last_scan, failures, lag, paused)` per observer.
#[allow(clippy::type_complexity)]
pub(crate) async fn observers(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, Value, Option<DateTime<Utc>>, i32, Option<i64>, bool)>> {
    let rows = client.query(OBSERVERS, &[]).await?;
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

/// Returns `(kind, currency, units)` per ledger entry kind.
pub(crate) async fn ledger_fees(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, String, String)>> {
    let rows = client.query(LEDGER_FEES, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

/// Returns `(chain, balance, gas_reserve, observed_at, reserved)` per chain.
#[allow(clippy::type_complexity)]
pub(crate) async fn inventory(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(String, String, String, DateTime<Utc>, String)>> {
    let rows = client.query(INVENTORY, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect())
}

/// Returns `(key, severity, owner, backup, trade_id, runbook, evidence,
/// detected_at, acknowledged_at, service_restored_at, financially_resolved_at,
/// closed_at, next_update_at, actions, customer_updates)` per open incident.
#[allow(clippy::type_complexity)]
pub(crate) async fn incidents(
    client: &(impl GenericClient + Sync),
) -> Result<
    Vec<(
        String,
        String,
        String,
        String,
        Option<Uuid>,
        String,
        Value,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Value,
        Value,
    )>,
> {
    let rows = client.query(INCIDENTS, &[]).await?;
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
                row.get(6),
                row.get(7),
                row.get(8),
                row.get(9),
                row.get(10),
                row.get(11),
                row.get(12),
                row.get(13),
                row.get(14),
            )
        })
        .collect())
}

/// Returns `(denominator, numerator)` of RFQs from the last seven days that completed.
pub(crate) async fn rfq_cohort(client: &(impl GenericClient + Sync)) -> Result<(i64, i64)> {
    let row = client.query_one(RFQ_COHORT, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

/// Completed desk volume in USDT units over the last 24 hours, as decimal text.
pub(crate) async fn completed_volume_24h(client: &(impl GenericClient + Sync)) -> Result<String> {
    Ok(client.query_one(COMPLETED_VOLUME_24H, &[]).await?.get(0))
}

pub(crate) async fn oldest_undelivered(
    client: &(impl GenericClient + Sync),
) -> Result<Option<DateTime<Utc>>> {
    Ok(client.query_one(OLDEST_UNDELIVERED, &[]).await?.get(0))
}

pub(crate) async fn paused(client: &(impl GenericClient + Sync)) -> Result<bool> {
    Ok(client.query_one(PAUSED, &[]).await?.get(0))
}

/// Returns `(fee_accrued, fee_collected, fee_compensation, costs, operator_minutes,
/// fee_refunded)` totals as decimal text.
pub(crate) async fn ledger_totals(
    client: &(impl GenericClient + Sync),
) -> Result<(String, String, String, String, String, String)> {
    let row = client.query_one(LEDGER_TOTALS, &[]).await?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
    ))
}

/// Returns `(trade_id, state, terms, funded_at)` per funded case still open.
pub(crate) async fn open_obligations(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<(Uuid, String, Value, DateTime<Utc>)>> {
    let rows = client.query(OPEN_OBLIGATIONS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
        .collect())
}

pub(crate) async fn unsafe_incidents(client: &(impl GenericClient + Sync)) -> Result<i64> {
    Ok(client.query_one(UNSAFE_INCIDENTS, &[]).await?.get(0))
}

pub(crate) async fn stale_observers(client: &(impl GenericClient + Sync)) -> Result<i64> {
    Ok(client.query_one(STALE_OBSERVERS, &[]).await?.get(0))
}

pub(crate) async fn set_paused(client: &(impl GenericClient + Sync), paused: bool) -> Result<()> {
    client.execute(SET_PAUSED, &[&paused]).await?;
    Ok(())
}

/// Returns `(trade_id, severity)` for the locked incident.
pub(crate) async fn lock_incident(
    client: &(impl GenericClient + Sync),
    key: &str,
) -> Result<Option<(Option<Uuid>, String)>> {
    let row = client.query_opt(LOCK_INCIDENT, &[&key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1))))
}

pub(crate) async fn trade_state(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(TRADE_STATE, &[id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn incident_resolved(
    client: &(impl GenericClient + Sync),
    key: &str,
) -> Result<bool> {
    Ok(client.query_one(INCIDENT_RESOLVED, &[&key]).await?.get(0))
}

pub(crate) async fn trade_customer(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(TRADE_CUSTOMER, &[id])
        .await?
        .map(|row| row.get(0)))
}

/// Records an incident action, optionally stamping one timestamp column, and
/// extends the next customer update window.
pub(crate) async fn record_incident_action(
    client: &(impl GenericClient + Sync),
    key: &str,
    stamp: &str,
    action: &Value,
    customer_update: &Value,
) -> Result<()> {
    let query = format!("{INCIDENT_ACTIONS_PREFIX}{stamp}{INCIDENT_ACTIONS_SUFFIX}");
    client
        .execute(&query, &[&key, action, customer_update])
        .await?;
    Ok(())
}

/// Returns `(key, severity, owner, backup, runbook, detected_at, acknowledged_at)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn due_notifications(
    client: &(impl GenericClient + Sync),
) -> Result<
    Vec<(
        String,
        String,
        String,
        String,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )>,
> {
    let rows = client.query(DUE_NOTIFICATIONS, &[]).await?;
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
                row.get(6),
            )
        })
        .collect())
}

pub(crate) async fn incident_notified(
    client: &(impl GenericClient + Sync),
    key: &str,
) -> Result<()> {
    client.execute(INCIDENT_NOTIFIED, &[&key]).await?;
    Ok(())
}

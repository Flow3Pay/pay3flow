use super::{
    error::{invalid, Error, Result},
    Service,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IncidentAction {
    pub action: String,
    pub evidence: Value,
    pub customer_update: Option<String>,
}
impl Service {
    pub(crate) async fn incident(
        &self,
        key: &str,
        severity: &str,
        trade: Option<Uuid>,
        runbook: &str,
        evidence: Value,
    ) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute("INSERT INTO otc_incidents(incident_key,severity,owner,backup,trade_id,runbook,evidence,next_update_at) VALUES($1,$2,$3,$4,$5,$6,$7,now()+interval '30 minutes') ON CONFLICT(incident_key) DO UPDATE SET evidence=$7,severity=CASE WHEN otc_incidents.severity='P0' THEN 'P0' WHEN $2='P0' THEN 'P0' WHEN otc_incidents.severity='P1' THEN 'P1' ELSE $2 END",&[&key,&severity,&self.cfg.incident_owner,&self.cfg.incident_backup,&trade,&format!("/otc-runbook#{runbook}"),&evidence]).await?;
        if severity == "P0" {
            client
                .execute(
                    "UPDATE otc_controls SET paused=TRUE,signing_paused=TRUE WHERE singleton",
                    &[],
                )
                .await?;
        }
        Ok(())
    }
    pub(crate) async fn dashboard(&self, actor: &str) -> Result<Value> {
        self.desk(actor)?;
        let client = self.pool.get().await?;
        let mut funnel = serde_json::Map::new();
        for event in [
            "rfq_requested",
            "quoted",
            "accepted",
            "funded",
            "completed",
            "refunded",
        ] {
            let row=client.query_one("WITH first_event AS (SELECT COALESCE(trade_id,rfq_id) AS entity,min(created_at) AS at FROM otc_events WHERE kind=$1 GROUP BY COALESCE(trade_id,rfq_id)) SELECT count(*),count(*) FROM first_event WHERE at>now()-interval '24 hours'",&[&event]).await?;
            funnel.insert(event.into(), json!(row.get::<_, i64>(0)));
        }
        let coverage=client.query_one("SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_trades t JOIN otc_inbox i ON i.activity_id=t.terms->'proposal'->>'id' AND i.actor=t.customer_actor WHERE t.rfq_id=r.id AND i.processed_at<=r.created_at+interval '120 seconds' AND i.processed_at<=(t.terms->>'quote_by')::timestamptz)) FROM otc_rfq r WHERE created_at<=now()-interval '120 seconds' AND created_at>now()-interval '24 hours'",&[]).await?;
        let quotes_den: i64 = coverage.get(0);
        let quotes_num: i64 = coverage.get(1);
        let payout=client.query_one("SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_events e WHERE e.trade_id=t.id AND e.kind='completed' AND e.created_at<=(t.terms->>'payout_by')::timestamptz)) FROM otc_trades t WHERE (t.terms->>'payout_by')::timestamptz BETWEEN now()-interval '24 hours' AND now() AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind='payment')",&[]).await?;
        let states:Vec<Value>=client.query("SELECT state,count(*),min(created_at) FROM otc_trades GROUP BY state",&[]).await?.iter().map(|r|json!({"state":r.get::<_,String>(0),"count":r.get::<_,i64>(1),"oldest":r.get::<_,Option<DateTime<Utc>>>(2)})).collect();
        let observers:Vec<Value>=client.query("SELECT chain,checkpoint,last_scan,failures,lag,paused FROM otc_observers ORDER BY chain",&[]).await?.iter().map(|r|json!({"chain":r.get::<_,String>(0),"checkpoint":r.get::<_,Value>(1),"last_scan":r.get::<_,Option<DateTime<Utc>>>(2),"failures":r.get::<_,i32>(3),"lag":r.get::<_,Option<i64>>(4),"paused":r.get::<_,bool>(5)})).collect();
        let fees:Vec<Value>=client.query("SELECT kind,currency,sum(amount)::text FROM otc_ledger GROUP BY kind,currency",&[]).await?.iter().map(|r|json!({"kind":r.get::<_,String>(0),"currency":r.get::<_,String>(1),"units":r.get::<_,String>(2)})).collect();
        let inventory:Vec<Value>=client.query("SELECT i.chain,i.balance::text,i.gas_reserve::text,i.observed_at,COALESCE(sum(r.amount) FILTER(WHERE r.status!='released'),0)::text FROM otc_inventory i LEFT JOIN otc_reservations r USING(chain) GROUP BY i.chain",&[]).await?.iter().map(|r|json!({"chain":r.get::<_,String>(0),"balance":r.get::<_,String>(1),"gas_reserve":r.get::<_,String>(2),"observed_at":r.get::<_,DateTime<Utc>>(3),"reserved":r.get::<_,String>(4)})).collect();
        let incidents:Vec<Value>=client.query("SELECT incident_key,severity,owner,backup,trade_id,runbook,evidence,detected_at,acknowledged_at,service_restored_at,financially_resolved_at,closed_at,next_update_at,actions,customer_updates FROM otc_incidents ORDER BY detected_at DESC LIMIT 100",&[]).await?.iter().map(|r|json!({"key":r.get::<_,String>(0),"severity":r.get::<_,String>(1),"owner":r.get::<_,String>(2),"backup":r.get::<_,String>(3),"trade_id":r.get::<_,Option<Uuid>>(4),"runbook":r.get::<_,String>(5),"evidence":r.get::<_,Value>(6),"detected_at":r.get::<_,DateTime<Utc>>(7),"acknowledged_at":r.get::<_,Option<DateTime<Utc>>>(8),"service_restored_at":r.get::<_,Option<DateTime<Utc>>>(9),"financially_resolved_at":r.get::<_,Option<DateTime<Utc>>>(10),"closed_at":r.get::<_,Option<DateTime<Utc>>>(11),"next_update_at":r.get::<_,Option<DateTime<Utc>>>(12),"actions":r.get::<_,Value>(13),"customer_updates":r.get::<_,Value>(14)})).collect();
        let rfq_cohort=client.query_one("SELECT count(*),count(*) FILTER(WHERE EXISTS(SELECT 1 FROM otc_trades t WHERE t.rfq_id=r.id AND t.state='completed')) FROM otc_rfq r WHERE r.created_at>now()-interval '7 days'",&[]).await?;
        let volume:String=client.query_one("SELECT COALESCE(sum((terms->'fee_policy'->>'basis_units')::numeric),0)::text FROM otc_trades t WHERE state='completed' AND (SELECT min(created_at) FROM otc_events e WHERE e.trade_id=t.id AND e.kind='completed')>now()-interval '24 hours'",&[]).await?.get(0);
        let oldest: Option<DateTime<Utc>> = client
            .query_one(
                "SELECT min(created_at) FROM otc_outbox WHERE delivered_at IS NULL",
                &[],
            )
            .await?
            .get(0);
        let paused: bool = client
            .query_one("SELECT paused FROM otc_controls WHERE singleton", &[])
            .await?
            .get(0);
        let totals=client.query_one("SELECT COALESCE(sum(amount) FILTER(WHERE kind='fee_accrued'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_collected'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_compensation'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind IN ('support_cost','monitoring_cost','exception_cost')),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='operator_minutes'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_refunded'),0)::text FROM otc_ledger",&[]).await?;
        let accrued = super::amount::integer(&totals.get::<_, String>(0))?;
        let collected = super::amount::integer(&totals.get::<_, String>(1))?
            .saturating_sub(super::amount::integer(&totals.get::<_, String>(5))?);
        let compensation = super::amount::integer(&totals.get::<_, String>(2))?;
        let costs = super::amount::integer(&totals.get::<_, String>(3))?;
        let contribution = if collected >= costs {
            (collected - costs).to_string()
        } else {
            format!("-{}", costs - collected)
        };
        let obligations:Vec<Value>=client.query("SELECT t.id,t.state,t.terms, min(e.included_at) AS funded_at FROM otc_trades t JOIN otc_evidence e ON e.trade_id=t.id AND e.kind IN ('payment','review_payment') WHERE t.state NOT IN ('completed','refunded') GROUP BY t.id ORDER BY min(e.included_at)",&[]).await?.iter().map(|r|json!({"trade_id":r.get::<_,Uuid>(0),"state":r.get::<_,String>(1),"terms":r.get::<_,Value>(2),"funded_at":r.get::<_,DateTime<Utc>>(3)})).collect();
        let reporting = self.reporting().await?;
        Ok(
            json!({"reporting":reporting,"environment":if self.cfg.demo{"demo"}else{"live"},"desk":self.cfg.desk_actor,"paused":paused,"funnel":funnel,"states":states,"quote_coverage":{"numerator":quotes_num,"denominator":quotes_den,"rate":rate(quotes_num,quotes_den)},"on_time_payout":{"numerator":payout.get::<_,i64>(1),"denominator":payout.get::<_,i64>(0),"rate":rate(payout.get(1),payout.get(0))},"conversion_7d":{"numerator":rfq_cohort.get::<_,i64>(1),"denominator":rfq_cohort.get::<_,i64>(0)},"completed_usdt_units_24h":volume,"oldest_undelivered":oldest,"observers":observers,"inventory":inventory,"fees":fees,"incidents":incidents,"open_funded_cases":obligations,"economics":{"operator_minutes":totals.get::<_,String>(4),"attributable_costs_usdt_units":costs.to_string(),"collected_usdt_units":collected.to_string(),"accrued_usdt_units":accrued.to_string(),"unpaid_usdt_units":accrued.saturating_sub(compensation).saturating_sub(collected).to_string(),"contribution_usdt_units":contribution}}),
        )
    }
    pub(crate) async fn control(&self, actor: &str, paused: bool) -> Result<Value> {
        self.desk(actor)?;
        if !paused {
            let client = self.pool.get().await?;
            let unsafe_incidents:i64=client.query_one("SELECT count(*) FROM otc_incidents WHERE severity='P0' AND service_restored_at IS NULL AND closed_at IS NULL",&[]).await?.get(0);
            if unsafe_incidents > 0 {
                return Err(Error::Conflict(
                    "restore and verify P0 containment before signing resumes".into(),
                ));
            }
            self.cfg.validate()?;
            if !self.cfg.enabled {
                return Err(Error::Disabled);
            }
            let client = self.pool.get().await?;
            let count:i64=client.query_one("SELECT count(*) FROM otc_observers WHERE paused OR last_scan IS NULL OR last_scan<now()-interval '60 seconds'",&[]).await?.get(0);
            if count > 0 {
                return Err(Error::Disabled);
            }
        }
        let client = self.pool.get().await?;
        client.execute("UPDATE otc_controls SET paused=$1,signing_paused=CASE WHEN NOT $1 THEN FALSE ELSE signing_paused END WHERE singleton",&[&paused]).await?;
        Ok(json!({"paused":paused}))
    }
    pub(crate) async fn incident_action(
        &self,
        actor: &str,
        key: &str,
        request: IncidentAction,
    ) -> Result<Value> {
        self.desk(actor)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT trade_id,severity FROM otc_incidents WHERE incident_key=$1 FOR UPDATE",
                &[&key],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let trade: Option<Uuid> = row.get(0);
        let field = match request.action.as_str() {
            "acknowledge" => "acknowledged_at",
            "service_restored" => "service_restored_at",
            "financially_resolved" => "financially_resolved_at",
            "close" => "closed_at",
            "update" => "next_update_at",
            _ => return Err(invalid("unknown incident action")),
        };
        if request.evidence.is_null() || request.evidence.as_object().is_none_or(|o| o.is_empty()) {
            return Err(invalid("incident action requires evidence"));
        }
        if ["financially_resolved", "close"].contains(&request.action.as_str()) {
            if let Some(trade) = trade {
                let state: String = tx
                    .query_one("SELECT state FROM otc_trades WHERE id=$1", &[&trade])
                    .await?
                    .get(0);
                if !["completed", "refunded", "rejected", "expired"].contains(&state.as_str()) {
                    return Err(Error::Conflict(
                        "financial obligation remains unresolved".into(),
                    ));
                }
            }
            if request.action == "close" {
                let allowed:bool=tx.query_one("SELECT service_restored_at IS NOT NULL AND financially_resolved_at IS NOT NULL FROM otc_incidents WHERE incident_key=$1",&[&key]).await?.get(0);
                if !allowed {
                    return Err(Error::Conflict(
                        "record service restoration and financial resolution before closure".into(),
                    ));
                }
            }
        }
        if let (Some(id), Some(message)) = (trade, request.customer_update.as_ref()) {
            let row = tx
                .query_one("SELECT customer_actor FROM otc_trades WHERE id=$1", &[&id])
                .await?;
            let customer: String = row.get(0);
            let update_id = Uuid::new_v4();
            let body = json!({"@context":super::protocol::context(),"id":format!("{actor}/updates/{update_id}"),"type":"Create","actor":actor,"to":[customer],"object":{"id":format!("{actor}/updates/{update_id}/notice"),"type":"Document","context":format!("{actor}/decisions/{id}/agreement"),"name":"OTC incident update","content":message}});
            super::repo::enqueue(&tx, actor, &body, "customer_update").await?;
        }
        if request.action == "close" && ["P0", "P1"].contains(&row.get::<_, String>(1).as_str()) {
            for field in [
                "root_cause",
                "preventive_test",
                "follow_up_owner",
                "follow_up_date",
            ] {
                if request.evidence[field]
                    .as_str()
                    .is_none_or(|v| v.trim().is_empty())
                {
                    return Err(invalid("P0/P1 closure requires root cause, preventive test, follow-up owner and date"));
                }
            }
        }
        let action = json!({"actor":actor,"action":request.action,"evidence":request.evidence,"at":Utc::now()});
        let stamp = if field == "next_update_at" {
            String::new()
        } else {
            format!("{field}=COALESCE({field},now()),")
        };
        let query=format!("UPDATE otc_incidents SET {stamp}actions=actions||$2::jsonb,customer_updates=customer_updates||$3::jsonb,next_update_at=now()+interval '30 minutes' WHERE incident_key=$1");
        tx.execute(
            &query,
            &[
                &key,
                &json!([action]),
                &request
                    .customer_update
                    .map(|message| json!([{"message":message,"at":Utc::now()}]))
                    .unwrap_or(json!([])),
            ],
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"recorded":true}))
    }
    pub(crate) async fn notify_incidents(&self) -> Result<()> {
        let Some(url) = &self.cfg.alert_webhook else {
            return Ok(());
        };
        let client = self.pool.get().await?;
        let rows=client.query("SELECT incident_key,severity,owner,backup,runbook,detected_at,acknowledged_at FROM otc_incidents WHERE closed_at IS NULL AND next_notification_at<=now() ORDER BY detected_at LIMIT 20",&[]).await?;
        for row in rows {
            let key: String = row.get(0);
            let severity: String = row.get(1);
            let detected: DateTime<Utc> = row.get(5);
            let acknowledged: Option<DateTime<Utc>> = row.get(6);
            let limit = match severity.as_str() {
                "P0" => 300,
                "P1" => 900,
                _ => 14400,
            };
            let escalated = acknowledged.is_none()
                && if severity == "P2" {
                    staffed_seconds(
                        detected,
                        Utc::now(),
                        self.cfg.staffed_utc_start,
                        self.cfg.staffed_utc_end,
                    ) > limit
                } else {
                    (Utc::now() - detected).num_seconds() > limit
                };
            let body = json!({"incident_key":super::protocol::digest(&json!(key)),"severity":severity,"owner":row.get::<_,String>(if escalated{3}else{2}),"escalated":escalated,"runbook":row.get::<_,String>(4),"dashboard":format!("{}/#/otc",self.cfg.origin)});
            if self
                .http
                .post(url)
                .json(&body)
                .send()
                .await?
                .status()
                .is_success()
            {
                client.execute("UPDATE otc_incidents SET notifications=notifications+1,next_notification_at=now()+interval '15 minutes' WHERE incident_key=$1",&[&key]).await?;
            }
        }
        Ok(())
    }
}
fn rate(num: i64, den: i64) -> Option<f64> {
    if den == 0 {
        None
    } else {
        Some(num as f64 / den as f64)
    }
}

fn staffed_seconds(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    hour_start: u32,
    hour_end: u32,
) -> i64 {
    let mut day = start.date_naive();
    let mut seconds = 0;
    while day <= end.date_naive() {
        let from = day
            .and_hms_opt(hour_start, 0, 0)
            .map(|t| t.and_utc())
            .unwrap_or(start);
        let to = if hour_end == 24 {
            day.succ_opt().and_then(|d| d.and_hms_opt(0, 0, 0))
        } else {
            day.and_hms_opt(hour_end, 0, 0)
        }
        .map(|t| t.and_utc())
        .unwrap_or(end);
        seconds += (to.min(end) - from.max(start)).num_seconds().max(0);
        let Some(next) = day.succ_opt() else { break };
        day = next;
    }
    seconds
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn p2_escalation_counts_staffed_hours_only() {
        let start = DateTime::parse_from_rfc3339("2026-10-09T17:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339("2026-10-10T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(staffed_seconds(start, end, 9, 18), 7200);
    }
}

use super::{
    error::{invalid, Error, Result},
    repo, Service,
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
        repo::operations::upsert_incident(
            &client,
            key,
            severity,
            &self.cfg.incident_owner,
            &self.cfg.incident_backup,
            &trade,
            &format!("/otc-runbook#{runbook}"),
            &evidence,
        )
        .await?;
        if severity == "P0" {
            repo::operations::pause_controls(&client).await?;
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
            let count = repo::operations::funnel_entities(&client, event).await?;
            funnel.insert(event.into(), json!(count));
        }
        let (quotes_den, quotes_num) = repo::operations::quote_coverage(&client).await?;
        let (payout_den, payout_num) = repo::operations::on_time_payout(&client).await?;
        let states: Vec<Value> = repo::operations::trade_states(&client)
            .await?
            .into_iter()
            .map(|(state, count, oldest)| json!({"state":state,"count":count,"oldest":oldest}))
            .collect();
        let observers:Vec<Value>=repo::operations::observers(&client).await?.into_iter().map(|(chain,checkpoint,last_scan,failures,lag,paused)|json!({"chain":chain,"checkpoint":checkpoint,"last_scan":last_scan,"failures":failures,"lag":lag,"paused":paused})).collect();
        let fees: Vec<Value> = repo::operations::ledger_fees(&client)
            .await?
            .into_iter()
            .map(|(kind, currency, units)| json!({"kind":kind,"currency":currency,"units":units}))
            .collect();
        let inventory:Vec<Value>=repo::operations::inventory(&client).await?.into_iter().map(|(chain,balance,gas_reserve,observed_at,reserved)|json!({"chain":chain,"balance":balance,"gas_reserve":gas_reserve,"observed_at":observed_at,"reserved":reserved})).collect();
        let incidents:Vec<Value>=repo::operations::incidents(&client).await?.into_iter().map(|(key,severity,owner,backup,trade_id,runbook,evidence,detected_at,acknowledged_at,service_restored_at,financially_resolved_at,closed_at,next_update_at,actions,customer_updates)|json!({"key":key,"severity":severity,"owner":owner,"backup":backup,"trade_id":trade_id,"runbook":runbook,"evidence":evidence,"detected_at":detected_at,"acknowledged_at":acknowledged_at,"service_restored_at":service_restored_at,"financially_resolved_at":financially_resolved_at,"closed_at":closed_at,"next_update_at":next_update_at,"actions":actions,"customer_updates":customer_updates})).collect();
        let (rfq_den, rfq_num) = repo::operations::rfq_cohort(&client).await?;
        let volume = repo::operations::completed_volume_24h(&client).await?;
        let oldest = repo::operations::oldest_undelivered(&client).await?;
        let paused = repo::operations::paused(&client).await?;
        let totals = repo::operations::ledger_totals(&client).await?;
        let accrued = super::amount::integer(&totals.0)?;
        let collected =
            super::amount::integer(&totals.1)?.saturating_sub(super::amount::integer(&totals.5)?);
        let compensation = super::amount::integer(&totals.2)?;
        let costs = super::amount::integer(&totals.3)?;
        let contribution = if collected >= costs {
            (collected - costs).to_string()
        } else {
            format!("-{}", costs - collected)
        };
        let obligations:Vec<Value>=repo::operations::open_obligations(&client).await?.into_iter().map(|(trade_id,state,terms,funded_at)|json!({"trade_id":trade_id,"state":state,"terms":terms,"funded_at":funded_at})).collect();
        let reporting = self.reporting().await?;
        Ok(
            json!({"reporting":reporting,"environment":if self.cfg.demo{"demo"}else{"live"},"desk":self.cfg.desk_actor,"paused":paused,"funnel":funnel,"states":states,"quote_coverage":{"numerator":quotes_num,"denominator":quotes_den,"rate":rate(quotes_num,quotes_den)},"on_time_payout":{"numerator":payout_num,"denominator":payout_den,"rate":rate(payout_num,payout_den)},"conversion_7d":{"numerator":rfq_num,"denominator":rfq_den},"completed_usdt_units_24h":volume,"oldest_undelivered":oldest,"observers":observers,"inventory":inventory,"fees":fees,"incidents":incidents,"open_funded_cases":obligations,"economics":{"operator_minutes":totals.4,"attributable_costs_usdt_units":costs.to_string(),"collected_usdt_units":collected.to_string(),"accrued_usdt_units":accrued.to_string(),"unpaid_usdt_units":accrued.saturating_sub(compensation).saturating_sub(collected).to_string(),"contribution_usdt_units":contribution}}),
        )
    }
    pub(crate) async fn control(&self, actor: &str, paused: bool) -> Result<Value> {
        self.desk(actor)?;
        if !paused {
            let client = self.pool.get().await?;
            let unsafe_incidents = repo::operations::unsafe_incidents(&client).await?;
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
            let count = repo::operations::stale_observers(&client).await?;
            if count > 0 {
                return Err(Error::Disabled);
            }
        }
        let client = self.pool.get().await?;
        repo::operations::set_paused(&client, paused).await?;
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
        let row = repo::operations::lock_incident(&tx, key)
            .await?
            .ok_or(Error::NotFound)?;
        let trade: Option<Uuid> = row.0;
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
                let state = repo::operations::trade_state(&tx, &trade)
                    .await?
                    .ok_or(Error::NotFound)?;
                if !["completed", "refunded", "rejected", "expired"].contains(&state.as_str()) {
                    return Err(Error::Conflict(
                        "financial obligation remains unresolved".into(),
                    ));
                }
            }
            if request.action == "close" {
                let allowed = repo::operations::incident_resolved(&tx, key).await?;
                if !allowed {
                    return Err(Error::Conflict(
                        "record service restoration and financial resolution before closure".into(),
                    ));
                }
            }
        }
        if let (Some(id), Some(message)) = (trade, request.customer_update.as_ref()) {
            let customer = repo::operations::trade_customer(&tx, &id)
                .await?
                .ok_or(Error::NotFound)?;
            let update_id = Uuid::new_v4();
            let body = json!({"@context":super::protocol::context(),"id":format!("{actor}/updates/{update_id}"),"type":"Create","actor":actor,"to":[customer],"object":{"id":format!("{actor}/updates/{update_id}/notice"),"type":"Document","context":format!("{actor}/decisions/{id}/agreement"),"name":"OTC incident update","content":message}});
            super::repo::enqueue(&tx, actor, &body, "customer_update").await?;
        }
        if request.action == "close" && ["P0", "P1"].contains(&row.1.as_str()) {
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
        repo::operations::record_incident_action(
            &tx,
            key,
            &stamp,
            &json!([action]),
            &request
                .customer_update
                .map(|message| json!([{"message":message,"at":Utc::now()}]))
                .unwrap_or(json!([])),
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
        let rows = repo::operations::due_notifications(&client).await?;
        for row in rows {
            let (key, severity, owner, backup, runbook, detected, acknowledged) = row;
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
            let body = json!({"incident_key":super::protocol::digest(&json!(key)),"severity":severity,"owner":if escalated{backup}else{owner},"escalated":escalated,"runbook":runbook,"dashboard":format!("{}/#/otc",self.cfg.origin)});
            if self
                .http
                .post(url)
                .json(&body)
                .send()
                .await?
                .status()
                .is_success()
            {
                repo::operations::incident_notified(&client, &key).await?;
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

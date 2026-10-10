use super::{error::Result, model::Evidence, Service};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
impl Service {
    pub(crate) async fn tick(&self) -> Result<()> {
        if !self.cfg.enabled {
            let pending:i64=self.pool.get().await?.query_one("SELECT count(*) FROM otc_trades WHERE state NOT IN ('completed','refunded','rejected','expired')",&[]).await?.get(0);
            if pending == 0 {
                return Ok(());
            }
        }
        let client = self.pool.get().await?;
        client
            .execute(
                "UPDATE otc_controls SET demo=$1 WHERE singleton AND demo IS NULL",
                &[&self.cfg.demo],
            )
            .await?;
        let mode: bool = client
            .query_one("SELECT demo FROM otc_controls WHERE singleton", &[])
            .await?
            .get(0);
        if mode != self.cfg.demo {
            return Err(super::error::Error::Disabled);
        }
        if !self.cfg.demo {
            self.observer_tick().await?;
        }
        if let Err(error) = self.relay_tick().await {
            tracing::warn!(error=%error,"otc.relay.tick_failed");
        }
        if !self.cfg.demo {
            if let Err(error) = self.transfer_tick().await {
                tracing::warn!(error=%error,"otc.transfers.tick_failed");
            }
            self.revalidate().await?;
        }
        self.alert_tick().await?;
        self.notify_incidents().await?;
        if let Some(url) = &self.cfg.heartbeat_url {
            self.http
                .post(url)
                .json(&json!({"service":"otc","at":Utc::now()}))
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }
    async fn observer_tick(&self) -> Result<()> {
        for chain in ["ethereum", "everscale"] {
            let result = if chain == "ethereum" {
                self.ethereum_scan().await
            } else {
                self.everscale_scan().await
            };
            let mut client = self.pool.get().await?;
            let tx = client.transaction().await?;
            match result {
                Ok((checkpoint, balance, at, lag)) => {
                    // Observed balances are authoritative only at their checkpoint. Keep debits
                    // reserved until the snapshot includes the verified outgoing transfer.
                    let gas = if chain == "ethereum" {
                        "0".to_string()
                    } else {
                        super::amount::units(&self.cfg.ever_gas_reserve, 9)?.to_string()
                    };
                    tx.execute("INSERT INTO otc_inventory(chain,balance,gas_reserve,observed_at,snapshot) VALUES($1,$2::text::numeric,$3::text::numeric,now(),$4) ON CONFLICT(chain) DO UPDATE SET balance=$2::text::numeric,gas_reserve=$3::text::numeric,observed_at=now(),snapshot=$4",&[&chain,&balance,&gas,&checkpoint]).await?;
                    tx.execute("UPDATE otc_reservations r SET status='released' WHERE chain=$1 AND status='debit' AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=r.trade_id AND e.kind IN ('payout','refund') AND e.valid AND e.included_at<=$2)",&[&chain,&at]).await?;
                    tx.execute("UPDATE otc_observers SET checkpoint=$2,last_scan=now(),failures=0,lag=$3,paused=FALSE,last_error=NULL WHERE chain=$1",&[&chain,&checkpoint,&(lag as i64)]).await?;
                }
                Err(error) => {
                    tracing::warn!(chain,error=%error,"otc.observer.scan_failed");
                    tx.execute("UPDATE otc_observers SET failures=failures+1,paused=TRUE,last_error='scan failed' WHERE chain=$1",&[&chain]).await?;
                }
            }
            tx.commit().await?;
        }
        Ok(())
    }
    async fn revalidate(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows=client.query("SELECT trade_id,kind,evidence FROM otc_evidence WHERE valid ORDER BY last_checked_at ASC NULLS FIRST LIMIT 100",&[]).await?;
        for row in rows {
            let id: Uuid = row.get(0);
            let kind: String = row.get(1);
            let evidence: Evidence = serde_json::from_value(row.get(2))?;
            let invalid = if evidence.chain == "ethereum" {
                let reference = evidence.raw["receipt"]["transactionHash"]
                    .as_str()
                    .unwrap_or("");
                match self
                    .evm_agree("eth_getTransactionReceipt", json!([reference]))
                    .await
                {
                    Ok(current) => {
                        if current.is_null()
                            || current["blockHash"] != evidence.raw["receipt"]["blockHash"]
                            || super::chain::number(&current["status"]).ok() != Some(1)
                        {
                            true
                        } else {
                            match self
                                .evm_agree(
                                    "eth_getBlockByNumber",
                                    json!([current["blockNumber"], false]),
                                )
                                .await
                            {
                                Ok(block) => block["hash"] != current["blockHash"],
                                Err(_) => false,
                            }
                        }
                    }
                    Err(_) => false,
                }
            } else {
                match self
                    .everscale_evidence(evidence.raw["id"].as_str().unwrap_or(""), &evidence.leg)
                    .await
                {
                    Ok(Some(current)) => !current.finalized || !current.successful,
                    Ok(None) => true,
                    Err(_) => false,
                }
            };
            client
                .execute(
                    "UPDATE otc_evidence SET last_checked_at=now() WHERE trade_id=$1 AND kind=$2",
                    &[&id, &kind],
                )
                .await?;
            if invalid {
                let mut client = self.pool.get().await?;
                let tx = client.transaction().await?;
                tx.execute(
                    "UPDATE otc_evidence SET valid=FALSE WHERE trade_id=$1 AND kind=$2",
                    &[&id, &kind],
                )
                .await?;
                tx.execute(
                    "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1",
                    &[&id],
                )
                .await?;
                tx.execute("UPDATE otc_attempts SET state='unknown' WHERE trade_id=$1 AND kind=$2 AND state IN ('verified','review')",&[&id,&if kind=="review_payment" {"payment"}else{&kind}]).await?;
                super::repo::event(
                    &tx,
                    Some(id),
                    None,
                    "evidence_invalidated",
                    &json!({"kind":kind}),
                )
                .await?;
                tx.commit().await?;
                self.incident(
                    &format!("canonical:{id}:{kind}"),
                    "P0",
                    Some(id),
                    "integrity",
                    json!({"kind":kind}),
                )
                .await?;
            }
        }
        Ok(())
    }
    pub(crate) async fn alert_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows=client.query("SELECT id FROM otc_trades WHERE state NOT IN ('completed','refunded','rejected','expired') AND (terms->>'payout_by')::timestamptz<now() AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=otc_trades.id AND e.kind IN ('payment','review_payment'))",&[]).await?;
        for row in rows {
            let id: Uuid = row.get(0);
            self.incident(
                &format!("overdue:{id}"),
                "P1",
                Some(id),
                "payout",
                json!({}),
            )
            .await?;
        }
        for row in client.query("SELECT trade_id,id FROM otc_attempts WHERE kind IN ('payout','refund') AND state='unknown'",&[]).await?{let id:Uuid=row.get(0);let attempt:Uuid=row.get(1);self.incident(&format!("unknown-outgoing:{attempt}"),"P1",Some(id),"payout",json!({"attempt":attempt})).await?;}
        for row in client.query("SELECT chain FROM otc_observers WHERE paused OR failures>=3 OR last_scan IS NULL OR last_scan<now()-interval '60 seconds'",&[]).await?{
            let chain:String=row.get(0);let count:i64=client.query_one("SELECT count(*) FROM otc_trades t WHERE state NOT IN ('completed','refunded','rejected','expired') AND EXISTS(SELECT 1 FROM otc_evidence e WHERE e.trade_id=t.id AND e.kind IN ('payment','review_payment'))",&[]).await?.get(0);
            self.incident(&format!("observer:{chain}"),if count>0{"P1"}else{"P2"},None,"observer",json!({"chain":chain})).await?;
        }
        for row in client.query("SELECT activity_id FROM otc_outbox WHERE delivered_at IS NULL AND created_at<now()-interval '60 seconds'",&[]).await?{let id:String=row.get(0);self.incident(&format!("delivery:{id}"),"P2",None,"federation",json!({"activity":id})).await?;}
        for row in client.query("SELECT id FROM otc_trades WHERE state='booking_pending' AND updated_at<now()-interval '60 seconds'",&[]).await?{let id:Uuid=row.get(0);self.incident(&format!("booking:{id}"),"P2",Some(id),"federation",json!({})).await?;}
        for row in client.query("SELECT r.trade_id FROM otc_reservations r JOIN otc_inventory i ON i.chain=r.chain WHERE r.status!='released' AND i.balance-i.gas_reserve<(SELECT COALESCE(sum(amount),0) FROM otc_reservations WHERE chain=i.chain AND status!='released')",&[]).await? {
            let id:Uuid=row.get(0);let funded=client.query_opt("SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind IN ('payment','review_payment') AND valid",&[&id]).await?.is_some();
            self.incident(&format!("inventory:{id}"),if funded {"P1"}else{"P2"},Some(id),"payout",json!({"shortfall":true})).await?;
        }
        for row in client.query("SELECT trade_id,incident_key FROM otc_incidents WHERE closed_at IS NULL AND trade_id IS NOT NULL AND next_update_at<now()",&[]).await? {
            let id:Uuid=row.get(0);let key:String=row.get(1);
            self.incident(&format!("customer-update:{id}"),"P1",Some(id),"payout",json!({"overdue_update_for":key})).await?;
        }
        let desk = &self.cfg.desk_actor;
        let dashboard = self.dashboard(desk).await?;
        if dashboard["quote_coverage"]["denominator"]
            .as_i64()
            .unwrap_or(0)
            >= 20
            && dashboard["quote_coverage"]["rate"].as_f64().unwrap_or(0.0) < 0.9
        {
            self.incident(
                "quote-coverage",
                "P2",
                None,
                "federation",
                dashboard["quote_coverage"].clone(),
            )
            .await?;
        }
        for row in client.query("SELECT t.id FROM otc_trades t LEFT JOIN otc_ledger l ON l.trade_id=t.id AND l.kind='fee_accrued' WHERE (t.state='completed' AND (l.id IS NULL OR l.amount!=(t.terms->'fee_policy'->>'fee_units')::numeric)) OR (t.state IN ('refunded','rejected','expired') AND l.id IS NOT NULL)",&[]).await?{let id:Uuid=row.get(0);self.incident(&format!("fee:{id}"),"P2",Some(id),"fees",json!({})).await?;}
        client.execute("UPDATE otc_trades SET state='expired',updated_at=now() WHERE state='quoted' AND (terms->>'quote_by')::timestamptz<now()",&[]).await?;
        Ok(())
    }
}

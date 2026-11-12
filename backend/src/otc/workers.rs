use super::{error::Result, model::Evidence, repo, Service};
use chrono::Utc;
use serde_json::json;
impl Service {
    pub(crate) async fn tick(&self) -> Result<()> {
        if self.p2p_available() {
            if let Err(error) = self.p2p_tick().await {
                tracing::warn!(error=%error,"otc.p2p.tick_failed");
            }
        }
        if !self.cfg.enabled {
            let pending = repo::workers::open_trades(&self.pool.get().await?).await?;
            if pending == 0 {
                return Ok(());
            }
        }
        let client = self.pool.get().await?;
        repo::workers::initialize_mode(&client, self.cfg.demo).await?;
        let mode = repo::workers::mode(&client).await?;
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
            if let Err(error) = self.escrow_tick().await {
                tracing::warn!(error=%error,"otc.escrow.tick_failed");
            }
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
                    repo::workers::record_inventory(&tx, chain, &balance, &gas, &checkpoint)
                        .await?;
                    repo::workers::release_settled_debits(&tx, chain, &at).await?;
                    repo::workers::release_escrow_debits(&tx, chain, &at).await?;
                    repo::workers::record_scan(&tx, chain, &checkpoint, lag as i64).await?;
                }
                Err(error) => {
                    tracing::warn!(chain,error=%error,"otc.observer.scan_failed");
                    repo::workers::record_scan_failure(&tx, chain).await?;
                }
            }
            tx.commit().await?;
        }
        Ok(())
    }
    async fn revalidate(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows = repo::workers::revalidate(&client).await?;
        for (id, kind, raw) in rows {
            let evidence: Evidence = serde_json::from_value(raw)?;
            let invalid = if evidence.chain == "ethereum" {
                let reference = evidence.raw["receipt"]["transactionHash"]
                    .as_str()
                    .unwrap_or("");
                match self
                    .evm_agree(
                        &self.desk_evm(),
                        "eth_getTransactionReceipt",
                        json!([reference]),
                    )
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
                                    &self.desk_evm(),
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
            repo::workers::mark_evidence_checked(&client, &id, &kind).await?;
            if invalid {
                let mut client = self.pool.get().await?;
                let tx = client.transaction().await?;
                repo::workers::invalidate_evidence(&tx, &id, &kind).await?;
                repo::workers::review_trade(&tx, &id).await?;
                repo::workers::mark_attempts_unknown(
                    &tx,
                    &id,
                    if kind == "review_payment" {
                        "payment"
                    } else {
                        &kind
                    },
                )
                .await?;
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
        for id in repo::workers::overdue_payouts(&client).await? {
            self.incident(
                &format!("overdue:{id}"),
                "P1",
                Some(id),
                "payout",
                json!({}),
            )
            .await?;
        }
        for (id, attempt) in repo::workers::unknown_outgoing(&client).await? {
            self.incident(
                &format!("unknown-outgoing:{attempt}"),
                "P1",
                Some(id),
                "payout",
                json!({"attempt":attempt}),
            )
            .await?;
        }
        for chain in repo::workers::unhealthy_observers(&client).await? {
            let count = repo::workers::funded_open_trades(&client).await?;
            self.incident(
                &format!("observer:{chain}"),
                if count > 0 { "P1" } else { "P2" },
                None,
                "observer",
                json!({"chain":chain}),
            )
            .await?;
        }
        for id in repo::workers::undelivered_outbox(&client).await? {
            self.incident(
                &format!("delivery:{id}"),
                "P2",
                None,
                "federation",
                json!({"activity":id}),
            )
            .await?;
        }
        for id in repo::workers::stale_bookings(&client).await? {
            self.incident(
                &format!("booking:{id}"),
                "P2",
                Some(id),
                "federation",
                json!({}),
            )
            .await?;
        }
        for id in repo::workers::inventory_shortfalls(&client).await? {
            let funded = repo::workers::case_funded(&client, &id).await?;
            self.incident(
                &format!("inventory:{id}"),
                if funded { "P1" } else { "P2" },
                Some(id),
                "payout",
                json!({"shortfall":true}),
            )
            .await?;
        }
        for (id, key) in repo::workers::stale_updates(&client).await? {
            self.incident(
                &format!("customer-update:{id}"),
                "P1",
                Some(id),
                "payout",
                json!({"overdue_update_for":key}),
            )
            .await?;
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
        for id in repo::workers::fee_mismatches(&client).await? {
            self.incident(&format!("fee:{id}"), "P2", Some(id), "fees", json!({}))
                .await?;
        }
        repo::workers::expire_quotes(&client).await?;
        Ok(())
    }
}

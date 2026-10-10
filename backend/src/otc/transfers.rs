use super::{
    amount,
    error::{invalid, Error, Result},
    model::{Evidence, Leg},
    protocol, repo, Service,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrepareRequest {
    pub kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SubmitRequest {
    pub reference: String,
}
impl Service {
    pub(crate) async fn prepare(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        kind: &str,
    ) -> Result<Value> {
        let owned = self.owned_trade(actor, id).await?;
        {
            let client = self.pool.get().await?;
            if let Some(row)=client.query_opt("SELECT id,instructions,state FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3",&[&id,&kind,&key]).await? { let expected=if kind=="payment" {&owned.terms.customer_actor}else{&owned.terms.desk_actor}; if actor!=expected {return Err(Error::Unauthorized);} return Ok(json!({"id":row.get::<_,Uuid>(0),"instructions":row.get::<_,Value>(1),"state":row.get::<_,String>(2)})); }
        }
        let expected = if kind == "payment" {
            &owned.terms.customer_actor
        } else {
            &owned.terms.desk_actor
        };
        if actor != expected {
            return Err(Error::Unauthorized);
        }
        if self.cfg.demo || owned.terms.demo {
            return Err(invalid(
                "demo transfers use the simulation harness; real signing is disabled",
            ));
        }
        let leg = if kind == "refund" {
            self.refund_leg(id, owned.terms.leg(kind)?).await?
        } else {
            owned.terms.leg(kind)?
        };
        if self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT signing_paused FROM otc_controls WHERE singleton",
                &[],
            )
            .await?
            .get::<_, bool>(0)
        {
            return Err(Error::Disabled);
        }
        let checkpoint=self.pool.get().await?.query_one("SELECT checkpoint FROM otc_observers WHERE chain=$1 AND NOT paused AND last_scan>now()-interval '60 seconds'",&[&leg.chain]).await?.get::<_,Value>(0);
        let (nonce, call) = if leg.chain == "ethereum" {
            if super::chain::number(&self.evm_agree("eth_chainId", json!([])).await?)? != 1 {
                return Err(invalid("wrong Ethereum network"));
            }
            let nonce = self
                .evm_agree("eth_getTransactionCount", json!([leg.sender, "pending"]))
                .await?;
            let data = super::chain::ethereum::transfer_data(&leg)?;
            (
                nonce.clone(),
                json!({"family":"evm","chain_id":1,"from":leg.sender,"to":super::config::USDT,"data":data,"value":"0x0","nonce":nonce}),
            )
        } else {
            (
                Value::Null,
                json!({"family":"everscale","sender":leg.sender,"recipient":leg.recipient,"amount":leg.transfer_amount,"bounce":true}),
            )
        };
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one("SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        let trade = repo::trade(&row)?;
        if let Some(row)=tx.query_opt("SELECT id,instructions,state FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3",&[&id,&kind,&key]).await? {return Ok(json!({"id":row.get::<_,Uuid>(0),"instructions":row.get::<_,Value>(1),"state":row.get::<_,String>(2)}));}
        let valid = match kind {
            "payment" => trade.state == "accepted" && Utc::now() <= trade.terms.pay_by,
            "payout" => trade.state == "funded",
            "refund" => ["review", "funded"].contains(&trade.state.as_str()),
            _ => false,
        };
        if tx.query_opt("SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND state='failed' AND NOT retry_authorized",&[&id,&kind]).await?.is_some() {return Err(Error::Conflict("review the confirmed failure before retrying".into()));}
        if !valid {
            repo::event(
                &tx,
                Some(id),
                None,
                "blocked_transfer",
                &json!({"reason":"state_conflict"}),
            )
            .await?;
            tx.commit().await?;
            return Err(Error::Conflict(
                "transfer is unavailable in the current trade state".into(),
            ));
        }
        if kind != "payment" {
            if tx.query_opt("SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind IN ('payout','refund') AND state!='failed'",&[&id]).await?.is_some(){repo::event(&tx,Some(id),None,"blocked_transfer",&json!({"reason":"outgoing_unresolved"})).await?;tx.commit().await?;return Err(Error::Conflict("original payout/refund must be conclusively resolved".into()));}
            if tx.query_opt("SELECT identity FROM otc_evidence WHERE trade_id=$1 AND valid AND (kind='payment' OR ($2::text='refund' AND kind='review_payment'))",&[&id,&kind]).await?.is_none(){return Err(Error::Conflict("verified customer receipt required".into()));}
        }
        if tx.query_opt("SELECT id FROM otc_attempts WHERE state IN ('prepared','unknown','submitted','confirming') AND instructions->'leg'->>'chain'=$1 AND instructions->'leg'->>'sender'=$2",&[&leg.chain,&leg.sender]).await?.is_some(){repo::event(&tx,Some(id),None,"blocked_transfer",&json!({"reason":"wallet_unresolved"})).await?;tx.commit().await?;return Err(Error::Conflict("this wallet has an unresolved outgoing attempt".into()));}
        // Serialize sends across trades for the same wallet, then recheck after acquiring the lock.
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtext($1))",
            &[&format!("{}:{}", leg.chain, leg.sender)],
        )
        .await?;
        if tx.query_opt("SELECT id FROM otc_attempts WHERE state IN ('prepared','unknown','submitted','confirming') AND instructions->'leg'->>'chain'=$1 AND instructions->'leg'->>'sender'=$2",&[&leg.chain,&leg.sender]).await?.is_some(){return Err(Error::Conflict("wallet handoff already pending".into()));}
        if kind == "refund" {
            let inventory=tx.query_opt("SELECT balance::text,gas_reserve::text FROM otc_inventory WHERE chain=$1 AND observed_at>now()-interval '60 seconds' FOR UPDATE",&[&leg.chain]).await?.ok_or(Error::Disabled)?;
            let reserved:String=tx.query_one("SELECT COALESCE(sum(amount),0)::text FROM otc_reservations WHERE chain=$1 AND status!='released' AND trade_id!=$2",&[&leg.chain,&id]).await?.get(0);
            let available = amount::integer(&inventory.get::<_, String>(0))?
                .saturating_sub(amount::integer(&inventory.get::<_, String>(1))?)
                .saturating_sub(amount::integer(&reserved)?);
            if available < amount::integer(&leg.transfer_amount)? {
                return Err(Error::Conflict("refund inventory is not covered".into()));
            }
            tx.execute("UPDATE otc_reservations SET chain=$2,amount=$3::text::numeric,status='active' WHERE trade_id=$1",&[&id,&leg.chain,&leg.transfer_amount]).await?;
        }
        let attempt = Uuid::new_v4();
        let instructions = json!({"leg":leg,"call":call,"nonce":nonce,"checkpoint":checkpoint,"attempt_id":attempt,"trade_id":id,"kind":kind,"prepared_at":Utc::now()});
        tx.execute("INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,$4,'unknown',$5,$6)",&[&attempt,&id,&kind,&leg.chain,&instructions,&key]).await?;
        let next = match kind {
            "payment" => "payment_confirming",
            "payout" => "payout_pending",
            _ => "refund_pending",
        };
        tx.execute(
            "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1",
            &[&id, &next],
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "transfer_handoff",
            &json!({"attempt":attempt,"kind":kind}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"id":attempt,"state":"unknown","instructions":instructions}))
    }
    pub(crate) async fn submit(
        &self,
        actor: &str,
        attempt: Uuid,
        request: SubmitRequest,
    ) -> Result<Value> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row=tx.query_opt("SELECT a.trade_id,a.kind,a.reference,a.chain,t.customer_actor,t.desk_actor FROM otc_attempts a JOIN otc_trades t ON t.id=a.trade_id WHERE a.id=$1 FOR UPDATE OF a",&[&attempt]).await?.ok_or(Error::NotFound)?;
        let kind: String = row.get(1);
        let expected: String = row.get(if kind == "payment" { 4 } else { 5 });
        if actor != expected {
            return Err(Error::NotFound);
        }
        let existing: Option<String> = row.get(2);
        if existing.as_ref().is_some_and(|r| r != &request.reference) {
            return Err(Error::Conflict(
                "original reference is immutable; reconcile replacement history".into(),
            ));
        }
        let chain: String = row.get(3);
        let bytes = super::wallet::unhex(&request.reference)?;
        if bytes.len() != 32 || (chain == "ethereum" && !request.reference.starts_with("0x")) {
            return Err(invalid("invalid chain reference"));
        }
        tx.execute("UPDATE otc_attempts SET reference=$2,state=CASE WHEN state IN ('unknown','prepared') THEN 'submitted' ELSE state END,updated_at=now() WHERE id=$1",&[&attempt,&request.reference]).await?;
        repo::event(
            &tx,
            Some(row.get(0)),
            None,
            "transfer_reference",
            &json!({"attempt":attempt,"reference":request.reference}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"tracked":true}))
    }
    pub(crate) async fn credit(&self, id: Uuid, kind: &str, evidence: &Evidence) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one("SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        let trade = repo::trade(&row)?;
        let expected = if kind == "refund" {
            self.refund_leg(id, trade.terms.leg(kind)?).await?
        } else {
            trade.terms.leg(kind)?
        };
        if evidence.chain != expected.chain
            || evidence.leg != expected
            || !evidence.canonical
            || !evidence.successful
        {
            return Err(invalid("receipt does not verify the booked transfer"));
        }
        if !evidence.finalized {
            tx.execute("UPDATE otc_attempts SET state='confirming',updated_at=now() WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted')",&[&id,&kind]).await?;
            tx.commit().await?;
            return Ok(());
        }
        let mut restoring = false;
        if let Some(existing)=tx.query_opt("SELECT trade_id,kind,valid,evidence FROM otc_evidence WHERE chain=$1 AND identity=$2",&[&evidence.chain,&evidence.identity]).await? {
            if existing.get::<_,Uuid>(0)!=id || existing.get::<_,String>(1)!=kind {return Err(Error::Conflict("transfer evidence already credited to another leg".into()));}
            if existing.get::<_,bool>(2){return Ok(());}
            restoring=true;
            repo::event(&tx,Some(id),Some(trade.rfq_id),"evidence_reconciled",&json!({"kind":kind,"previous":existing.get::<_,Value>(3),"current":evidence})).await?;
        }
        if let Some(existing) = tx
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2",
                &[&id, &kind],
            )
            .await?
        {
            if existing.get::<_, String>(0) != evidence.identity {
                return Err(Error::Conflict(
                    "trade leg already has another credited transfer".into(),
                ));
            }
        }
        let serialized = serde_json::to_value(evidence)?;
        if restoring {
            tx.execute("UPDATE otc_evidence SET valid=TRUE,evidence=$3,included_at=$4,last_checked_at=now() WHERE trade_id=$1 AND kind=$2",&[&id,&kind,&serialized,&evidence.included_at]).await?;
        } else {
            tx.execute("INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES($1,$2,$3,$4,$5,$6)",&[&evidence.chain,&evidence.identity,&id,&kind,&serialized,&evidence.included_at]).await?;
        }
        tx.execute("UPDATE otc_attempts SET state='verified',updated_at=now() WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted','confirming')",&[&id,&kind]).await?;
        let payment=tx.query_opt("SELECT included_at FROM otc_evidence WHERE trade_id=$1 AND kind='payment' AND valid",&[&id]).await?;
        if kind != "payment" && payment.is_none() && !(kind=="refund" && tx.query_opt("SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='review_payment' AND valid",&[&id]).await?.is_some()) {
            return Err(invalid("verified original customer receipt required"));
        }
        let payout = tx
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='payout' AND valid",
                &[&id],
            )
            .await?
            .is_some();
        let refund = tx
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='refund' AND valid",
                &[&id],
            )
            .await?
            .is_some();
        let double_outgoing = payout && refund;
        let next = if double_outgoing {
            "review"
        } else if refund {
            "refunded"
        } else if payout && payment.is_some() {
            let fee = &trade.terms.fee_policy;
            let expected_fee = amount::fee(amount::integer(&fee.basis_units)?, fee.precision)?;
            if fee.rate != "0.0025"
                || fee.floor != "5"
                || amount::integer(&fee.fee_units)? != expected_fee
            {
                return Err(invalid("fee snapshot mismatch"));
            }
            tx.execute("INSERT INTO otc_ledger(id,trade_id,kind,currency,amount,policy) VALUES($1,$2,'fee_accrued','USDT',$3::text::numeric,$4) ON CONFLICT DO NOTHING",&[&Uuid::new_v4(),&id,&fee.fee_units,&serde_json::to_value(fee)?]).await?;
            let confirmation = json!({"@context":protocol::context(),"id":format!("{}/confirmations/{id}",trade.terms.desk_actor),"type":"Create","actor":trade.terms.desk_actor,"to":[trade.terms.customer_actor],"object":{"id":format!("{}/confirmations/{id}/receipt",trade.terms.desk_actor),"type":"Document","context":trade.decision.as_ref().map(|d|&d["result"]["id"]),"name":"OTC trade completed","content":"Both transfer legs have been verified."}});
            repo::enqueue(&tx, &trade.terms.desk_actor, &confirmation, "confirmation").await?;
            "completed"
        } else if payment
            .is_some_and(|r| r.get::<_, chrono::DateTime<Utc>>(0) <= trade.terms.pay_by)
        {
            "funded"
        } else {
            "review"
        };
        tx.execute(
            "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1",
            &[&id, &next],
        )
        .await?;
        if kind != "payment" {
            tx.execute(
                "UPDATE otc_reservations SET status='debit' WHERE trade_id=$1",
                &[&id],
            )
            .await?;
        }
        repo::event(
            &tx,
            Some(id),
            Some(trade.rfq_id),
            next,
            &json!({"kind":kind,"evidence":evidence.identity}),
        )
        .await?;
        tx.commit().await?;
        if double_outgoing {
            self.incident(
                &format!("double-outgoing:{id}"),
                "P0",
                Some(id),
                "integrity",
                json!({"payout_and_refund":true}),
            )
            .await?;
        }
        if next == "review" && !double_outgoing {
            self.incident(
                &format!("late-payment:{id}"),
                "P1",
                Some(id),
                "payment",
                json!({"included_at":evidence.included_at}),
            )
            .await?;
        }
        Ok(())
    }
    pub(crate) async fn transfer_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows=client.query("SELECT id,trade_id,kind,chain,instructions,reference FROM otc_attempts WHERE state IN ('unknown','submitted','confirming') ORDER BY created_at LIMIT 100",&[]).await?;
        for row in rows {
            let attempt: Uuid = row.get(0);
            let id: Uuid = row.get(1);
            let kind: String = row.get(2);
            let chain: String = row.get(3);
            let instructions: Value = row.get(4);
            let mut reference: Option<String> = row.get(5);
            let leg: Leg = serde_json::from_value(instructions["leg"].clone())?;
            if reference.is_none() {
                reference = if chain == "ethereum" {
                    self.locate_ethereum(&leg, &instructions).await?
                } else {
                    self.locate_everscale(&leg, &instructions).await?
                };
                if let Some(reference) = &reference {
                    client.execute("UPDATE otc_attempts SET reference=$2,state='submitted' WHERE id=$1 AND reference IS NULL",&[&attempt,&reference]).await?;
                }
            }
            let Some(reference) = reference else {
                continue;
            };
            if chain == "ethereum" {
                if let Some(proof) = self.failed_ethereum(&reference, &instructions).await? {
                    self.record_failure(attempt, id, &kind, &proof).await?;
                    continue;
                }
            }
            let result = if chain == "ethereum" {
                let nonce = instructions["nonce"]
                    .as_str()
                    .and_then(|n| u128::from_str_radix(n.trim_start_matches("0x"), 16).ok());
                self.ethereum_evidence(&reference, &leg, nonce).await
            } else {
                self.everscale_evidence(&reference, &leg).await
            };
            match result {
                Ok(Some(evidence)) => {
                    let prepared = chrono::DateTime::parse_from_rfc3339(
                        instructions["prepared_at"]
                            .as_str()
                            .ok_or_else(|| invalid("missing persisted handoff time"))?,
                    )
                    .map_err(|_| invalid("invalid persisted handoff time"))?;
                    if evidence.included_at.timestamp() < prepared.timestamp() {
                        self.incident(
                            &format!("old-evidence:{attempt}"),
                            "P1",
                            Some(id),
                            "payment",
                            json!({"attempt":attempt}),
                        )
                        .await?;
                        continue;
                    }
                    if evidence.finalized && !evidence.successful {
                        self.record_failure(attempt, id, &kind, &evidence.raw)
                            .await?;
                        continue;
                    }
                    if evidence.finalized {
                        let mut other_kind = if kind != "payment" {
                            "payment"
                        } else {
                            "payout"
                        };
                        let mut row=client.query_opt("SELECT evidence FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid",&[&id,&other_kind]).await?;
                        if row.is_none() && kind == "refund" {
                            other_kind = "review_payment";
                            row=client.query_opt("SELECT evidence FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid",&[&id,&other_kind]).await?;
                        }
                        if row.is_none() && kind != "payment" {
                            continue;
                        }
                        if let Some(row) = row {
                            let paid: Evidence = serde_json::from_value(row.get(0))?;
                            let still_valid = if paid.chain == "ethereum" {
                                let reference = paid.raw["receipt"]["transactionHash"]
                                    .as_str()
                                    .ok_or_else(|| invalid("missing credited payment reference"))?;
                                let current =
                                    self.ethereum_evidence(reference, &paid.leg, None).await?;
                                current.is_some_and(|e| {
                                    e.canonical
                                        && e.finalized
                                        && e.successful
                                        && e.identity == paid.identity
                                })
                            } else {
                                let reference = paid.raw["id"].as_str().ok_or_else(|| {
                                    invalid("missing credited receiving reference")
                                })?;
                                self.everscale_evidence(reference, &paid.leg)
                                    .await?
                                    .is_some_and(|e| {
                                        e.canonical
                                            && e.finalized
                                            && e.successful
                                            && e.identity == paid.identity
                                    })
                            };
                            if !still_valid {
                                self.incident(
                                    &format!("canonical:{id}:{other_kind}"),
                                    "P0",
                                    Some(id),
                                    "integrity",
                                    json!({"kind":other_kind,"evidence":paid.identity}),
                                )
                                .await?;
                                let mut client = self.pool.get().await?;
                                let tx = client.transaction().await?;
                                tx.query_one(
                                    "SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE",
                                    &[&id],
                                )
                                .await?;
                                tx.execute("UPDATE otc_evidence SET valid=FALSE WHERE trade_id=$1 AND kind=$2",&[&id,&other_kind]).await?;
                                tx.execute("UPDATE otc_attempts SET state='unknown' WHERE trade_id=$1 AND kind=$2 AND state IN ('verified','review')",&[&id,&if other_kind=="review_payment" {"payment"} else {other_kind}]).await?;
                                tx.execute("UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1",&[&id]).await?;
                                repo::event(
                                    &tx,
                                    Some(id),
                                    None,
                                    "evidence_invalidated",
                                    &json!({"kind":other_kind}),
                                )
                                .await?;
                                tx.commit().await?;
                                continue;
                            }
                        }
                    }
                    if let Err(error) = self.credit(id, &kind, &evidence).await {
                        tracing::warn!(error=%error,"otc.credit.review");
                        self.incident(
                            &format!("receipt:{attempt}"),
                            "P1",
                            Some(id),
                            "payment",
                            json!({"attempt":attempt}),
                        )
                        .await?;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    if kind == "payment" {
                        let nonce = instructions["nonce"].as_str().and_then(|n| {
                            u128::from_str_radix(n.trim_start_matches("0x"), 16).ok()
                        });
                        let reviewed = if chain == "ethereum" {
                            self.ethereum_review(&reference, &leg, nonce).await
                        } else {
                            self.everscale_review(&reference, &leg).await
                        };
                        if let Ok(Some(actual)) = reviewed {
                            let prepared = chrono::DateTime::parse_from_rfc3339(
                                instructions["prepared_at"]
                                    .as_str()
                                    .ok_or_else(|| invalid("missing handoff time"))?,
                            )
                            .map_err(|_| invalid("invalid handoff time"))?;
                            if actual.finalized
                                && actual.included_at.timestamp() >= prepared.timestamp()
                            {
                                self.review_payment(id, attempt, &actual).await?;
                                continue;
                            }
                        }
                    }
                    tracing::warn!(error=%error,"otc.receipt.review");
                    client.execute("UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1 AND state NOT IN ('completed','refunded')",&[&id]).await?;
                    self.incident(
                        &format!("receipt:{attempt}"),
                        "P1",
                        Some(id),
                        "payment",
                        json!({"attempt":attempt}),
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }
}
impl Service {
    pub(crate) async fn failed_ethereum(
        &self,
        reference: &str,
        instructions: &Value,
    ) -> Result<Option<Value>> {
        let receipt = self
            .evm_agree("eth_getTransactionReceipt", json!([reference]))
            .await?;
        if receipt.is_null() || super::chain::number(&receipt["status"]).ok() != Some(0) {
            return Ok(None);
        }
        let transaction = self
            .evm_agree("eth_getTransactionByHash", json!([reference]))
            .await?;
        let block = self
            .evm_agree(
                "eth_getBlockByNumber",
                json!([receipt["blockNumber"], false]),
            )
            .await?;
        let finalized = self
            .evm_agree("eth_getBlockByNumber", json!(["finalized", false]))
            .await?;
        let leg: Leg = serde_json::from_value(instructions["leg"].clone())?;
        if transaction["hash"] == receipt["transactionHash"]
            && receipt["blockHash"] == block["hash"]
            && super::chain::number(&receipt["blockNumber"])?
                <= super::chain::number(&finalized["number"])?
            && transaction["from"]
                .as_str()
                .is_some_and(|sender| sender.eq_ignore_ascii_case(&leg.sender))
            && transaction["to"]
                .as_str()
                .is_some_and(|to| to.eq_ignore_ascii_case(super::config::USDT))
            && transaction["input"] == super::chain::ethereum::transfer_data(&leg)?
            && transaction["nonce"] == instructions["nonce"]
        {
            Ok(Some(
                json!({"receipt":receipt,"transaction":transaction,"block":block}),
            ))
        } else {
            Ok(None)
        }
    }
    async fn record_failure(
        &self,
        attempt: Uuid,
        id: Uuid,
        kind: &str,
        proof: &Value,
    ) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        let changed=tx.execute("UPDATE otc_attempts SET state='failed',failure_evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('unknown','submitted','confirming')",&[&attempt,&proof]).await?;
        if changed > 0 {
            tx.execute(
                "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1",
                &[&id],
            )
            .await?;
            repo::event(
                &tx,
                Some(id),
                None,
                "transfer_failed",
                &json!({"attempt":attempt,"kind":kind}),
            )
            .await?;
        }
        tx.commit().await?;
        self.incident(
            &format!("failed-transfer:{attempt}"),
            if kind == "payment" { "P2" } else { "P1" },
            Some(id),
            "payout",
            json!({"attempt":attempt}),
        )
        .await
    }
    pub(crate) async fn authorize_retry(
        &self,
        actor: &str,
        attempt: Uuid,
        evidence: &Value,
    ) -> Result<Value> {
        if evidence.as_object().is_none_or(|o| o.is_empty()) {
            return Err(invalid("reviewed retry requires evidence"));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row=tx.query_opt("SELECT a.trade_id,a.kind,a.state,a.failure_evidence,t.customer_actor,t.desk_actor FROM otc_attempts a JOIN otc_trades t ON t.id=a.trade_id WHERE a.id=$1 FOR UPDATE OF a,t",&[&attempt]).await?.ok_or(Error::NotFound)?;
        let id: Uuid = row.get(0);
        let kind: String = row.get(1);
        if row.get::<_, String>(if kind == "payment" { 4 } else { 5 }) != actor {
            return Err(Error::NotFound);
        }
        if row.get::<_, String>(2) != "failed" || row.get::<_, Option<Value>>(3).is_none() {
            return Err(Error::Conflict(
                "only a conclusively failed original attempt permits a reviewed retry".into(),
            ));
        }
        tx.execute(
            "UPDATE otc_attempts SET retry_authorized=TRUE WHERE id=$1",
            &[&attempt],
        )
        .await?;
        let next = if kind == "payment" {
            "accepted"
        } else if kind == "payout" {
            "funded"
        } else {
            "review"
        };
        tx.execute(
            "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1 AND state='review'",
            &[&id, &next],
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "retry_authorized",
            &json!({"attempt":attempt,"actor":actor,"evidence":evidence}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"authorized":true}))
    }
}

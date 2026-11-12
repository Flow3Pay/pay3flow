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
        if owned.terms.escrow.is_some() {
            return self.prepare_escrow_ever(actor, &owned, key, kind).await;
        }
        {
            let client = self.pool.get().await?;
            if let Some((attempt, instructions, state)) =
                repo::transfers::attempt_by_key(&client, &id, kind, key).await?
            {
                let expected = if kind == "payment" {
                    &owned.terms.customer_actor
                } else {
                    &owned.terms.desk_actor
                };
                if actor != expected {
                    return Err(Error::Unauthorized);
                }
                return Ok(json!({"id":attempt,"instructions":instructions,"state":state}));
            }
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
        if repo::transfers::signing_paused(&self.pool.get().await?).await? {
            return Err(Error::Disabled);
        }
        let checkpoint =
            repo::transfers::live_checkpoint(&self.pool.get().await?, &leg.chain).await?;
        let (nonce, call) = if leg.chain == "ethereum" {
            if super::chain::number(
                &self
                    .evm_agree(&self.desk_evm(), "eth_chainId", json!([]))
                    .await?,
            )? != u128::from(self.cfg.ethereum_chain_id)
            {
                return Err(invalid("wrong Ethereum network"));
            }
            let nonce = self
                .evm_agree(
                    &self.desk_evm(),
                    "eth_getTransactionCount",
                    json!([leg.sender, "pending"]),
                )
                .await?;
            let data = super::chain::ethereum::transfer_data(&leg)?;
            (
                nonce.clone(),
                json!({"family":"evm","chain_id":self.cfg.ethereum_chain_id,"from":leg.sender,"to":self.cfg.usdt_contract,"data":data,"value":"0x0","nonce":nonce}),
            )
        } else {
            (
                Value::Null,
                json!({"family":"everscale","sender":leg.sender,"recipient":leg.recipient,"amount":leg.transfer_amount,"bounce":true}),
            )
        };
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = repo::transfers::lock_trade(&tx, &id).await?;
        let trade = repo::trade(&row)?;
        if let Some((attempt, instructions, state)) =
            repo::transfers::attempt_by_key(&tx, &id, kind, key).await?
        {
            return Ok(json!({"id":attempt,"instructions":instructions,"state":state}));
        }
        let valid = match kind {
            "payment" => trade.state == "accepted" && Utc::now() <= trade.terms.pay_by,
            "payout" => trade.state == "funded",
            "refund" => ["review", "funded"].contains(&trade.state.as_str()),
            _ => false,
        };
        if repo::transfers::failed_attempt(&tx, &id, kind).await? {
            return Err(Error::Conflict(
                "review the confirmed failure before retrying".into(),
            ));
        }
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
            if repo::transfers::unresolved_outgoing(&tx, &id).await? {
                repo::event(
                    &tx,
                    Some(id),
                    None,
                    "blocked_transfer",
                    &json!({"reason":"outgoing_unresolved"}),
                )
                .await?;
                tx.commit().await?;
                return Err(Error::Conflict(
                    "original payout/refund must be conclusively resolved".into(),
                ));
            }
            if !repo::transfers::has_valid_receipt(&tx, &id, kind).await? {
                return Err(Error::Conflict("verified customer receipt required".into()));
            }
        }
        if repo::transfers::open_attempt_for_wallet(&tx, &leg.chain, &leg.sender).await? {
            repo::event(
                &tx,
                Some(id),
                None,
                "blocked_transfer",
                &json!({"reason":"wallet_unresolved"}),
            )
            .await?;
            tx.commit().await?;
            return Err(Error::Conflict(
                "this wallet has an unresolved outgoing attempt".into(),
            ));
        }
        // Serialize sends across trades for the same wallet, then recheck after acquiring the lock.
        tx.execute(
            repo::booking::ACTOR_LOCK,
            &[&format!("{}:{}", leg.chain, leg.sender)],
        )
        .await?;
        if repo::transfers::open_attempt_for_wallet(&tx, &leg.chain, &leg.sender).await? {
            return Err(Error::Conflict("wallet handoff already pending".into()));
        }
        if leg.chain == "ethereum"
            && repo::transfers::unresolved_escrow_call(&tx, &leg.sender).await?
        {
            return Err(Error::Conflict(
                "this wallet has an unresolved escrow handoff".into(),
            ));
        }
        if kind == "refund" {
            let inventory = repo::transfers::lock_inventory(&tx, &leg.chain).await?;
            let reserved = repo::transfers::reserved_elsewhere(&tx, &leg.chain, &id).await?;
            let available = amount::integer(&inventory.0)?
                .saturating_sub(amount::integer(&inventory.1)?)
                .saturating_sub(amount::integer(&reserved)?);
            if available < amount::integer(&leg.transfer_amount)? {
                return Err(Error::Conflict("refund inventory is not covered".into()));
            }
            repo::transfers::reassign_reservation(&tx, &id, &leg.chain, &leg.transfer_amount)
                .await?;
        }
        let attempt = Uuid::new_v4();
        let instructions = json!({"leg":leg,"call":call,"nonce":nonce,"checkpoint":checkpoint,"attempt_id":attempt,"trade_id":id,"kind":kind,"prepared_at":Utc::now()});
        repo::transfers::insert_attempt(&tx, &attempt, &id, kind, &leg.chain, &instructions, key)
            .await?;
        let next = match kind {
            "payment" => "payment_confirming",
            "payout" => "payout_pending",
            _ => "refund_pending",
        };
        repo::transfers::set_trade_state(&tx, &id, next).await?;
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
        let (trade_id, kind, existing, chain, customer_actor, desk_actor) =
            repo::transfers::submit_context(&tx, &attempt).await?;
        let expected: String = if kind == "payment" {
            customer_actor
        } else {
            desk_actor
        };
        if actor != expected {
            return Err(Error::NotFound);
        }
        if existing.as_ref().is_some_and(|r| r != &request.reference) {
            return Err(Error::Conflict(
                "original reference is immutable; reconcile replacement history".into(),
            ));
        }
        let bytes = super::wallet::unhex(&request.reference)?;
        if bytes.len() != 32 || (chain == "ethereum" && !request.reference.starts_with("0x")) {
            return Err(invalid("invalid chain reference"));
        }
        repo::transfers::set_attempt_reference(&tx, &attempt, &request.reference).await?;
        repo::event(
            &tx,
            Some(trade_id),
            None,
            "transfer_reference",
            &json!({"attempt":attempt,"reference":request.reference}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"tracked":true}))
    }
    pub(crate) async fn credit(&self, id: Uuid, kind: &str, evidence: &Evidence) -> Result<()> {
        let terms: super::model::Terms = serde_json::from_value(
            repo::transfers::trade_terms(&self.pool.get().await?, &id).await?,
        )?;
        if terms.escrow.is_some() {
            return self.credit_escrow_ever(id, kind, evidence).await;
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = repo::transfers::lock_trade(&tx, &id).await?;
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
            repo::transfers::mark_confirming(&tx, &id, kind).await?;
            tx.commit().await?;
            return Ok(());
        }
        let mut restoring = false;
        if let Some((credited_id, credited_kind, valid, previous)) =
            repo::transfers::evidence_by_identity(&tx, &evidence.chain, &evidence.identity).await?
        {
            if credited_id != id || credited_kind != kind {
                return Err(Error::Conflict(
                    "transfer evidence already credited to another leg".into(),
                ));
            }
            if valid {
                return Ok(());
            }
            restoring = true;
            repo::event(
                &tx,
                Some(id),
                Some(trade.rfq_id),
                "evidence_reconciled",
                &json!({"kind":kind,"previous":previous,"current":evidence}),
            )
            .await?;
        }
        if let Some(identity) = repo::transfers::evidence_identity(&tx, &id, kind).await? {
            if identity != evidence.identity {
                return Err(Error::Conflict(
                    "trade leg already has another credited transfer".into(),
                ));
            }
        }
        let serialized = serde_json::to_value(evidence)?;
        if restoring {
            repo::transfers::refresh_evidence(&tx, &id, kind, &serialized, &evidence.included_at)
                .await?;
        } else {
            repo::transfers::insert_evidence(
                &tx,
                &evidence.chain,
                &evidence.identity,
                &id,
                kind,
                &serialized,
                &evidence.included_at,
            )
            .await?;
        }
        repo::transfers::verify_attempt(&tx, &id, kind).await?;
        let payment = repo::transfers::payment_included_at(&tx, &id).await?;
        if kind != "payment"
            && payment.is_none()
            && !(kind == "refund" && repo::transfers::review_payment_receipt(&tx, &id).await?)
        {
            return Err(invalid("verified original customer receipt required"));
        }
        let payout = repo::transfers::payout_receipt(&tx, &id).await?;
        let refund = repo::transfers::refund_receipt(&tx, &id).await?;
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
            repo::transfers::insert_fee_accrued(
                &tx,
                &id,
                &fee.fee_units,
                &serde_json::to_value(fee)?,
            )
            .await?;
            let confirmation = json!({"@context":protocol::context(),"id":format!("{}/confirmations/{id}",trade.terms.desk_actor),"type":"Create","actor":trade.terms.desk_actor,"to":[trade.terms.customer_actor],"object":{"id":format!("{}/confirmations/{id}/receipt",trade.terms.desk_actor),"type":"Document","context":trade.decision.as_ref().map(|d|&d["result"]["id"]),"name":"OTC trade completed","content":"Both transfer legs have been verified."}});
            repo::enqueue(&tx, &trade.terms.desk_actor, &confirmation, "confirmation").await?;
            "completed"
        } else if payment.is_some_and(|included_at| included_at <= trade.terms.pay_by) {
            "funded"
        } else {
            "review"
        };
        repo::transfers::set_trade_state(&tx, &id, next).await?;
        if kind != "payment" {
            repo::transfers::debit_reservation(&tx, &id).await?;
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
        let rows = repo::transfers::open_attempts(&client).await?;
        for (attempt, id, kind, chain, instructions, mut reference) in rows {
            let leg: Leg = serde_json::from_value(instructions["leg"].clone())?;
            if reference.is_none() {
                reference = if chain == "ethereum" {
                    self.locate_ethereum(&leg, &instructions, &self.desk_evm())
                        .await?
                } else {
                    self.locate_everscale(&leg, &instructions).await?
                };
                if let Some(reference) = &reference {
                    repo::transfers::locate_reference(&client, &attempt, reference).await?;
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
                self.ethereum_evidence(&reference, &leg, nonce, &self.desk_evm())
                    .await
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
                    if evidence.finalized && instructions["escrow"] != true {
                        let mut other_kind = if kind != "payment" {
                            "payment"
                        } else {
                            "payout"
                        };
                        let mut row =
                            repo::transfers::valid_evidence(&client, &id, other_kind).await?;
                        if row.is_none() && kind == "refund" {
                            other_kind = "review_payment";
                            row = repo::transfers::valid_evidence(&client, &id, other_kind).await?;
                        }
                        if row.is_none() && kind != "payment" {
                            continue;
                        }
                        if let Some(row) = row {
                            let paid: Evidence = serde_json::from_value(row)?;
                            let still_valid = if paid.chain == "ethereum" {
                                let reference = paid.raw["receipt"]["transactionHash"]
                                    .as_str()
                                    .ok_or_else(|| invalid("missing credited payment reference"))?;
                                let current = self
                                    .ethereum_evidence(reference, &paid.leg, None, &self.desk_evm())
                                    .await?;
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
                                repo::transfers::lock_trade_id(&tx, &id).await?;
                                repo::transfers::invalidate_evidence(&tx, &id, other_kind).await?;
                                repo::transfers::reset_attempts(
                                    &tx,
                                    &id,
                                    if other_kind == "review_payment" {
                                        "payment"
                                    } else {
                                        other_kind
                                    },
                                )
                                .await?;
                                repo::transfers::map_trade_to_review(&tx, &id).await?;
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
                    repo::transfers::guarded_map_trade_to_review(&client, &id).await?;
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
            .evm_agree(
                &self.desk_evm(),
                "eth_getTransactionReceipt",
                json!([reference]),
            )
            .await?;
        if receipt.is_null() || super::chain::number(&receipt["status"]).ok() != Some(0) {
            return Ok(None);
        }
        let transaction = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getTransactionByHash",
                json!([reference]),
            )
            .await?;
        let block = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!([receipt["blockNumber"], false]),
            )
            .await?;
        let finalized = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["finalized", false]),
            )
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
        repo::transfers::lock_trade_id(&tx, &id).await?;
        let changed = repo::transfers::fail_attempt(&tx, &attempt, proof).await?;
        if changed > 0 {
            repo::transfers::map_trade_to_review(&tx, &id).await?;
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
        let (id, kind, state, failure_evidence, customer_actor, desk_actor) =
            repo::transfers::retry_context(&tx, &attempt).await?;
        let expected: String = if kind == "payment" {
            customer_actor
        } else {
            desk_actor
        };
        if expected != actor {
            return Err(Error::NotFound);
        }
        if state != "failed" || failure_evidence.is_none() {
            return Err(Error::Conflict(
                "only a conclusively failed original attempt permits a reviewed retry".into(),
            ));
        }
        repo::transfers::authorize_retry(&tx, &attempt).await?;
        let next = if kind == "payment" {
            "accepted"
        } else if kind == "payout" {
            "funded"
        } else {
            "review"
        };
        repo::transfers::reviewed_trade(&tx, &id, next).await?;
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

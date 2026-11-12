use super::{
    abi::{self, Factory, Safe, Token, Vault},
    *,
};
use crate::otc::{
    chain,
    error::{invalid, Error, Result},
    model::Direction,
    protocol, repo, Service,
};
use alloy_primitives::{Address, B256};
use alloy_sol_types::{SolCall, SolEvent, SolValue};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

fn decode<E: SolEvent>(log: &Value) -> Result<E> {
    let topics = log["topics"]
        .as_array()
        .ok_or_else(|| invalid("missing event topics"))?
        .iter()
        .map(|v| abi::hash(v.as_str().ok_or_else(|| invalid("invalid event topic"))?))
        .collect::<Result<Vec<_>>>()?;
    let data = wallet::unhex(
        log["data"]
            .as_str()
            .ok_or_else(|| invalid("missing event data"))?,
    )?;
    E::decode_raw_log_validate(topics, &data).map_err(|_| invalid("invalid escrow event ABI"))
}
fn event_logs<'a, E: SolEvent>(receipt: &'a Value, emitter: &str) -> Vec<&'a Value> {
    receipt["logs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|log| {
            log["address"]
                .as_str()
                .is_some_and(|v| v.eq_ignore_ascii_case(emitter))
                && log["topics"][0]
                    .as_str()
                    .is_some_and(|v| v.eq_ignore_ascii_case(&format!("{:#x}", E::SIGNATURE_HASH)))
                && log["removed"] != true
        })
        .collect()
}
fn exact_transfer(receipt: &Value, from: &str, to: &str, amount: &str) -> Result<()> {
    let mut matching = 0;
    for log in event_logs::<Token::Transfer>(receipt, crate::otc::config::USDT) {
        let transfer = decode::<Token::Transfer>(log)?;
        if transfer.from == abi::addr(from)?
            && transfer.to == abi::addr(to)?
            && transfer.amount == abi::units(amount)?
        {
            matching += 1;
        }
    }
    if matching != 1 {
        return Err(invalid("exact escrow USDT transfer missing or ambiguous"));
    }
    Ok(())
}

impl Service {
    pub(super) async fn verify_escrow_factory(&self, p: &EscrowPolicy) -> Result<()> {
        p.validate()?;
        if chain::number(
            &self
                .evm_agree(&self.desk_evm(), "eth_chainId", json!([]))
                .await?,
        )? != 1
        {
            return Err(invalid("wrong escrow network"));
        }
        let block = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["finalized", false]),
            )
            .await?;
        for (address, expected) in [
            (&p.factory, &p.factory_code_hash),
            (&p.singleton, &p.singleton_code_hash),
        ] {
            let code = self
                .evm_agree(
                    &self.desk_evm(),
                    "eth_getCode",
                    json!([address, block["number"]]),
                )
                .await?;
            let bytes = wallet::unhex(
                code.as_str()
                    .ok_or_else(|| invalid("missing deployment code"))?,
            )?;
            if bytes.is_empty() || alloy_primitives::keccak256(bytes) != abi::hash(expected)? {
                return Err(invalid(
                    "escrow deployment bytecode differs from pinned configuration",
                ));
            }
        }
        for (call, expected) in [
            (Factory::tokenCall {}.abi_encode(), crate::otc::config::USDT),
            (
                Factory::arbitratorCall {}.abi_encode(),
                p.arbitrator.as_str(),
            ),
            (Factory::singletonCall {}.abi_encode(), p.singleton.as_str()),
        ] {
            let result = self.evm_call(&p.factory, call, &block["number"]).await?;
            if Address::abi_decode_validate(&result)
                .map_err(|_| invalid("invalid factory configuration"))?
                != abi::addr(expected)?
            {
                return Err(invalid("factory configuration differs from quote policy"));
            }
        }
        Ok(())
    }

    /// Return independently agreed canonical, finalized transaction evidence.
    async fn escrow_receipt(
        &self,
        reference: &str,
    ) -> Result<Option<(Value, Value, DateTime<Utc>)>> {
        let receipt = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getTransactionReceipt",
                json!([reference]),
            )
            .await?;
        if receipt.is_null() {
            return Ok(None);
        }
        let block = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!([receipt["blockNumber"], false]),
            )
            .await?;
        let final_head = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["finalized", false]),
            )
            .await?;
        if receipt["blockHash"] != block["hash"]
            || chain::number(&receipt["blockNumber"])? != chain::number(&block["number"])?
            || chain::number(&block["number"])? > chain::number(&final_head["number"])?
        {
            return Ok(None);
        }
        let transaction = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getTransactionByHash",
                json!([reference]),
            )
            .await?;
        if transaction["hash"] != receipt["transactionHash"]
            || transaction["blockHash"] != block["hash"]
            || receipt["transactionHash"].as_str() != Some(reference)
        {
            return Err(invalid("escrow receipt is not canonical"));
        }
        let seconds = i64::try_from(chain::number(&block["timestamp"])?)
            .map_err(|_| invalid("escrow block time overflow"))?;
        let at = Utc
            .timestamp_opt(seconds, 0)
            .single()
            .ok_or_else(|| invalid("invalid escrow block time"))?;
        Ok(Some((
            receipt.clone(),
            json!({"receipt":receipt,"transaction":transaction,"block":block}),
            at,
        )))
    }

    pub(crate) async fn escrow_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows = repo::escrow_observer::observed_escrows(&client).await?;
        for id in rows {
            if let Err(error) = self.observe_escrow(id).await {
                tracing::warn!(error=%error,"otc.escrow.observation_failed");
                self.incident(
                    &format!("escrow-observer:{id}"),
                    "P1",
                    Some(id),
                    "observer",
                    json!({"escrow":true}),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn observe_escrow(&self, id: Uuid) -> Result<()> {
        let client = self.pool.get().await?;
        let (deployment, policy, _, _, start, _) =
            repo::escrow_observer::observation(&client, &id).await?;
        let d: Deployment = serde_json::from_value(deployment)?;
        let p: EscrowPolicy = serde_json::from_value(policy)?;
        self.verify_escrow_factory(&p).await?;
        self.revalidate_escrow(id).await?;
        let final_head = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["finalized", false]),
            )
            .await?;
        let scanned_through: Option<String> = repo::escrow_observer::cursor(&client, &id).await?;
        let from = match scanned_through {
            Some(cursor) => format!("0x{:x}", chain::number(&json!(cursor))? + 1),
            None => start,
        };
        let scan_end =
            chain::number(&final_head["number"])?.min(chain::number(&json!(from))? + 511);
        let scan_end_hex = format!("0x{scan_end:x}");
        let caught_up = scan_end == chain::number(&final_head["number"])?;
        let logs = if chain::number(&json!(from))? > scan_end {
            json!([])
        } else {
            self.evm_agree(&self.desk_evm(), "eth_getLogs",json!([{"address":p.factory,"fromBlock":from,"toBlock":scan_end_hex,"topics":[format!("{:#x}",Factory::EscrowCreated::SIGNATURE_HASH),d.trade_id]}])).await?
        };
        for log in logs
            .as_array()
            .ok_or_else(|| invalid("missing factory logs"))?
        {
            let reference = log["transactionHash"]
                .as_str()
                .ok_or_else(|| invalid("factory transaction missing"))?;
            let Some((receipt, proof, at)) = self.escrow_receipt(reference).await? else {
                continue;
            };
            if chain::number(&receipt["status"])? != 1 {
                continue;
            }
            let created = event_logs::<Factory::EscrowCreated>(&receipt, &p.factory)
                .into_iter()
                .map(|l| decode::<Factory::EscrowCreated>(l).map(|e| (l, e)))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(_, e)| e.tradeId == abi::hash(&d.trade_id).unwrap_or(B256::ZERO))
                .collect::<Vec<_>>();
            if created.len() != 1 {
                return Err(invalid("deployment evidence is ambiguous"));
            }
            let (event_log, event) = &created[0];
            if event.termsHash != abi::hash(&d.terms_hash)?
                || event.createdAt != alloy_primitives::U256::from(at.timestamp() as u64)
                || event.expiresAt
                    != event.createdAt + alloy_primitives::U256::from(LIFETIME_SECONDS as u64)
            {
                return Err(invalid("deployed escrow differs from accepted terms"));
            }
            let vault = format!("{:#x}", event.vault);
            let safe = format!("{:#x}", event.safe);
            let (_, prior_vault) = repo::escrow_observer::deployed_addresses(&client, &id).await?;
            if prior_vault.is_some_and(|previous| previous != vault) {
                return Err(invalid(
                    "canonical deployment changed escrow address; operator review required",
                ));
            }
            let call = Factory::createEscrowCall::abi_decode_validate(&wallet::unhex(
                proof["transaction"]["input"]
                    .as_str()
                    .ok_or_else(|| invalid("missing deployment calldata"))?,
            )?)
            .map_err(|_| invalid("invalid deployment calldata"))?;
            if call.terms != abi::onchain_terms(&d)?
                || proof["transaction"]["to"]
                    .as_str()
                    .is_none_or(|v| !v.eq_ignore_ascii_case(&p.factory))
            {
                return Err(invalid(
                    "deployment parameters differ from original consent",
                ));
            }
            let (digest, _) = abi::consent(&d, &p)?;
            abi::verify_signature(digest, &abi::hex(&call.buyerSignature), &d.buyer)?;
            abi::verify_signature(digest, &abi::hex(&call.sellerSignature), &d.seller)?;
            let owners = Safe::getOwnersCall::abi_decode_returns_validate(
                &self
                    .evm_call(
                        &safe,
                        Safe::getOwnersCall {}.abi_encode(),
                        &receipt["blockNumber"],
                    )
                    .await?,
            )
            .map_err(|_| invalid("invalid Safe owners"))?;
            let threshold = Safe::getThresholdCall::abi_decode_returns(
                &self
                    .evm_call(
                        &safe,
                        Safe::getThresholdCall {}.abi_encode(),
                        &receipt["blockNumber"],
                    )
                    .await?,
            )
            .map_err(|_| invalid("invalid Safe threshold"))?;
            let mut expected = vec![
                abi::addr(&d.buyer)?,
                abi::addr(&d.seller)?,
                abi::addr(&p.arbitrator)?,
            ];
            expected.sort();
            let mut actual = owners;
            actual.sort();
            if actual != expected || threshold != alloy_primitives::U256::from(2) {
                return Err(invalid("Safe owners or threshold mismatch"));
            }
            self.record_escrow_event(id, "deployment", event_log, &proof, at)
                .await?;
            let expires = at + chrono::Duration::seconds(LIFETIME_SECONDS);
            let block_hash = receipt["blockHash"]
                .as_str()
                .ok_or_else(|| invalid("missing deployment block"))?;
            repo::escrow_observer::record_deployment(
                &client, &id, &safe, &vault, at, expires, block_hash,
            )
            .await?;
        }
        let (safe, vault) = repo::escrow_observer::deployed_addresses(&client, &id).await?;
        if let (Some(safe), Some(vault)) = (safe, vault) {
            let logs = if chain::number(&json!(from))? > chain::number(&final_head["number"])? {
                json!([])
            } else {
                self.evm_agree(&self.desk_evm(), "eth_getLogs",json!([{"address":vault,"fromBlock":from,"toBlock":scan_end_hex,"topics":[null,d.trade_id]}])).await?
            };
            for log in logs
                .as_array()
                .ok_or_else(|| invalid("missing vault logs"))?
            {
                let reference = log["transactionHash"]
                    .as_str()
                    .ok_or_else(|| invalid("vault transaction missing"))?;
                let Some((receipt, proof, at)) = self.escrow_receipt(reference).await? else {
                    continue;
                };
                if chain::number(&receipt["status"])? != 1 {
                    continue;
                }
                self.observe_vault_receipt(id, &d, (&safe, &vault), &receipt, &proof, at)
                    .await?;
            }
        } else if chain::number(&final_head["timestamp"])? > d.valid_until as u128 {
            repo::escrow_observer::expire_unsigned(&client, &id).await?;
            repo::escrow_observer::expire_trade(&client, &id).await?;
            repo::escrow_observer::release_reservation(&client, &id).await?;
        }
        self.observe_escrow_calls(id, &final_head).await?;
        repo::escrow_observer::record_progress(&client, &id, &scan_end_hex, caught_up).await?;
        self.finish_escrow(id).await
    }

    async fn observe_vault_receipt(
        &self,
        id: Uuid,
        d: &Deployment,
        addresses: (&str, &str),
        receipt: &Value,
        proof: &Value,
        at: DateTime<Utc>,
    ) -> Result<()> {
        let (safe, vault) = addresses;
        for log in event_logs::<Vault::Funded>(receipt, vault) {
            let event = decode::<Vault::Funded>(log)?;
            if event.tradeId != abi::hash(&d.trade_id)?
                || event.depositor != abi::addr(&d.buyer)?
                || event.amount != abi::units(&d.usdt_amount)?
            {
                return Err(invalid("escrow funding differs from booked deposit"));
            }
            exact_transfer(receipt, &d.buyer, vault, &d.usdt_amount)?;
            self.record_escrow_event(id, "funding", log, proof, at)
                .await?;
            let client = self.pool.get().await?;
            repo::escrow_observer::mark_funded(&client, &id).await?;
            repo::escrow_observer::debit_sell_reservation(&client, &id).await?;
        }
        for log in event_logs::<Vault::Released>(receipt, vault) {
            let event = decode::<Vault::Released>(log)?;
            if event.tradeId != abi::hash(&d.trade_id)?
                || event.recipient != abi::addr(&d.seller)?
                || event.amount != abi::units(&d.usdt_amount)?
            {
                return Err(invalid("escrow release differs from booked recipient"));
            }
            if event_logs::<Safe::ExecutionSuccess>(receipt, safe).len() != 1 {
                return Err(invalid("Safe release execution evidence missing"));
            }
            exact_transfer(receipt, vault, &d.seller, &d.usdt_amount)?;
            self.record_escrow_event(id, "release", log, proof, at)
                .await?;
            repo::escrow_observer::mark_released(&self.pool.get().await?, &id).await?;
        }
        for log in event_logs::<Vault::Refunded>(receipt, vault) {
            let event = decode::<Vault::Refunded>(log)?;
            if event.tradeId != abi::hash(&d.trade_id)?
                || event.depositor != abi::addr(&d.buyer)?
                || event.amount != abi::units(&d.usdt_amount)?
            {
                return Err(invalid("escrow refund differs from original depositor"));
            }
            if !event.expired && event_logs::<Safe::ExecutionSuccess>(receipt, safe).len() != 1 {
                return Err(invalid("Safe refund execution evidence missing"));
            }
            exact_transfer(receipt, vault, &d.buyer, &d.usdt_amount)?;
            self.record_escrow_event(id, "refund", log, proof, at)
                .await?;
            repo::escrow_observer::mark_refunded(&self.pool.get().await?, &id).await?;
        }
        for log in event_logs::<Vault::Expired>(receipt, vault) {
            let event = decode::<Vault::Expired>(log)?;
            if event.tradeId != abi::hash(&d.trade_id)? {
                return Err(invalid("escrow expiry differs from booked trade"));
            }
            self.record_escrow_event(id, "expiry", log, proof, at)
                .await?;
            repo::escrow_observer::mark_expired(&self.pool.get().await?, &id).await?;
        }
        Ok(())
    }

    async fn record_escrow_event(
        &self,
        id: Uuid,
        kind: &str,
        log: &Value,
        proof: &Value,
        at: DateTime<Utc>,
    ) -> Result<()> {
        let identity = format!(
            "{}:{}",
            proof["receipt"]["transactionHash"]
                .as_str()
                .ok_or_else(|| invalid("missing escrow transaction hash"))?,
            chain::number(&log["logIndex"])?
        );
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow_observer::lock_trade(&tx, &id).await?;
        if let Some(previous) = repo::escrow_observer::evidence_identity(&tx, &id, kind).await? {
            if previous != identity {
                return Err(Error::Conflict(
                    "escrow event already has another credited identity".into(),
                ));
            }
            repo::escrow_observer::refresh_evidence(&tx, &id, kind, proof).await?;
        } else {
            repo::escrow_observer::insert_evidence(&tx, &id, kind, &identity, proof, at).await?;
            repo::event(
                &tx,
                Some(id),
                None,
                &format!("escrow_{kind}_verified"),
                &json!({"identity":identity}),
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn revalidate_escrow(&self, id: Uuid) -> Result<()> {
        let client = self.pool.get().await?;
        let rows = repo::escrow_observer::valid_evidence(&client, &id).await?;
        for (kind, previous) in rows {
            let reference = previous["receipt"]["transactionHash"]
                .as_str()
                .ok_or_else(|| invalid("missing saved escrow receipt"))?;
            let current = self.escrow_receipt(reference).await?;
            if current.is_none_or(|(r, _, _)| {
                r["blockHash"] != previous["receipt"]["blockHash"]
                    || chain::number(&r["status"]).ok() != Some(1)
            }) {
                let mut c = self.pool.get().await?;
                let tx = c.transaction().await?;
                repo::escrow_observer::lock_trade(&tx, &id).await?;
                repo::escrow_observer::invalidate_evidence(&tx, &id, &kind).await?;
                repo::escrow_observer::review_escrow(&tx, &id).await?;
                repo::escrow_observer::review_trade(&tx, &id).await?;
                repo::event(
                    &tx,
                    Some(id),
                    None,
                    "escrow_evidence_invalidated",
                    &json!({"kind":kind}),
                )
                .await?;
                tx.commit().await?;
                self.incident(
                    &format!("escrow-canonical:{id}:{kind}"),
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

    async fn observe_escrow_calls(&self, id: Uuid, head: &Value) -> Result<()> {
        let client = self.pool.get().await?;
        let calls = repo::escrow_observer::open_calls(&client, &id).await?;
        for (call_id, instructions, mut reference, cursor) in calls {
            let start = chain::number(
                &cursor
                    .clone()
                    .map_or_else(|| instructions["checkpoint"].clone(), |s| json!(s)),
            )?;
            if reference.is_none() {
                // Search from the saved handoff checkpoint. Never infer failure from elapsed time.
                let end = chain::number(&head["number"])?;
                for height in start..=end.min(start + 31) {
                    let block = self
                        .evm_agree(
                            &self.desk_evm(),
                            "eth_getBlockByNumber",
                            json!([format!("0x{height:x}"), true]),
                        )
                        .await?;
                    for transaction in block["transactions"].as_array().into_iter().flatten() {
                        if matching_call(transaction, &instructions) {
                            reference = transaction["hash"].as_str().map(str::to_owned);
                            break;
                        }
                    }
                    if reference.is_some() {
                        break;
                    }
                }
            }
            let Some(reference) = reference else {
                let end = chain::number(&head["number"])?;
                if end >= start {
                    repo::escrow_observer::advance_call_cursor(
                        &client,
                        &call_id,
                        &format!("0x{:x}", end.min(start + 31) + 1),
                    )
                    .await?;
                }
                continue;
            };
            let Some((receipt, proof, _)) = self.escrow_receipt(&reference).await? else {
                continue;
            };
            if !matching_call(&proof["transaction"], &instructions) {
                return Err(invalid(
                    "submitted reference differs from saved escrow handoff",
                ));
            }
            let state = if chain::number(&receipt["status"])? == 1 {
                "verified"
            } else {
                "failed"
            };
            repo::escrow_observer::resolve_call(&client, &call_id, &reference, state, &proof)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn finish_escrow(&self, id: Uuid) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = repo::escrow_observer::lock_escrow_trade(&tx, &id).await?;
        let trade = repo::trade(&row)?;
        let (state, expires) = repo::escrow_observer::lock_escrow_state(&tx, &id).await?;
        let ever_kind = if trade.terms.direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        let delivered = repo::escrow_observer::valid_evidence_at(&tx, &id, ever_kind).await?;
        let funding = repo::escrow_observer::funding_at(&tx, &id).await?;
        let released = repo::escrow_observer::released(&tx, &id).await?;
        let refunded = repo::escrow_observer::refunded(&tx, &id).await?;
        let unresolved = repo::escrow_observer::has_unresolved_attempt(&tx, &id).await?;
        let timely = match (delivered, funding, expires) {
            (Some(ever), Some(funding), Some(expiry)) => ever < expiry && ever >= funding,
            _ => false,
        };
        let invalidated = repo::escrow_observer::has_invalidated_evidence(&tx, &id).await?;
        let next = if invalidated || released && refunded {
            "review"
        } else if released && timely && !unresolved {
            let fee = &trade.terms.fee_policy;
            if amount::fee(amount::integer(&fee.basis_units)?, fee.precision)?.to_string()
                != fee.fee_units
                || fee.rate != "0.0025"
                || fee.floor != "5"
            {
                return Err(invalid("escrow completion fee mismatch"));
            }
            repo::escrow_observer::insert_fee(
                &tx,
                &Uuid::new_v4(),
                &id,
                &fee.fee_units,
                &serde_json::to_value(fee)?,
            )
            .await?;
            let confirmation = json!({"@context":protocol::context(),"id":format!("{}/confirmations/{id}",trade.terms.desk_actor),"type":"Create","actor":trade.terms.desk_actor,"to":[trade.terms.customer_actor],"object":{"id":format!("{}/confirmations/{id}/receipt",trade.terms.desk_actor),"type":"Document","context":trade.decision.as_ref().map(|d|&d["result"]["id"]),"name":"OTC trade completed","content":"USDT escrow release and native EVER delivery have been verified."}});
            repo::enqueue(&tx, &trade.terms.desk_actor, &confirmation, "confirmation").await?;
            "completed"
        } else if released || refunded && (delivered.is_some() || unresolved) {
            "review"
        } else if refunded {
            "refunded"
        } else if state == "expired" && delivered.is_none() && !unresolved {
            "expired"
        } else if state == "review" || delivered.is_some() && !timely {
            "review"
        } else if funding.is_some()
            && !matches!(
                trade.state.as_str(),
                "payment_confirming" | "payout_pending"
            )
        {
            "funded"
        } else {
            trade.state.as_str()
        };
        if next != trade.state {
            repo::escrow_observer::set_trade_state(&tx, &id, next).await?;
            repo::event(
                &tx,
                Some(id),
                Some(trade.rfq_id),
                next,
                &json!({"escrow":true}),
            )
            .await?;
        }
        if matches!(next, "expired" | "refunded") {
            repo::escrow_observer::release_active_reservation(&tx, &id).await?;
        }
        tx.commit().await?;
        if next == "review" {
            self.incident(&format!("escrow-resolution:{id}"),"P1",Some(id),"payout",json!({"escrow_state":state,"ever_delivered":delivered.is_some(),"ever_unresolved":unresolved})).await?;
        } else if funding.is_some()
            && !matches!(next, "completed" | "refunded")
            && expires
                .is_some_and(|at| Utc::now() >= at - chrono::Duration::seconds(EVER_CUTOFF_SECONDS))
        {
            self.incident(
                &format!("escrow-deadline:{id}"),
                "P1",
                Some(id),
                "payout",
                json!({"expires_at":expires}),
            )
            .await?;
        }
        Ok(())
    }
}

pub(super) fn matching_call(transaction: &Value, instructions: &Value) -> bool {
    ["from", "to"].iter().all(|key| {
        transaction[*key]
            .as_str()
            .zip(instructions[*key].as_str())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
    }) && transaction["input"] == instructions["data"]
        && chain::number(&transaction["nonce"])
            .ok()
            .zip(chain::number(&instructions["nonce"]).ok())
            .is_some_and(|(a, b)| a == b)
        && chain::number(&transaction["value"]).ok() == Some(0)
}

use super::{
    abi::{self, Factory, Safe, Token, Vault},
    *,
};
use crate::otc::{
    chain,
    error::{Error, Result},
    model::{Evidence, Trade},
    repo, Service,
};
use alloy_primitives::U256;
use alloy_sol_types::SolCall;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

impl Service {
    pub(crate) async fn escrow_trade(&self, actor: &str, id: Uuid) -> Result<Trade> {
        let client = self.pool.get().await?;
        let row = repo::escrow::trade_for_participant(&client, &id, actor).await?;
        repo::trade(&row)
    }

    async fn escrow_owner(&self, actor: &str, trade: &Trade) -> Result<String> {
        let policy = trade
            .terms
            .escrow
            .as_ref()
            .ok_or_else(|| invalid("trade uses direct settlement"))?;
        let expected = if actor == trade.terms.customer_actor {
            &trade.terms.customer_usdt
        } else if actor == trade.terms.desk_actor {
            &trade.terms.desk_usdt
        } else {
            &policy.arbitrator
        };
        if self.wallet(actor, "ethereum").await? != *expected {
            return Err(Error::Unauthorized);
        }
        Ok(expected.clone())
    }

    pub(super) async fn ensure_escrow(&self, trade: &Trade) -> Result<()> {
        let Some(policy) = &trade.terms.escrow else {
            return Err(invalid("trade uses direct settlement"));
        };
        if repo::escrow::exists(&self.pool.get().await?, &trade.id).await? {
            return Ok(());
        }
        if matches!(
            trade.state.as_str(),
            "quoted" | "booking_pending" | "rejected" | "expired"
        ) {
            return Err(Error::Conflict("accepted trade required for escrow".into()));
        }
        let deployment = Deployment::from_terms(trade.id, &trade.terms)?;
        let client = self.pool.get().await?;
        let checkpoint = repo::escrow::observer_checkpoint(&client, "ethereum").await?;
        let scan_from = checkpoint["block"].as_str().unwrap_or("0x0");
        repo::escrow::insert(
            &client,
            &trade.id,
            &serde_json::to_value(deployment)?,
            &serde_json::to_value(policy)?,
            scan_from,
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn escrow_details(&self, actor: &str, id: Uuid) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        self.ensure_escrow(&trade).await?;
        let policy = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        let client = self.pool.get().await?;
        let (deployment, safe, vault, created_at, expires_at, state, observed_at) =
            repo::escrow::details(&client, &id).await?;
        let deployment: Deployment = serde_json::from_value(deployment)?;
        let (_, typed_data) = abi::consent(&deployment, policy)?;
        let consents: Vec<Value> = repo::escrow::consent_owners(&client, &id)
            .await?
            .into_iter()
            .map(|owner| json!({"owner":owner}))
            .collect();
        let proposals = repo::escrow::proposals(&client, &id).await?;
        let mut pending = Vec::new();
        for (proposal_id, action, hash, typed_data, dispute, reason, evidence) in proposals {
            let approvals: Vec<Value> = repo::escrow::approval_owners(&client, &proposal_id)
                .await?
                .into_iter()
                .map(|owner| json!({"owner":owner}))
                .collect();
            pending.push(json!({"id":proposal_id,"action":action,"hash":hash,"typed_data":typed_data,"dispute":dispute,"reason":reason,"evidence":evidence,"approvals":approvals}));
        }
        let calls: Vec<Value> = repo::escrow::calls(&client, &id)
            .await?
            .into_iter()
            .map(|(id, action, state, instructions, reference, actor, proposal_id)| {
                json!({"id":id,"action":action,"state":state,"instructions":instructions,"reference":reference,"actor":actor,"proposal_id":proposal_id})
            })
            .collect();
        let evidence: Vec<Value> = repo::escrow::evidence(&client, &id)
            .await?
            .into_iter()
            .map(|(kind, evidence, valid)| json!({"kind":kind,"evidence":evidence,"valid":valid}))
            .collect();
        Ok(
            json!({"deployment":deployment,"policy":policy,"typed_data":typed_data,"safe":safe,"vault":vault,"created_at":created_at,"expires_at":expires_at,"state":state,"observed_at":observed_at,"consents":consents,"proposals":pending,"calls":calls,"evidence":evidence,"ever_cutoff_seconds":EVER_CUTOFF_SECONDS,"lifetime_seconds":LIFETIME_SECONDS}),
        )
    }

    pub(crate) async fn escrow_consent(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        request: ConsentRequest,
    ) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        self.escrow_signing(&trade).await?;
        let owner = self.escrow_owner(actor, &trade).await?;
        let deployment = Deployment::from_terms(id, &trade.terms)?;
        if owner != deployment.buyer && owner != deployment.seller {
            return Err(Error::Unauthorized);
        }
        let policy = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        let (digest, _) = abi::consent(&deployment, policy)?;
        let signature = abi::verify_signature(digest, &request.signature, &owner)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow::lock_trade(&tx, &id).await?;
        if let Some(existing) = repo::escrow::consent_signature(&tx, &id, &owner).await? {
            if existing != signature {
                return Err(Error::Conflict("deployment consent is immutable".into()));
            }
        } else {
            if Utc::now().timestamp() > deployment.valid_until {
                return Err(invalid("deployment consent expired"));
            }
            repo::escrow::insert_consent(&tx, &id, &owner, &signature, key).await?;
            repo::event(
                &tx,
                Some(id),
                None,
                "escrow_consent",
                &json!({"owner":owner}),
            )
            .await?;
        }
        tx.commit().await?;
        self.escrow_details(actor, id).await
    }

    pub(super) async fn escrow_signing(&self, trade: &Trade) -> Result<()> {
        if self.cfg.demo || trade.terms.demo {
            return Err(invalid("real escrow signing is disabled in demo mode"));
        }
        let paused = repo::escrow::signing_paused(&self.pool.get().await?).await?;
        if paused {
            return Err(Error::Disabled);
        }
        self.ensure_escrow(trade).await?;
        let p = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        self.verify_escrow_factory(p).await
    }

    pub(super) async fn evm_call(&self, to: &str, data: Vec<u8>, block: &Value) -> Result<Vec<u8>> {
        let value = self
            .evm_agree(
                &self.desk_evm(),
                "eth_call",
                json!([{"to":to,"data":abi::hex(data)},block]),
            )
            .await?;
        wallet::unhex(
            value
                .as_str()
                .ok_or_else(|| invalid("invalid contract response"))?,
        )
    }

    pub(crate) async fn escrow_propose(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        request: ProposalRequest,
    ) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        self.escrow_signing(&trade).await?;
        self.escrow_owner(actor, &trade).await?;
        if !matches!(request.action.as_str(), "release" | "refund_early") {
            return Err(invalid("invalid escrow proposal"));
        }
        if request.dispute
            && (request.reason.trim().is_empty()
                || request.evidence.as_object().is_none_or(|v| v.is_empty()))
        {
            return Err(invalid("dispute requires a reason and evidence object"));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow::lock_trade(&tx, &id).await?;
        if let Some((action, dispute, reason, evidence)) =
            repo::escrow::proposal_by_key(&tx, &id, actor, key).await?
        {
            if action != request.action
                || dispute != request.dispute
                || reason != request.reason
                || evidence != request.evidence
            {
                return Err(Error::Conflict(
                    "proposal key reused with changed terms".into(),
                ));
            }
            tx.commit().await?;
            return self.escrow_details(actor, id).await;
        }
        let (safe, vault, expires, escrow_state) = repo::escrow::lock_escrow(&tx, &id).await?;
        let (Some(safe), Some(vault)) = (safe, vault) else {
            return Err(Error::Conflict("escrow is not deployed".into()));
        };
        if escrow_state != "funded" || Utc::now() >= expires {
            return Err(Error::Conflict(
                "escrow is not funded or has expired".into(),
            ));
        }
        let ever_kind = if trade.terms.direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        let delivered = repo::escrow::has_valid_evidence(&tx, &id, ever_kind).await?;
        let unresolved = repo::escrow::has_unresolved_attempt(&tx, &id).await?;
        if !request.dispute
            && ((request.action == "release" && !delivered)
                || (request.action == "refund_early" && (delivered || unresolved)))
        {
            return Err(Error::Conflict(
                "this settlement requires a documented dispute proposal".into(),
            ));
        }
        let latest = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["latest", false]),
            )
            .await?;
        if chain::number(&latest["timestamp"])? >= expires.timestamp() as u128 {
            return Err(invalid("escrow expired on chain"));
        }
        let nonce = Safe::nonceCall::abi_decode_returns(
            &self
                .evm_call(&safe, Safe::nonceCall {}.abi_encode(), &latest["number"])
                .await?,
        )
        .map_err(|_| invalid("invalid Safe nonce"))?;
        let state = Vault::stateCall::abi_decode_returns(
            &self
                .evm_call(&vault, Vault::stateCall {}.abi_encode(), &latest["number"])
                .await?,
        )
        .map_err(|_| invalid("invalid vault state"))?;
        if state != 1 {
            return Err(Error::Conflict("vault is no longer funded".into()));
        }
        let (hash, typed_data) = abi::safe_tx(&safe, &vault, &request.action, nonce)?;
        if repo::escrow::proposal_exists(&tx, &id, &nonce.to_string(), &request.action).await? {
            return Err(Error::Conflict("an immutable proposal already exists for this action and nonce; approve that proposal".into()));
        }
        let proposal_id = Uuid::new_v4();
        repo::escrow::insert_proposal(
            &tx,
            &proposal_id,
            &id,
            &request.action,
            &nonce.to_string(),
            &format!("{hash:#x}"),
            &typed_data,
            request.dispute,
            &request.reason,
            &request.evidence,
            actor,
            key,
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "escrow_proposed",
            &json!({"proposal":proposal_id,"action":request.action,"dispute":request.dispute}),
        )
        .await?;
        tx.commit().await?;
        self.escrow_details(actor, id).await
    }

    pub(crate) async fn escrow_approve(
        &self,
        actor: &str,
        id: Uuid,
        request: ApprovalRequest,
    ) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        self.escrow_signing(&trade).await?;
        let owner = self.escrow_owner(actor, &trade).await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow::lock_trade(&tx, &id).await?;
        let (hash, nonce, safe, expires, escrow_state) =
            repo::escrow::approval_context(&tx, &request.proposal_id, &id)
                .await?
                .ok_or(Error::NotFound)?;
        if escrow_state != "funded" || Utc::now() >= expires {
            return Err(Error::Conflict("escrow is closed or expired".into()));
        }
        let current = Safe::nonceCall::abi_decode_returns(
            &self
                .evm_call(&safe, Safe::nonceCall {}.abi_encode(), &json!("latest"))
                .await?,
        )
        .map_err(|_| invalid("invalid Safe nonce"))?;
        if current.to_string() != nonce {
            return Err(Error::Conflict(
                "proposal nonce has already been consumed".into(),
            ));
        }
        let signature = abi::verify_signature(abi::hash(&hash)?, &request.signature, &owner)?;
        if let Some(previous) =
            repo::escrow::approval_signature(&tx, &request.proposal_id, &owner).await?
        {
            if previous != signature {
                return Err(Error::Conflict("approval is immutable".into()));
            }
        } else {
            repo::escrow::insert_approval(&tx, &request.proposal_id, &owner, &signature).await?;
            repo::event(
                &tx,
                Some(id),
                None,
                "escrow_approved",
                &json!({"proposal":request.proposal_id,"owner":owner}),
            )
            .await?;
        }
        tx.commit().await?;
        self.escrow_details(actor, id).await
    }

    pub(crate) async fn escrow_prepare(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        request: CallRequest,
    ) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        // Timeout recovery deliberately does not depend on route/signing pause or factory availability.
        if request.action != "refund_expired" {
            self.escrow_signing(&trade).await?;
        } else {
            self.ensure_escrow(&trade).await?;
        }
        let owner = self.escrow_owner(actor, &trade).await?;
        let deployment = Deployment::from_terms(id, &trade.terms)?;
        let policy = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow::lock_trade(&tx, &id).await?;
        if let Some((call_id, action, proposal_id, instructions, state, reference)) =
            repo::escrow::call_by_key(&tx, &id, actor, key).await?
        {
            if action != request.action || proposal_id != request.proposal_id {
                return Err(Error::Conflict(
                    "handoff key reused with changed call".into(),
                ));
            }
            return Ok(
                json!({"id":call_id,"action":request.action,"proposal_id":request.proposal_id,"actor":actor,"instructions":instructions,"state":state,"reference":reference}),
            );
        }
        let (safe, vault, expires, state) = repo::escrow::lock_escrow(&tx, &id).await?;
        let latest = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["latest", false]),
            )
            .await?;
        let chain_time = chain::number(&latest["timestamp"])?;
        let (to, data) = match request.action.as_str() {
            "deploy" => {
                if actor != trade.terms.customer_actor && actor != trade.terms.desk_actor {
                    return Err(Error::Unauthorized);
                }
                if safe.is_some() || chain_time > deployment.valid_until as u128 {
                    return Err(Error::Conflict(
                        "deployment unavailable or consent expired".into(),
                    ));
                }
                let mut signatures = Vec::new();
                for signer in [&deployment.buyer, &deployment.seller] {
                    let signature = repo::escrow::consent_by_owner(&tx, &id, signer)
                        .await?
                        .ok_or_else(|| {
                            Error::Conflict("both traders must consent before deployment".into())
                        })?;
                    signatures.push(wallet::unhex(&signature)?);
                }
                (
                    policy.factory.clone(),
                    Factory::createEscrowCall {
                        terms: abi::onchain_terms(&deployment)?,
                        buyerSignature: signatures[0].clone().into(),
                        sellerSignature: signatures[1].clone().into(),
                    }
                    .abi_encode(),
                )
            }
            "approve" | "approve_reset" | "fund" => {
                if owner != deployment.buyer {
                    return Err(Error::Unauthorized);
                }
                let vault = vault
                    .as_ref()
                    .ok_or_else(|| Error::Conflict("escrow not deployed".into()))?;
                if state != "created" || chain_time >= expires.timestamp() as u128 {
                    return Err(Error::Conflict("funding unavailable".into()));
                }
                if request.action == "fund" {
                    let allowance = Token::allowanceCall::abi_decode_returns(
                        &self
                            .evm_call(
                                crate::otc::config::USDT,
                                Token::allowanceCall {
                                    owner: abi::addr(&owner)?,
                                    spender: abi::addr(vault)?,
                                }
                                .abi_encode(),
                                &latest["number"],
                            )
                            .await?,
                    )
                    .map_err(|_| invalid("invalid USDT allowance"))?;
                    if allowance < abi::units(&deployment.usdt_amount)? {
                        return Err(Error::Conflict(
                            "approve the exact USDT deposit before funding".into(),
                        ));
                    }
                    (vault.clone(), Vault::fundCall {}.abi_encode())
                } else {
                    (
                        crate::otc::config::USDT.into(),
                        Token::approveCall {
                            spender: abi::addr(vault)?,
                            amount: if request.action == "approve_reset" {
                                U256::ZERO
                            } else {
                                abi::units(&deployment.usdt_amount)?
                            },
                        }
                        .abi_encode(),
                    )
                }
            }
            "execute" => {
                let proposal_id = request
                    .proposal_id
                    .ok_or_else(|| invalid("proposal_id required"))?;
                let safe = safe.as_ref().ok_or(Error::NotFound)?;
                let vault = vault.as_ref().ok_or(Error::NotFound)?;
                if state != "funded" || chain_time >= expires.timestamp() as u128 {
                    return Err(Error::Conflict("settlement unavailable".into()));
                }
                let (_, nonce, typed) = repo::escrow::proposal_by_id(&tx, &proposal_id, &id)
                    .await?
                    .ok_or(Error::NotFound)?;
                let current = Safe::nonceCall::abi_decode_returns(
                    &self
                        .evm_call(safe, Safe::nonceCall {}.abi_encode(), &latest["number"])
                        .await?,
                )
                .map_err(|_| invalid("invalid Safe nonce"))?;
                if current.to_string() != nonce {
                    return Err(Error::Conflict("Safe nonce already consumed".into()));
                }
                let signatures = repo::escrow::approval_signatures(&tx, &proposal_id).await?;
                if signatures.len() < 2 {
                    return Err(Error::Conflict("two distinct owners must approve".into()));
                }
                let bytes = signatures
                    .iter()
                    .take(2)
                    .map(|signature| wallet::unhex(signature))
                    .collect::<Result<Vec<_>>>()?
                    .concat();
                (
                    safe.clone(),
                    Safe::execTransactionCall {
                        to: abi::addr(vault)?,
                        value: U256::ZERO,
                        data: wallet::unhex(
                            typed["message"]["data"]
                                .as_str()
                                .ok_or_else(|| invalid("invalid saved Safe transaction"))?,
                        )?
                        .into(),
                        operation: 0,
                        safeTxGas: U256::ZERO,
                        baseGas: U256::ZERO,
                        gasPrice: U256::ZERO,
                        gasToken: alloy_primitives::Address::ZERO,
                        refundReceiver: alloy_primitives::Address::ZERO,
                        signatures: bytes.into(),
                    }
                    .abi_encode(),
                )
            }
            "refund_expired" => {
                let vault = vault.as_ref().ok_or(Error::NotFound)?;
                if chain_time < expires.timestamp() as u128
                    || !matches!(state.as_str(), "created" | "funded" | "review")
                {
                    return Err(Error::Conflict("timeout refund is not claimable".into()));
                }
                (vault.clone(), Vault::refundExpiredCall {}.abi_encode())
            }
            _ => return Err(invalid("invalid escrow handoff action")),
        };
        repo::escrow::advisory_lock(&tx, &format!("ethereum:{owner}")).await?;
        if repo::escrow::has_unresolved_handoff(&tx, &owner).await? {
            return Err(Error::Conflict(
                "this wallet has an unresolved handoff; track the original reference".into(),
            ));
        }
        let nonce = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getTransactionCount",
                json!([owner, "pending"]),
            )
            .await?;
        let instructions = json!({"chain_id":1,"from":owner,"to":to,"data":abi::hex(data),"value":"0x0","nonce":nonce,"checkpoint":latest["number"]});
        let call_id = Uuid::new_v4();
        repo::escrow::insert_call(
            &tx,
            &call_id,
            &id,
            actor,
            &request.action,
            &request.proposal_id,
            &owner,
            &instructions,
            key,
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "escrow_handoff",
            &json!({"call":call_id,"action":request.action}),
        )
        .await?;
        tx.commit().await?;
        Ok(
            json!({"id":call_id,"action":request.action,"proposal_id":request.proposal_id,"actor":actor,"instructions":instructions,"state":"unknown","reference":null}),
        )
    }

    pub(crate) async fn escrow_reference(
        &self,
        actor: &str,
        id: Uuid,
        request: ReferenceRequest,
    ) -> Result<Value> {
        self.escrow_trade(actor, id).await?;
        if !request.reference.starts_with("0x") || wallet::unhex(&request.reference)?.len() != 32 {
            return Err(invalid("invalid Ethereum reference"));
        }
        let client = self.pool.get().await?;
        let changed = repo::escrow::set_call_reference(
            &client,
            &request.call_id,
            &id,
            actor,
            &request.reference.to_ascii_lowercase(),
        )
        .await?;
        if changed == 0 {
            return Err(Error::Conflict(
                "original reference is immutable or belongs to another participant".into(),
            ));
        }
        Ok(json!({"tracked":true}))
    }

    pub(crate) async fn prepare_escrow_ever(
        &self,
        actor: &str,
        trade: &Trade,
        key: &str,
        kind: &str,
    ) -> Result<Value> {
        let expected_kind = if trade.terms.direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        if kind != expected_kind {
            return Err(invalid("USDT escrow actions use the escrow interface"));
        }
        let expected = if kind == "payment" {
            &trade.terms.customer_actor
        } else {
            &trade.terms.desk_actor
        };
        if actor != expected {
            return Err(Error::Unauthorized);
        }
        self.escrow_signing(trade).await?;
        self.ready().await?;
        let id = trade.id;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        repo::escrow::lock_trade(&tx, &id).await?;
        if let Some((attempt, instructions, state)) =
            repo::escrow::attempt_by_key(&tx, &id, kind, key).await?
        {
            return Ok(json!({"id":attempt,"instructions":instructions,"state":state}));
        }
        let (expires, state, observed) = repo::escrow::lock_ever_escrow(&tx, &id).await?;
        if state != "funded"
            || observed.is_none_or(|at| Utc::now() - at > chrono::Duration::seconds(60))
            || Utc::now().timestamp() >= expires.timestamp() - EVER_CUTOFF_SECONDS
        {
            return Err(Error::Conflict(
                "finalized USDT funding and at least 30 minutes remaining are required".into(),
            ));
        }
        if !repo::escrow::has_funding_evidence(&tx, &id).await?
            || repo::escrow::has_valid_evidence(&tx, &id, kind).await?
        {
            return Err(Error::Conflict("EVER transfer is unavailable".into()));
        }
        if repo::escrow::has_prior_attempt(&tx, &id, kind).await? {
            return Err(Error::Conflict(
                "original EVER attempt must resolve before a reviewed retry".into(),
            ));
        }
        let vault = repo::escrow::ever_vault(&tx, &id).await?;
        let head = self
            .evm_agree(
                &self.desk_evm(),
                "eth_getBlockByNumber",
                json!(["latest", false]),
            )
            .await?;
        let chain_state = Vault::stateCall::abi_decode_returns(
            &self
                .evm_call(&vault, Vault::stateCall {}.abi_encode(), &head["number"])
                .await?,
        )
        .map_err(|_| invalid("invalid vault state"))?;
        if chain_state != 1
            || chain::number(&head["timestamp"])?
                >= (expires.timestamp() - EVER_CUTOFF_SECONDS) as u128
        {
            return Err(Error::Conflict(
                "escrow has settled or the final 30-minute window has started".into(),
            ));
        }
        let leg = trade.terms.leg(kind)?;
        repo::escrow::eversale_advisory_lock(&tx, &format!("everscale:{}", leg.sender)).await?;
        if repo::escrow::has_eversale_handoff(&tx, &leg.sender).await? {
            return Err(Error::Conflict(
                "this EVER wallet has an unresolved handoff".into(),
            ));
        }
        let checkpoint = repo::escrow::eversale_checkpoint(&tx).await?;
        let attempt = Uuid::new_v4();
        let instructions = json!({"leg":leg,"call":{"family":"everscale","sender":leg.sender,"recipient":leg.recipient,"amount":leg.transfer_amount,"bounce":true},"nonce":null,"checkpoint":checkpoint,"attempt_id":attempt,"trade_id":id,"kind":kind,"prepared_at":Utc::now(),"escrow":true,"expires_at":expires});
        repo::escrow::insert_ever_attempt(&tx, &attempt, &id, kind, &instructions, key).await?;
        repo::escrow::set_trade_state(
            &tx,
            &id,
            if kind == "payment" {
                "payment_confirming"
            } else {
                "payout_pending"
            },
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "transfer_handoff",
            &json!({"attempt":attempt,"kind":kind,"escrow":true}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"id":attempt,"state":"unknown","instructions":instructions}))
    }

    pub(crate) async fn credit_escrow_ever(
        &self,
        id: Uuid,
        kind: &str,
        evidence: &Evidence,
    ) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = repo::escrow::lock_escrow_trade(&tx, &id).await?;
        let trade = repo::trade(&row)?;
        let expected_kind = if trade.terms.direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        if kind != expected_kind
            || evidence.leg != trade.terms.leg(kind)?
            || evidence.chain != "everscale"
            || !evidence.successful
            || !evidence.canonical
        {
            return Err(invalid("receipt differs from the escrow EVER leg"));
        }
        if !evidence.finalized {
            repo::escrow::confirm_attempt(&tx, &id, kind).await?;
            tx.commit().await?;
            return Ok(());
        }
        if repo::escrow::evidence_identity(&tx, &id, kind)
            .await?
            .is_some_and(|identity| identity != evidence.identity)
        {
            return Err(Error::Conflict(
                "another EVER receipt already belongs to this trade".into(),
            ));
        }
        repo::escrow::insert_ever_evidence(
            &tx,
            &evidence.identity,
            &id,
            kind,
            &serde_json::to_value(evidence)?,
            evidence.included_at,
        )
        .await?;
        repo::escrow::verify_attempt(&tx, &id, kind).await?;
        if trade.terms.direction == Direction::Buy {
            repo::escrow::debit_reservation(&tx, &id).await?;
        }
        repo::event(
            &tx,
            Some(id),
            None,
            "escrow_ever_delivered",
            &json!({"evidence":evidence.identity}),
        )
        .await?;
        tx.commit().await?;
        self.finish_escrow(id).await
    }
}

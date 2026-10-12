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
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

impl Service {
    pub(crate) async fn escrow_trade(&self, actor: &str, id: Uuid) -> Result<Trade> {
        let client = self.pool.get().await?;
        let row = client.query_opt("SELECT * FROM otc_trades t WHERE id=$1 AND (customer_actor=$2 OR desk_actor=$2 OR (state NOT IN ('quoted','booking_pending','rejected') AND EXISTS(SELECT 1 FROM otc_login_wallets w WHERE w.actor=$2 AND w.chain='ethereum' AND w.address=t.terms->'escrow'->>'arbitrator')))", &[&id,&actor]).await?.ok_or(Error::NotFound)?;
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
        if self
            .pool
            .get()
            .await?
            .query_opt(
                "SELECT trade_id FROM otc_escrows WHERE trade_id=$1",
                &[&trade.id],
            )
            .await?
            .is_some()
        {
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
        let checkpoint: Value = client
            .query_one(
                "SELECT checkpoint FROM otc_observers WHERE chain='ethereum'",
                &[],
            )
            .await?
            .get(0);
        let scan_from = checkpoint["block"].as_str().unwrap_or("0x0");
        client.execute("INSERT INTO otc_escrows(trade_id,deployment,policy,scan_from) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING", &[&trade.id,&serde_json::to_value(deployment)?,&serde_json::to_value(policy)?,&scan_from]).await?;
        Ok(())
    }

    pub(crate) async fn escrow_details(&self, actor: &str, id: Uuid) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        self.ensure_escrow(&trade).await?;
        let policy = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        let client = self.pool.get().await?;
        let row = client.query_one("SELECT deployment,safe,vault,deployed_at,expires_at,state,observed_at FROM otc_escrows WHERE trade_id=$1", &[&id]).await?;
        let deployment: Deployment = serde_json::from_value(row.get(0))?;
        let (_, typed_data) = abi::consent(&deployment, policy)?;
        let consents: Vec<Value> = client
            .query(
                "SELECT owner FROM otc_escrow_consents WHERE trade_id=$1 ORDER BY owner",
                &[&id],
            )
            .await?
            .iter()
            .map(|r| json!({"owner":r.get::<_,String>(0)}))
            .collect();
        let proposals = client.query("SELECT id,action,hash,typed_data,dispute,reason,evidence FROM otc_escrow_proposals WHERE trade_id=$1 ORDER BY created_at", &[&id]).await?;
        let mut pending = Vec::new();
        for proposal in proposals {
            let proposal_id: Uuid = proposal.get(0);
            let approvals: Vec<Value> = client
                .query(
                    "SELECT owner FROM otc_escrow_approvals WHERE proposal_id=$1 ORDER BY owner",
                    &[&proposal_id],
                )
                .await?
                .iter()
                .map(|r| json!({"owner":r.get::<_,String>(0)}))
                .collect();
            pending.push(json!({"id":proposal_id,"action":proposal.get::<_,String>(1),"hash":proposal.get::<_,String>(2),"typed_data":proposal.get::<_,Value>(3),"dispute":proposal.get::<_,bool>(4),"reason":proposal.get::<_,String>(5),"evidence":proposal.get::<_,Value>(6),"approvals":approvals}));
        }
        let calls: Vec<Value> = client.query("SELECT id,action,state,instructions,reference,actor,proposal_id FROM otc_escrow_calls WHERE trade_id=$1 ORDER BY created_at", &[&id]).await?.iter().map(|r|json!({"id":r.get::<_,Uuid>(0),"action":r.get::<_,String>(1),"state":r.get::<_,String>(2),"instructions":r.get::<_,Value>(3),"reference":r.get::<_,Option<String>>(4),"actor":r.get::<_,String>(5),"proposal_id":r.get::<_,Option<Uuid>>(6)})).collect();
        let evidence: Vec<Value> = client.query("SELECT kind,evidence,valid FROM otc_escrow_evidence WHERE trade_id=$1 ORDER BY included_at", &[&id]).await?.iter().map(|r|json!({"kind":r.get::<_,String>(0),"evidence":r.get::<_,Value>(1),"valid":r.get::<_,bool>(2)})).collect();
        Ok(
            json!({"deployment":deployment,"policy":policy,"typed_data":typed_data,"safe":row.get::<_,Option<String>>(1),"vault":row.get::<_,Option<String>>(2),"created_at":row.get::<_,Option<DateTime<Utc>>>(3),"expires_at":row.get::<_,Option<DateTime<Utc>>>(4),"state":row.get::<_,String>(5),"observed_at":row.get::<_,Option<DateTime<Utc>>>(6),"consents":consents,"proposals":pending,"calls":calls,"evidence":evidence,"ever_cutoff_seconds":EVER_CUTOFF_SECONDS,"lifetime_seconds":LIFETIME_SECONDS}),
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
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        if let Some(row) = tx
            .query_opt(
                "SELECT signature FROM otc_escrow_consents WHERE trade_id=$1 AND owner=$2",
                &[&id, &owner],
            )
            .await?
        {
            if row.get::<_, String>(0) != signature {
                return Err(Error::Conflict("deployment consent is immutable".into()));
            }
        } else {
            if Utc::now().timestamp() > deployment.valid_until {
                return Err(invalid("deployment consent expired"));
            }
            tx.execute("INSERT INTO otc_escrow_consents(trade_id,owner,signature,idempotency_key) VALUES($1,$2,$3,$4)", &[&id,&owner,&signature,&key]).await?;
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
        let paused: bool = self
            .pool
            .get()
            .await?
            .query_one(
                "SELECT signing_paused FROM otc_controls WHERE singleton",
                &[],
            )
            .await?
            .get(0);
        if paused {
            return Err(Error::Disabled);
        }
        self.ensure_escrow(trade).await?;
        let p = trade.terms.escrow.as_ref().ok_or(Error::NotFound)?;
        self.verify_escrow_factory(p).await
    }

    pub(super) async fn evm_call(&self, to: &str, data: Vec<u8>, block: &Value) -> Result<Vec<u8>> {
        let value = self
            .evm_agree("eth_call", json!([{"to":to,"data":abi::hex(data)},block]))
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
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        if let Some(row) = tx.query_opt("SELECT action,dispute,reason,evidence FROM otc_escrow_proposals WHERE trade_id=$1 AND author=$2 AND idempotency_key=$3", &[&id,&actor,&key]).await? {
            if row.get::<_,String>(0)!=request.action || row.get::<_,bool>(1)!=request.dispute || row.get::<_,String>(2)!=request.reason || row.get::<_,Value>(3)!=request.evidence { return Err(Error::Conflict("proposal key reused with changed terms".into())); }
            tx.commit().await?; return self.escrow_details(actor,id).await;
        }
        let row = tx
            .query_one(
                "SELECT safe,vault,expires_at,state FROM otc_escrows WHERE trade_id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let safe: Option<String> = row.get(0);
        let vault: Option<String> = row.get(1);
        let (Some(safe), Some(vault)) = (safe, vault) else {
            return Err(Error::Conflict("escrow is not deployed".into()));
        };
        let expires: DateTime<Utc> = row.get(2);
        if row.get::<_, String>(3) != "funded" || Utc::now() >= expires {
            return Err(Error::Conflict(
                "escrow is not funded or has expired".into(),
            ));
        }
        let ever_kind = if trade.terms.direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        let delivered = tx
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid",
                &[&id, &ever_kind],
            )
            .await?
            .is_some();
        let unresolved = tx.query_opt("SELECT id FROM otc_attempts WHERE trade_id=$1 AND state IN ('unknown','submitted','confirming','review')", &[&id]).await?.is_some();
        if !request.dispute
            && ((request.action == "release" && !delivered)
                || (request.action == "refund_early" && (delivered || unresolved)))
        {
            return Err(Error::Conflict(
                "this settlement requires a documented dispute proposal".into(),
            ));
        }
        let latest = self
            .evm_agree("eth_getBlockByNumber", json!(["latest", false]))
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
        if tx
            .query_opt(
                "SELECT id FROM otc_escrow_proposals WHERE trade_id=$1 AND nonce=$2 AND action=$3",
                &[&id, &nonce.to_string(), &request.action],
            )
            .await?
            .is_none()
        {
            let proposal_id = Uuid::new_v4();
            tx.execute("INSERT INTO otc_escrow_proposals(id,trade_id,action,nonce,hash,typed_data,dispute,reason,evidence,author,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", &[&proposal_id,&id,&request.action,&nonce.to_string(),&format!("{hash:#x}"),&typed_data,&request.dispute,&request.reason,&request.evidence,&actor,&key]).await?;
            repo::event(
                &tx,
                Some(id),
                None,
                "escrow_proposed",
                &json!({"proposal":proposal_id,"action":request.action,"dispute":request.dispute}),
            )
            .await?;
        } else {
            return Err(Error::Conflict("an immutable proposal already exists for this action and nonce; approve that proposal".into()));
        }
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
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        let row=tx.query_opt("SELECT p.hash,p.nonce,e.safe,e.expires_at,e.state FROM otc_escrow_proposals p JOIN otc_escrows e ON e.trade_id=p.trade_id WHERE p.id=$1 AND p.trade_id=$2", &[&request.proposal_id,&id]).await?.ok_or(Error::NotFound)?;
        if row.get::<_, String>(4) != "funded" || Utc::now() >= row.get::<_, DateTime<Utc>>(3) {
            return Err(Error::Conflict("escrow is closed or expired".into()));
        }
        let safe: String = row.get(2);
        let current = Safe::nonceCall::abi_decode_returns(
            &self
                .evm_call(&safe, Safe::nonceCall {}.abi_encode(), &json!("latest"))
                .await?,
        )
        .map_err(|_| invalid("invalid Safe nonce"))?;
        if current.to_string() != row.get::<_, String>(1) {
            return Err(Error::Conflict(
                "proposal nonce has already been consumed".into(),
            ));
        }
        let signature = abi::verify_signature(
            abi::hash(&row.get::<_, String>(0))?,
            &request.signature,
            &owner,
        )?;
        if let Some(previous) = tx
            .query_opt(
                "SELECT signature FROM otc_escrow_approvals WHERE proposal_id=$1 AND owner=$2",
                &[&request.proposal_id, &owner],
            )
            .await?
        {
            if previous.get::<_, String>(0) != signature {
                return Err(Error::Conflict("approval is immutable".into()));
            }
        } else {
            tx.execute(
                "INSERT INTO otc_escrow_approvals(proposal_id,owner,signature) VALUES($1,$2,$3)",
                &[&request.proposal_id, &owner, &signature],
            )
            .await?;
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
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        if let Some(row)=tx.query_opt("SELECT id,action,proposal_id,instructions,state,reference FROM otc_escrow_calls WHERE trade_id=$1 AND actor=$2 AND idempotency_key=$3", &[&id,&actor,&key]).await? {
            if row.get::<_,String>(1)!=request.action || row.get::<_,Option<Uuid>>(2)!=request.proposal_id {return Err(Error::Conflict("handoff key reused with changed call".into()));}
            return Ok(json!({"id":row.get::<_,Uuid>(0),"action":request.action,"proposal_id":request.proposal_id,"actor":actor,"instructions":row.get::<_,Value>(3),"state":row.get::<_,String>(4),"reference":row.get::<_,Option<String>>(5)}));
        }
        let row = tx
            .query_one(
                "SELECT safe,vault,expires_at,state FROM otc_escrows WHERE trade_id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let safe: Option<String> = row.get(0);
        let vault: Option<String> = row.get(1);
        let state: String = row.get(3);
        let latest = self
            .evm_agree("eth_getBlockByNumber", json!(["latest", false]))
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
                    let r=tx.query_opt("SELECT signature FROM otc_escrow_consents WHERE trade_id=$1 AND owner=$2", &[&id,&signer]).await?.ok_or_else(||Error::Conflict("both traders must consent before deployment".into()))?;
                    signatures.push(wallet::unhex(&r.get::<_, String>(0))?);
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
                let expires: DateTime<Utc> = row.get(2);
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
                let expires: DateTime<Utc> = row.get(2);
                if state != "funded" || chain_time >= expires.timestamp() as u128 {
                    return Err(Error::Conflict("settlement unavailable".into()));
                }
                let p=tx.query_opt("SELECT action,nonce,typed_data FROM otc_escrow_proposals WHERE id=$1 AND trade_id=$2", &[&proposal_id,&id]).await?.ok_or(Error::NotFound)?;
                let current = Safe::nonceCall::abi_decode_returns(
                    &self
                        .evm_call(safe, Safe::nonceCall {}.abi_encode(), &latest["number"])
                        .await?,
                )
                .map_err(|_| invalid("invalid Safe nonce"))?;
                if current.to_string() != p.get::<_, String>(1) {
                    return Err(Error::Conflict("Safe nonce already consumed".into()));
                }
                let signatures=tx.query("SELECT signature FROM otc_escrow_approvals WHERE proposal_id=$1 ORDER BY owner", &[&proposal_id]).await?;
                if signatures.len() < 2 {
                    return Err(Error::Conflict("two distinct owners must approve".into()));
                }
                let bytes = signatures
                    .iter()
                    .take(2)
                    .map(|r| wallet::unhex(&r.get::<_, String>(0)))
                    .collect::<Result<Vec<_>>>()?
                    .concat();
                let typed: Value = p.get(2);
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
                let expires: DateTime<Utc> = row.get(2);
                if chain_time < expires.timestamp() as u128
                    || !matches!(state.as_str(), "created" | "funded" | "review")
                {
                    return Err(Error::Conflict("timeout refund is not claimable".into()));
                }
                (vault.clone(), Vault::refundExpiredCall {}.abi_encode())
            }
            _ => return Err(invalid("invalid escrow handoff action")),
        };
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtext($1))",
            &[&format!("ethereum:{owner}")],
        )
        .await?;
        if tx.query_opt("SELECT id FROM otc_escrow_calls WHERE sender=$1 AND state IN ('unknown','submitted') UNION ALL SELECT id FROM otc_attempts WHERE instructions->'leg'->>'chain'='ethereum' AND instructions->'leg'->>'sender'=$1 AND state IN ('prepared','unknown','submitted','confirming') LIMIT 1", &[&owner]).await?.is_some() {return Err(Error::Conflict("this wallet has an unresolved handoff; track the original reference".into()));}
        let nonce = self
            .evm_agree("eth_getTransactionCount", json!([owner, "pending"]))
            .await?;
        let instructions = json!({"chain_id":1,"from":owner,"to":to,"data":abi::hex(data),"value":"0x0","nonce":nonce,"checkpoint":latest["number"]});
        let call_id = Uuid::new_v4();
        tx.execute("INSERT INTO otc_escrow_calls(id,trade_id,actor,action,proposal_id,sender,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7,$8)", &[&call_id,&id,&actor,&request.action,&request.proposal_id,&owner,&instructions,&key]).await?;
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
        let changed=client.execute("UPDATE otc_escrow_calls SET reference=$4,state=CASE WHEN state='unknown' THEN 'submitted' ELSE state END WHERE id=$1 AND trade_id=$2 AND actor=$3 AND (reference IS NULL OR reference=$4)", &[&request.call_id,&id,&actor,&request.reference.to_ascii_lowercase()]).await?;
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
        tx.query_one("SELECT id FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        if let Some(row)=tx.query_opt("SELECT id,instructions,state FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3", &[&id,&kind,&key]).await? {return Ok(json!({"id":row.get::<_,Uuid>(0),"instructions":row.get::<_,Value>(1),"state":row.get::<_,String>(2)}));}
        let row = tx
            .query_one(
                "SELECT expires_at,state,observed_at FROM otc_escrows WHERE trade_id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let expires: DateTime<Utc> = row.get(0);
        let observed: Option<DateTime<Utc>> = row.get(2);
        if row.get::<_, String>(1) != "funded"
            || observed.is_none_or(|at| Utc::now() - at > chrono::Duration::seconds(60))
            || Utc::now().timestamp() >= expires.timestamp() - EVER_CUTOFF_SECONDS
        {
            return Err(Error::Conflict(
                "finalized USDT funding and at least 30 minutes remaining are required".into(),
            ));
        }
        if tx.query_opt("SELECT identity FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='funding' AND valid", &[&id]).await?.is_none() || tx.query_opt("SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2 AND valid", &[&id,&kind]).await?.is_some() {return Err(Error::Conflict("EVER transfer is unavailable".into()));}
        if tx.query_opt("SELECT id FROM otc_attempts WHERE trade_id=$1 AND kind=$2 AND (state!='failed' OR NOT retry_authorized)", &[&id,&kind]).await?.is_some() {return Err(Error::Conflict("original EVER attempt must resolve before a reviewed retry".into()));}
        let row = tx
            .query_one("SELECT vault FROM otc_escrows WHERE trade_id=$1", &[&id])
            .await?;
        let vault: String = row.get(0);
        let head = self
            .evm_agree("eth_getBlockByNumber", json!(["latest", false]))
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
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtext($1))",
            &[&format!("everscale:{}", leg.sender)],
        )
        .await?;
        if tx.query_opt("SELECT id FROM otc_attempts WHERE instructions->'leg'->>'chain'='everscale' AND instructions->'leg'->>'sender'=$1 AND state IN ('unknown','submitted','confirming')", &[&leg.sender]).await?.is_some() {return Err(Error::Conflict("this EVER wallet has an unresolved handoff".into()));}
        let checkpoint:Value=tx.query_one("SELECT checkpoint FROM otc_observers WHERE chain='everscale' AND NOT paused AND last_scan>now()-interval '60 seconds'", &[]).await?.get(0);
        let attempt = Uuid::new_v4();
        let instructions = json!({"leg":leg,"call":{"family":"everscale","sender":leg.sender,"recipient":leg.recipient,"amount":leg.transfer_amount,"bounce":true},"nonce":null,"checkpoint":checkpoint,"attempt_id":attempt,"trade_id":id,"kind":kind,"prepared_at":Utc::now(),"escrow":true,"expires_at":expires});
        tx.execute("INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,'everscale','unknown',$4,$5)", &[&attempt,&id,&kind,&instructions,&key]).await?;
        tx.execute(
            "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1",
            &[
                &id,
                &if kind == "payment" {
                    "payment_confirming"
                } else {
                    "payout_pending"
                },
            ],
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
        let row = tx
            .query_one("SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
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
            tx.execute("UPDATE otc_attempts SET state='confirming' WHERE trade_id=$1 AND kind=$2 AND state IN ('unknown','submitted')", &[&id,&kind]).await?;
            tx.commit().await?;
            return Ok(());
        }
        let prior = tx
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind=$2",
                &[&id, &kind],
            )
            .await?;
        if prior.is_some_and(|r| r.get::<_, String>(0) != evidence.identity) {
            return Err(Error::Conflict(
                "another EVER receipt already belongs to this trade".into(),
            ));
        }
        tx.execute("INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES('everscale',$1,$2,$3,$4,$5) ON CONFLICT(trade_id,kind) DO UPDATE SET valid=TRUE,evidence=$4,last_checked_at=now()", &[&evidence.identity,&id,&kind,&serde_json::to_value(evidence)?,&evidence.included_at]).await?;
        tx.execute(
            "UPDATE otc_attempts SET state='verified' WHERE trade_id=$1 AND kind=$2",
            &[&id, &kind],
        )
        .await?;
        if trade.terms.direction == Direction::Buy {
            tx.execute(
                "UPDATE otc_reservations SET status='debit' WHERE trade_id=$1",
                &[&id],
            )
            .await?;
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

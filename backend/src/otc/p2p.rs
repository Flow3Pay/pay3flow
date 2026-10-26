//! Direct peer-to-peer settlement of public offers.
//! Two independent handoff-signed transfers (USDT first, then EVER); deliberately
//! bypasses the desk/RFQ, inventory and federation machinery.
use super::{
    amount,
    error::{invalid, Error, Result},
    model::{Direction, Evidence, Leg},
    wallet, Service,
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DealPrepareRequest {
    pub kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DealSubmitRequest {
    pub reference: String,
}

impl Service {
    pub(crate) fn p2p_available(&self) -> bool {
        self.cfg.p2p_enabled
            && self.public_offers_available()
            && self.wallet_login_chain_available("ethereum")
            && self.cfg.p2p_rpc().len() >= 2
    }
    /// Reads always follow the network the deal was booked on. If an operator
    /// repoints the peer contour mid-deal, the chain check fails and the attempt
    /// stays unresolved instead of confirming a transfer on another network.
    fn p2p_net(&self, body: &Value) -> Result<super::chain::EvmNet> {
        let chain_id = body["ethereum_chain_id"]
            .as_u64()
            .ok_or_else(|| invalid("deal network is missing"))?;
        let usdt = body["usdt_contract"]
            .as_str()
            .ok_or_else(|| invalid("deal token is missing"))?
            .to_string();
        Ok(super::chain::EvmNet {
            rpcs: self.cfg.p2p_rpc().to_vec(),
            chain_id,
            usdt,
        })
    }
    pub(crate) async fn accept_offer(
        &self,
        actor: &str,
        offer_id: Uuid,
        key: &str,
    ) -> Result<Value> {
        if !self.p2p_available() {
            return Err(Error::Disabled);
        }
        let taker_eth = self.wallet(actor, "ethereum").await?;
        let taker_ever = self.wallet(actor, "everscale").await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("otc-p2p-offer:{offer_id}")],
        )
        .await?;
        if let Some(row) = tx
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE taker_actor=$1 AND idempotency_key=$2",
                &[&actor, &key],
            )
            .await?
        {
            if row.get::<_, Uuid>("offer_id") != offer_id {
                return Err(Error::Conflict(
                    "acceptance key reused for another offer".into(),
                ));
            }
            let result = deal(&row, actor);
            tx.commit().await?;
            return Ok(result);
        }
        let offer = tx
            .query_opt(
                "SELECT * FROM otc_public_offers WHERE id=$1 FOR UPDATE",
                &[&offer_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let owner: String = offer.get("actor");
        if owner == actor {
            return Err(Error::Conflict("cannot accept your own offer".into()));
        }
        if offer.get::<_, String>("state") != "open"
            || offer.get::<_, DateTime<Utc>>("expires_at") <= Utc::now()
        {
            return Err(Error::Conflict("offer is no longer open".into()));
        }
        let terms: Value = offer.get("body");
        let seller_ever: String = offer.get("seller_ever");
        let usdt_address = terms["usdt_address"]
            .as_str()
            .ok_or_else(|| invalid("offer terms are malformed"))?
            .to_string();
        let direction: Direction = serde_json::from_value(terms["direction"].clone())?;
        if terms["ethereum_chain_id"] != json!(self.cfg.p2p_chain())
            || terms["usdt_contract"] != json!(self.cfg.p2p_usdt())
        {
            return Err(Error::Conflict(
                "offer was published for a different Ethereum network".into(),
            ));
        }
        let (usdt_payer, usdt_payee, ever_payer, ever_payee) = match direction {
            Direction::Sell => (
                taker_eth,
                usdt_address.clone(),
                seller_ever.clone(),
                taker_ever,
            ),
            Direction::Buy => (usdt_address.clone(), taker_eth, taker_ever, seller_ever),
        };
        if usdt_payer == usdt_payee || ever_payer == ever_payee {
            return Err(Error::Conflict(
                "offer uses the same wallet as your verified accounts".into(),
            ));
        }
        let pay_by = (Utc::now() + Duration::seconds(i64::from(self.cfg.payment_seconds)))
            .min(offer.get::<_, DateTime<Utc>>("expires_at"));
        let body = json!({
            "direction": direction,
            "ever_amount": terms["ever_amount"], "usdt_amount": terms["usdt_amount"],
            "ever_units": terms["ever_units"], "usdt_units": terms["usdt_units"],
            "usdt_payer": usdt_payer, "usdt_payee": usdt_payee,
            "ever_payer": ever_payer, "ever_payee": ever_payee,
            "usdt_contract": self.cfg.p2p_usdt(),
            "ethereum_chain_id": self.cfg.p2p_chain(),
            "ever_network_id": 42,
        });
        let id = Uuid::new_v4();
        tx.execute(
            "UPDATE otc_public_offers SET state='taken' WHERE id=$1",
            &[&offer_id],
        )
        .await?;
        tx.execute(
            "INSERT INTO otc_p2p_trades(id,offer_id,owner_actor,taker_actor,body,state,idempotency_key,pay_by) VALUES($1,$2,$3,$4,$5,'payment_pending',$6,$7)",
            &[&id, &offer_id, &owner, &actor, &body, &key, &pay_by],
        )
        .await?;
        tracing::info!(trade=%id, offer=%offer_id, taker=actor, "otc.p2p.accepted");
        let row = tx
            .query_one("SELECT * FROM otc_p2p_trades WHERE id=$1", &[&id])
            .await?;
        let result = deal(&row, actor);
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn p2p_trades(&self, actor: &str) -> Result<Value> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT * FROM otc_p2p_trades WHERE owner_actor=$1 OR taker_actor=$1 ORDER BY created_at DESC LIMIT 50",
                &[&actor],
            )
            .await?;
        let ids = rows
            .iter()
            .map(|row| row.get::<_, Uuid>("id"))
            .collect::<Vec<_>>();
        let attempts = self.p2p_attempts(&ids).await?;
        let deals = rows
            .iter()
            .map(|row| {
                let mut deal = deal(row, actor);
                deal["attempts"] = json!(attempts
                    .iter()
                    .filter(|(trade, _)| *trade == row.get::<_, Uuid>("id"))
                    .map(|(_, attempt)| attempt.clone())
                    .collect::<Vec<_>>());
                deal
            })
            .collect::<Vec<_>>();
        Ok(json!({"deals": deals}))
    }
    pub(crate) async fn p2p_deal(&self, actor: &str, id: Uuid) -> Result<Value> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2)",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let attempts = self.p2p_attempts(&[id]).await?;
        Ok(
            json!({"deal": deal(&row, actor), "attempts": attempts.into_iter().map(|(_, attempt)| attempt).collect::<Vec<_>>()}),
        )
    }
    async fn p2p_attempts(&self, ids: &[Uuid]) -> Result<Vec<(Uuid, Value)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id,trade_id,kind,state,instructions,reference FROM otc_p2p_attempts WHERE trade_id = ANY($1) AND state!='failed' ORDER BY created_at",
                &[&ids],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get::<_, Uuid>("trade_id"),
                    json!({"id": row.get::<_, Uuid>("id"), "kind": row.get::<_, String>("kind"),
                        "state": row.get::<_, String>("state"),
                        "instructions": row.get::<_, Value>("instructions"),
                        "reference": row.get::<_, Option<String>>("reference")}),
                )
            })
            .collect())
    }
    fn deal_leg(&self, body: &Value, kind: &str) -> Result<Leg> {
        let text = |key: &str| body[key].as_str().unwrap_or_default().to_string();
        let (chain, amount, sender, recipient) = match kind {
            "payment" => (
                "ethereum",
                text("usdt_units"),
                text("usdt_payer"),
                text("usdt_payee"),
            ),
            "payout" => (
                "everscale",
                text("ever_units"),
                text("ever_payer"),
                text("ever_payee"),
            ),
            _ => return Err(invalid("invalid transfer kind")),
        };
        if sender.is_empty() || recipient.is_empty() || sender == recipient {
            return Err(invalid("deal legs require distinct wallets"));
        }
        let transfer_amount = if chain == "everscale" {
            amount::integer(&amount)?
                .checked_add(amount::units(&self.cfg.p2p_ever_fee_reserve, 9)?)
                .ok_or_else(|| invalid("EVER network cost overflow"))?
                .to_string()
        } else {
            amount.clone()
        };
        Ok(Leg {
            chain: chain.into(),
            amount,
            transfer_amount,
            sender,
            recipient,
        })
    }
    pub(crate) async fn p2p_prepare(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        kind: &str,
    ) -> Result<Value> {
        if !self.p2p_available() {
            return Err(Error::Disabled);
        }
        if !["payment", "payout"].contains(&kind) {
            return Err(invalid("invalid transfer kind"));
        }
        // Wallet freshness proves the signing party controls the booked sender address.
        let verified = self
            .wallet(
                actor,
                if kind == "payment" {
                    "ethereum"
                } else {
                    "everscale"
                },
            )
            .await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(existing) = tx
            .query_opt(
                "SELECT id,instructions,state FROM otc_p2p_attempts WHERE trade_id=$1 AND kind=$2 AND idempotency_key=$3",
                &[&id, &kind, &key],
            )
            .await?
        {
            let row = tx
                .query_opt(
                    "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2)",
                    &[&id, &actor],
                )
                .await?
                .ok_or(Error::NotFound)?;
            let mut result = deal(&row, actor);
            result["attempt"] = json!({
                "id": existing.get::<_, Uuid>(0),
                "state": existing.get::<_, String>(2),
                "instructions": existing.get::<_, Value>(1),
            });
            tx.commit().await?;
            return Ok(result);
        }
        let row = tx
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2) FOR UPDATE",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let state: String = row.get("state");
        let body: Value = row.get("body");
        let pay_by: DateTime<Utc> = row.get("pay_by");
        let payer_field = if kind == "payment" {
            "usdt_payer"
        } else {
            "ever_payer"
        };
        if body[payer_field] != json!(verified) {
            return Err(Error::Unauthorized);
        }
        let allowed = match state.as_str() {
            "payment_pending" => {
                kind == "payment" && Utc::now() <= pay_by && body["usdt_payer"] == json!(verified)
            }
            "delivery_pending" => {
                kind == "payout"
                    && row
                        .get::<_, Option<DateTime<Utc>>>("payout_by")
                        .is_none_or(|until| Utc::now() <= until)
            }
            _ => false,
        };
        if !allowed {
            return Err(Error::Conflict(
                "transfer is unavailable in the current deal state".into(),
            ));
        }
        if tx
            .query_opt(
                "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind=$2 AND state!='failed'",
                &[&id, &kind],
            )
            .await?
            .is_some()
        {
            return Err(Error::Conflict(
                "this deal leg already has an unresolved attempt".into(),
            ));
        }
        let leg = self.deal_leg(&body, kind)?;
        let net = &self.p2p_net(&body)?;
        let call = if kind == "payment" {
            if super::chain::number(&self.evm_agree(net, "eth_chainId", json!([])).await?)?
                != u128::from(net.chain_id)
            {
                return Err(invalid("wrong Ethereum network"));
            }
            let nonce = format!("0x{:x}", self.evm_pending_nonce(net, &leg.sender).await?);
            let data = super::chain::ethereum::transfer_data(&leg)?;
            json!({"family":"evm","chain_id":net.chain_id,"from":leg.sender,"to":net.usdt,"data":data,"value":"0x0","nonce":nonce})
        } else {
            json!({"family":"everscale","sender":leg.sender,"recipient":leg.recipient,"amount":leg.transfer_amount,"bounce":true})
        };
        let checkpoint = if kind == "payment" {
            json!({"block": format!("0x{:x}", self.evm_finalized_height(net).await?)})
        } else {
            Value::Null
        };
        let attempt = Uuid::new_v4();
        let instructions = json!({
            "leg": leg, "call": call, "checkpoint": checkpoint,
            "attempt_id": attempt, "trade_id": id, "kind": kind,
            "submit_path": format!("/p2p/attempts/{attempt}/reference"),
            "prepared_at": Utc::now(),
        });
        tx.execute(
            "INSERT INTO otc_p2p_attempts(id,trade_id,kind,payer_actor,chain,state,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,'prepared',$6,$7)",
            &[&attempt, &id, &kind, &actor, &leg.chain, &instructions, &key],
        )
        .await?;
        tracing::info!(trade=%id, attempt=%attempt, kind, actor, "otc.p2p.handoff");
        let row = tx
            .query_one("SELECT * FROM otc_p2p_trades WHERE id=$1", &[&id])
            .await?;
        let mut result = deal(&row, actor);
        result["attempt"] =
            json!({"id": attempt, "state": "prepared", "instructions": instructions});
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn p2p_submit(
        &self,
        actor: &str,
        attempt: Uuid,
        reference: &str,
    ) -> Result<Value> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT a.trade_id,a.kind,a.reference,a.chain,t.owner_actor,t.taker_actor FROM otc_p2p_attempts a JOIN otc_p2p_trades t ON t.id=a.trade_id WHERE a.id=$1",
                &[&attempt],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let owner: String = row.get(4);
        let taker: String = row.get(5);
        if actor != owner && actor != taker {
            return Err(Error::NotFound);
        }
        let kind: String = row.get(1);
        let chain: String = row.get(3);
        let existing: Option<String> = row.get(2);
        if existing.as_ref().is_some_and(|r| r != reference) {
            return Err(Error::Conflict("original reference is immutable".into()));
        }
        let bytes = wallet::unhex(reference)?;
        if bytes.len() != 32 || (chain == "ethereum" && !reference.starts_with("0x")) {
            return Err(invalid("invalid chain reference"));
        }
        client
            .execute(
                "UPDATE otc_p2p_attempts SET reference=$2,state=CASE WHEN state='prepared' THEN 'submitted' ELSE state END,updated_at=now() WHERE id=$1",
                &[&attempt, &reference],
            )
            .await?;
        tracing::info!(trade=%row.get::<_, Uuid>(0), kind, "otc.p2p.reference");
        Ok(json!({"tracked": true}))
    }
    pub(crate) async fn p2p_cancel(&self, actor: &str, id: Uuid) -> Result<Value> {
        if !self.p2p_available() {
            return Err(Error::Disabled);
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 FOR UPDATE",
                &[&id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let owner: String = row.get("owner_actor");
        let taker: String = row.get("taker_actor");
        let state: String = row.get("state");
        let offer_id: Uuid = row.get("offer_id");
        if actor != owner && actor != taker {
            return Err(Error::NotFound);
        }
        if state != "payment_pending"
            || tx
                .query_opt(
                    "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND state NOT IN ('failed','verified')",
                    &[&id],
                )
                .await?
                .is_some()
        {
            return Err(Error::Conflict(
                "deal can only be cancelled before the USDT leg is sent".into(),
            ));
        }
        tx.execute(
            "UPDATE otc_p2p_trades SET state='cancelled',updated_at=now() WHERE id=$1",
            &[&id],
        )
        .await?;
        tx.execute(
            "UPDATE otc_public_offers SET state='open' WHERE id=$1 AND state='taken' AND expires_at>now()",
            &[&offer_id],
        )
        .await?;
        let row = tx
            .query_one("SELECT * FROM otc_p2p_trades WHERE id=$1", &[&id])
            .await?;
        let result = deal(&row, actor);
        tracing::info!(trade=%id, actor, "otc.p2p.cancelled");
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn p2p_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT a.id,a.trade_id,a.kind,a.chain,a.instructions,a.reference FROM otc_p2p_attempts a JOIN otc_p2p_trades t ON t.id=a.trade_id WHERE a.state IN ('prepared','submitted','confirming') AND t.state IN ('payment_pending','delivery_pending') ORDER BY a.created_at LIMIT 50",
                &[],
            )
            .await?;
        for row in rows {
            let attempt: Uuid = row.get(0);
            let id: Uuid = row.get(1);
            let kind: String = row.get(2);
            let chain: String = row.get(3);
            let instructions: Value = row.get(4);
            let mut reference: Option<String> = row.get(5);
            let leg: Leg = serde_json::from_value(instructions["leg"].clone())?;
            // Read the leg back on the network the handoff was built for.
            let evm_net = || super::chain::EvmNet {
                rpcs: self.cfg.p2p_rpc().to_vec(),
                chain_id: instructions["call"]["chain_id"]
                    .as_u64()
                    .unwrap_or_else(|| self.cfg.p2p_chain()),
                usdt: instructions["call"]["to"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| self.cfg.p2p_usdt().to_string()),
            };
            if reference.is_none() {
                let located = if chain == "ethereum" {
                    self.locate_ethereum(&leg, &instructions, &evm_net())
                        .await
                        .unwrap_or(None)
                } else {
                    self.locate_everscale(&leg, &instructions)
                        .await
                        .unwrap_or(None)
                };
                if let Some(found) = located {
                    client
                        .execute(
                            "UPDATE otc_p2p_attempts SET reference=$2,state='submitted' WHERE id=$1 AND reference IS NULL",
                            &[&attempt, &found],
                        )
                        .await?;
                    reference = Some(found);
                }
            }
            let Some(reference) = reference else { continue };
            let result = if chain == "ethereum" {
                let nonce = instructions["call"]["nonce"]
                    .as_str()
                    .and_then(|n| u128::from_str_radix(n.trim_start_matches("0x"), 16).ok());
                self.ethereum_evidence(&reference, &leg, nonce, &evm_net())
                    .await
            } else {
                self.everscale_deal_evidence(&reference, &leg).await
            };
            match result {
                Ok(Some(evidence)) if evidence.finalized && evidence.successful => {
                    self.credit_p2p(attempt, id, &kind, &evidence).await?;
                }
                Ok(Some(evidence)) if evidence.finalized => {
                    client
                        .execute(
                            "UPDATE otc_p2p_attempts SET state='failed',updated_at=now() WHERE id=$1",
                            &[&attempt],
                        )
                        .await?;
                    tracing::warn!(trade=%id, attempt=%attempt, "otc.p2p.transfer_failed");
                }
                Ok(Some(_)) => {
                    client
                        .execute(
                            "UPDATE otc_p2p_attempts SET state='confirming',updated_at=now() WHERE id=$1 AND state='submitted'",
                            &[&attempt],
                        )
                        .await?;
                }
                Ok(None) => {}
                Err(error) => {
                    // A reader disagreement or a transient RPC fault says nothing about the
                    // transfer itself; keeping the attempt open is what prevents a resend.
                    tracing::warn!(trade=%id, attempt=%attempt, error=%error, "otc.p2p.evidence_unreadable");
                    client
                        .execute(
                            "UPDATE otc_p2p_attempts SET state='confirming',updated_at=now() WHERE id=$1 AND state='submitted'",
                            &[&attempt],
                        )
                        .await?;
                }
            }
        }
        // Unpaid deals expire; the offer may return to the book.
        let stale = client
            .query(
                "SELECT id,offer_id FROM otc_p2p_trades WHERE state='payment_pending' AND pay_by<now() AND NOT EXISTS(SELECT 1 FROM otc_p2p_attempts WHERE trade_id=otc_p2p_trades.id AND kind='payment' AND state='verified')",
                &[],
            )
            .await?;
        for row in stale {
            let id: Uuid = row.get(0);
            let offer_id: Uuid = row.get(1);
            client
                .execute(
                    "UPDATE otc_p2p_trades SET state='expired',updated_at=now() WHERE id=$1 AND state='payment_pending'",
                    &[&id],
                )
                .await?;
            client
                .execute(
                    "UPDATE otc_public_offers SET state='open' WHERE id=$1 AND state='taken' AND expires_at>now()",
                    &[&offer_id],
                )
                .await?;
            tracing::info!(trade=%id, "otc.p2p.payment_expired");
        }
        let overdue = client
            .query(
                "SELECT id FROM otc_p2p_trades WHERE state='delivery_pending' AND payout_by IS NOT NULL AND payout_by<now()",
                &[],
            )
            .await?;
        for row in overdue {
            tracing::warn!(trade=%row.get::<_, Uuid>(0), "otc.p2p.delivery_overdue");
        }
        Ok(())
    }
    async fn credit_p2p(
        &self,
        attempt: Uuid,
        id: Uuid,
        kind: &str,
        evidence: &Evidence,
    ) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        let state: String = row.get("state");
        let offer_id: Uuid = row.get("offer_id");
        let expected = if kind == "payment" {
            "payment_pending"
        } else {
            "delivery_pending"
        };
        if state != expected {
            tx.commit().await?;
            return Ok(());
        }
        tx.execute(
            "UPDATE otc_p2p_attempts SET state='verified',evidence=$2,updated_at=now() WHERE id=$1",
            &[&attempt, &serde_json::to_value(evidence)?],
        )
        .await?;
        let (next, payout_by) = if kind == "payment" {
            (
                "delivery_pending",
                Some(Utc::now() + Duration::seconds(i64::from(self.cfg.payout_seconds))),
            )
        } else {
            (
                "completed",
                row.get::<_, Option<DateTime<Utc>>>("payout_by"),
            )
        };
        tx.execute(
            "UPDATE otc_p2p_trades SET state=$2,payout_by=$3,updated_at=now() WHERE id=$1",
            &[&id, &next, &payout_by],
        )
        .await?;
        if next == "completed" {
            tx.execute(
                "UPDATE otc_public_offers SET state='settled' WHERE id=$1 AND state='taken'",
                &[&offer_id],
            )
            .await?;
        }
        tracing::info!(trade=%id, kind, state=next, "otc.p2p.leg_verified");
        tx.commit().await?;
        Ok(())
    }
}
fn deal(row: &Row, actor: &str) -> Value {
    json!({
        "id": row.get::<_, Uuid>("id"),
        "offer_id": row.get::<_, Uuid>("offer_id"),
        "state": row.get::<_, String>("state"),
        "role": if row.get::<_, String>("owner_actor") == actor { "owner" } else { "taker" },
        "counterparty": if row.get::<_, String>("owner_actor") == actor {
            row.get::<_, String>("taker_actor")
        } else {
            row.get::<_, String>("owner_actor")
        },
        "terms": row.get::<_, Value>("body"),
        "pay_by": row.get::<_, DateTime<Utc>>("pay_by"),
        "payout_by": row.get::<_, Option<DateTime<Utc>>>("payout_by"),
        "created_at": row.get::<_, DateTime<Utc>>("created_at"),
    })
}

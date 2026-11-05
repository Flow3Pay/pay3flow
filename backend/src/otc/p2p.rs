//! Direct peer-to-peer settlement of public offers.
//! Two independent handoff-signed transfers (stablecoin first, then EVER); deliberately
//! bypasses the desk/RFQ, inventory and federation machinery.
use super::{
    amount,
    error::{invalid, Error, Result},
    model::{Direction, Evidence, Leg},
    wallet, Service,
};
use crate::route_engine::{Amount, Asset, NearQuoteRequest};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DealPrepareRequest {
    pub kind: String,
    #[serde(default)]
    pub source_asset: Option<String>,
    #[serde(default)]
    pub source_asset_id: Option<String>,
    #[serde(default)]
    pub source_amount: Option<String>,
    #[serde(default)]
    pub source_address: Option<String>,
    #[serde(default)]
    pub slippage_bps: Option<u32>,
}
impl From<&str> for DealPrepareRequest {
    fn from(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
            source_asset: None,
            source_asset_id: None,
            source_amount: None,
            source_address: None,
            slippage_bps: None,
        }
    }
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
        let usdt = body
            .get("stablecoin_contract")
            .or_else(|| body.get("usdt_contract"))
            .and_then(Value::as_str)
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
        // The fee policy is frozen with the deal: republishing the offer later
        // must never change what this trade agreed to pay.
        let stablecoin = super::public_offers::Stablecoin::from_terms(&terms)?;
        let token_contract = terms
            .get("stablecoin_contract")
            .or_else(|| terms.get("usdt_contract"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("offer terms are malformed"))?;
        let usdt_basis = amount::integer(
            terms["usdt_units"]
                .as_str()
                .ok_or_else(|| invalid("offer terms are malformed"))?,
        )?;
        let mut fee_policy = terms
            .get("fee_policy")
            .filter(|policy| policy.is_object())
            .cloned()
            .unwrap_or_else(|| super::public_offers::fee_policy(usdt_basis));
        fee_policy["version"] = json!(super::public_offers::P2P_FEE_VERSION);
        fee_policy["asset"] = json!("USDT");
        fee_policy["payer"] = json!("stablecoin_sender");
        fee_policy["withheld"] = json!(false);
        let direction: Direction = serde_json::from_value(terms["direction"].clone())?;
        let fee_payer_actor = if direction == Direction::Sell {
            actor
        } else {
            owner.as_str()
        };
        if self.cfg.p2p_chain() != 1
            || terms["ethereum_chain_id"] != json!(1)
            || token_contract != stablecoin.contract()
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
            "stablecoin": stablecoin.symbol(),
            "ever_amount": terms["ever_amount"], "usdt_amount": terms["usdt_amount"],
            "ever_units": terms["ever_units"], "usdt_units": terms["usdt_units"],
            "usdt_payer": usdt_payer, "usdt_payee": usdt_payee,
            "ever_payer": ever_payer, "ever_payee": ever_payee,
            "fee_payer_actor": fee_payer_actor,
            "usdt_contract": token_contract,
            "stablecoin_contract": token_contract,
            "ethereum_chain_id": 1,
            "ever_network_id": 42,
            "fee_policy": fee_policy,
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
    pub(crate) async fn p2p_confirm(&self, actor: &str, id: Uuid) -> Result<Value> {
        if !self.p2p_available() {
            return Err(Error::Disabled);
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2) FOR UPDATE",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let state: String = row.get("state");
        if state == "payment_pending" || state == "delivery_pending" {
            let owner: String = row.get("owner_actor");
            let sql = if owner == actor {
                "UPDATE otc_p2p_trades SET owner_confirmed_at=COALESCE(owner_confirmed_at,now()),updated_at=now() WHERE id=$1 RETURNING *"
            } else {
                "UPDATE otc_p2p_trades SET taker_confirmed_at=COALESCE(taker_confirmed_at,now()),updated_at=now() WHERE id=$1 RETURNING *"
            };
            let row = tx.query_one(sql, &[&id]).await?;
            let deal = deal(&row, actor);
            tx.commit().await?;
            tracing::info!(trade=%id, actor, "otc.p2p.confirmed");
            Ok(deal)
        } else {
            let deal = deal(&row, actor);
            tx.commit().await?;
            Ok(deal)
        }
    }
    async fn p2p_attempts(&self, ids: &[Uuid]) -> Result<Vec<(Uuid, Value)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.pool.get().await?;
        let rows = client
            .query(
                // Failed transfers are listed too: a leg that the chain rejected is something the
                // trader has to see and retry, not a silent reset back to "send".
                "SELECT id,trade_id,kind,state,instructions,reference FROM otc_p2p_attempts WHERE trade_id = ANY($1) ORDER BY created_at",
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
        request: impl Into<DealPrepareRequest>,
    ) -> Result<Value> {
        let request = request.into();
        if request.kind == "near-intents" {
            let source_asset = request
                .source_asset
                .as_deref()
                .ok_or_else(|| invalid("source asset is required"))?;
            let source_amount = request
                .source_amount
                .as_deref()
                .ok_or_else(|| invalid("source amount is required"))?;
            let source_address = request
                .source_address
                .as_deref()
                .ok_or_else(|| invalid("source address is required"))?;
            return self
                .p2p_prepare_near_intents(
                    actor,
                    id,
                    key,
                    source_asset,
                    request.source_asset_id.as_deref(),
                    source_amount,
                    source_address,
                    request.slippage_bps.unwrap_or(100),
                )
                .await;
        }
        let kind = request.kind.as_str();
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
            // The stablecoin leg is already verified and the counterparty has paid.
            // Refusing a late EVER delivery would strand those funds, so the
            // nominal payout deadline is advisory once the deal is here.
            "delivery_pending" => kind == "payout",
            _ => false,
        };
        if !allowed {
            let message = match state.as_str() {
                "payment_pending" if kind == "payout" => {
                    "the EVER transfer becomes available after the stablecoin payment is verified"
                }
                "payment_pending" => "the stablecoin payment window has expired",
                "delivery_pending" => "the EVER transfer is unavailable for this deal",
                "completed" | "cancelled" | "expired" => "this deal is already closed",
                _ => "transfer is unavailable in the current deal state",
            };
            return Err(Error::Conflict(message.into()));
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
            "stablecoin": body.get("stablecoin").cloned().unwrap_or(json!("USDT")),
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
    async fn p2p_prepare_near_intents(
        &self,
        actor: &str,
        id: Uuid,
        key: &str,
        source_asset: &str,
        source_asset_id: Option<&str>,
        source_amount: &str,
        source_address: &str,
        slippage_bps: u32,
    ) -> Result<Value> {
        if !self.p2p_available() {
            return Err(Error::Disabled);
        }
        if key.trim().is_empty() || key.len() > 128 {
            return Err(invalid("Idempotency-Key required (1–128 characters)"));
        }
        let from = Asset::parse(source_asset).map_err(|_| invalid("unsupported source asset"))?;
        let network = from
            .location
            .as_deref()
            .ok_or_else(|| invalid("source asset must include its network"))?;
        if !crate::route_execution::supports_otc_wallet_network(network) {
            return Err(invalid("source network has no OTC wallet executor"));
        }
        crate::route_execution::validate_otc_wallet_address(network, source_address)
            .map_err(|_| invalid("invalid source wallet address"))?;
        let mut client = self.pool.get().await?;
        let body: Value = client
            .query_opt(
                "SELECT body FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2)",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?
            .get(0);
        let stablecoin = super::public_offers::Stablecoin::from_terms(&body)?;
        let to = Asset::new(stablecoin.symbol(), Some("ethereum"))?;
        if from == to {
            drop(client);
            return Box::pin(self.p2p_prepare(actor, id, key, "payment")).await;
        }
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT * FROM otc_p2p_trades WHERE id=$1 AND (owner_actor=$2 OR taker_actor=$2) FOR UPDATE",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let state: String = row.get("state");
        let body: Value = row.get("body");
        let owner: String = row.get("owner_actor");
        let taker: String = row.get("taker_actor");
        let pay_by: DateTime<Utc> = row.get("pay_by");
        let expected_payer = match body["direction"].as_str() {
            Some("sell") => taker,
            Some("buy") => owner,
            _ => return Err(invalid("deal direction is malformed")),
        };
        if actor != expected_payer {
            return Err(Error::Unauthorized);
        }
        if let Some(existing) = tx
            .query_opt(
                "SELECT id,state,instructions FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND idempotency_key=$2",
                &[&id, &key],
            )
            .await?
        {
            let mut result = deal(&row, actor);
            result["attempt"] = json!({
                "id": existing.get::<_, Uuid>(0),
                "state": existing.get::<_, String>(1),
                "instructions": existing.get::<_, Value>(2),
            });
            tx.commit().await?;
            return Ok(result);
        }
        if state != "payment_pending" || Utc::now() > pay_by {
            return Err(Error::Conflict("stablecoin payment is unavailable".into()));
        }
        if tx
            .query_opt(
                "SELECT id FROM otc_p2p_attempts WHERE trade_id=$1 AND kind='payment' AND state!='failed'",
                &[&id],
            )
            .await?
            .is_some()
        {
            return Err(Error::Conflict(
                "this deal leg already has an unresolved attempt".into(),
            ));
        }
        let provider = self.near_intents.as_ref().ok_or(Error::Disabled)?;
        let source_asset_id = source_asset_id
            .filter(|asset_id| !asset_id.trim().is_empty())
            .ok_or_else(|| invalid("source asset id is required"))?;
        let source_token = provider
            .token_for_asset_id(&from, source_asset_id)
            .await
            .map_err(|_| invalid("selected payment asset is no longer available"))?;
        if !matches!(
            source_token.symbol.to_ascii_uppercase().as_str(),
            "USDT" | "USDC"
        ) {
            return Err(invalid("only USDT and USDC can fund an OTC deal"));
        }
        let decimals = source_token
            .decimals
            .ok_or_else(|| invalid("source token precision is unavailable"))?;
        let token_contract = source_token
            .contract_address
            .clone()
            .filter(|contract| !contract.trim().is_empty())
            .ok_or_else(|| invalid("source token wallet transfer is unavailable"))?;
        amount::units(source_amount, u32::from(decimals))?;
        let destination_token = provider
            .token_for_asset(&to)
            .await
            .map_err(|error| {
                tracing::warn!(trade=%id, error=%error, "OTC destination token is unavailable");
                Error::RouteUnavailable
            })?;
        if destination_token.decimals != Some(6)
            || destination_token
                .contract_address
                .as_deref()
                .is_none_or(|address| !address.eq_ignore_ascii_case(stablecoin.contract()))
        {
            return Err(invalid("destination token precision is unsupported"));
        }
        if network == "ethereum" {
            let expected_contract = if source_token.symbol.eq_ignore_ascii_case("USDT") {
                super::config::USDT
            } else {
                super::config::USDC
            };
            if source_token
                .contract_address
                .as_deref()
                .is_none_or(|address| !address.eq_ignore_ascii_case(expected_contract))
            {
                return Err(invalid("unsupported Ethereum stablecoin contract"));
            }
        }
        let recipient = body["usdt_payee"]
            .as_str()
            .ok_or_else(|| invalid("deal recipient is missing"))?;
        let refund_to = source_address;
        let amount = Amount::new(source_amount, from.clone())?;
        let quote = provider
            .executable_quote(NearQuoteRequest {
                from: from.clone(),
                to: to.clone(),
                origin_asset_id: Some(source_token.asset_id.clone()),
                destination_asset_id: Some(destination_token.asset_id.clone()),
                amount,
                recipient: recipient.to_string(),
                refund_to: refund_to.to_string(),
                slippage_bps: slippage_bps.min(10_000),
                dry: false,
            })
            .await
            .map_err(|error| {
                tracing::warn!(trade=%id, error=%error, "OTC payment route quote failed");
                Error::RouteUnavailable
            })?;
        let deposit_address = quote
            .deposit_address
            .as_deref()
            .filter(|address| !address.trim().is_empty())
            .ok_or_else(|| invalid("Cross-network payment is temporarily unavailable"))?;
        let expires_at = quote
            .expires_at
            .unwrap_or_else(|| Utc::now() + Duration::minutes(3));
        if expires_at <= Utc::now() {
            return Err(Error::Conflict(
                "The payment quote has expired; request a new quote".into(),
            ));
        }
        let required_units = amount::integer(
            body["usdt_units"]
                .as_str()
                .ok_or_else(|| invalid("deal amount is malformed"))?,
        )?;
        let quoted_units = amount::units(&quote.output.value, 6)?;
        if quoted_units != required_units {
            return Err(Error::Conflict(
                "The route quote must match the exact deal amount".into(),
            ));
        }
        let deposit_memo =
            crate::route_engine::first_string(&quote.raw, &["depositMemo", "deposit_memo"]);
        if deposit_memo.is_some() && network != "near" {
            return Err(invalid(
                "This payment requires a wallet instruction that is not supported on the selected network",
            ));
        }
        let leg = super::model::Leg {
            chain: network.to_string(),
            amount: source_amount.to_string(),
            transfer_amount: source_amount.to_string(),
            sender: source_address.to_string(),
            recipient: deposit_address.to_string(),
        };
        let attempt = Uuid::new_v4();
        let instructions = json!({
            "leg": leg,
            "stablecoin": stablecoin.symbol(),
            "source_asset": from,
            "source_network": network,
            "source_asset_id": source_token.asset_id,
            "source_decimals": decimals,
            "token_contract": token_contract,
            "source_amount": source_amount,
            "source_address": source_address,
            "destination_asset": to,
            "destination_asset_id": destination_token.asset_id,
            "destination_decimals": 6,
            "recipient": recipient,
            "refund_to": refund_to,
            "expected_output": quote.output.value,
            "expected_fee": quote.fee.as_ref().map(|fee| json!({
                "asset": fee.asset.to_string(),
                "amount": fee.value,
            })),
            "slippage_bps": slippage_bps.min(10_000),
            "minimum_output": body["usdt_amount"],
            "deposit_address": deposit_address,
            "deposit_memo": deposit_memo,
            "quote_id": quote.quote_id,
            "quote_expires_at": expires_at,
            "quote": quote.raw,
            "attempt_id": attempt,
            "trade_id": id,
            "submit_path": format!("/p2p/attempts/{attempt}/reference"),
        });

        tx.execute(
            "INSERT INTO otc_p2p_attempts(id,trade_id,kind,payer_actor,chain,state,instructions,idempotency_key) VALUES($1,$2,'payment',$3,'near-intents','prepared',$4,$5)",
            &[&attempt, &id, &actor, &instructions, &key],
        )
        .await?;
        let trade = tx
            .query_one("SELECT * FROM otc_p2p_trades WHERE id=$1", &[&id])
            .await?;
        let mut result = deal(&trade, actor);
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
                "SELECT a.trade_id,a.kind,a.reference,a.chain,t.owner_actor,t.taker_actor,a.instructions FROM otc_p2p_attempts a JOIN otc_p2p_trades t ON t.id=a.trade_id WHERE a.id=$1",
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
        let instructions: Value = row.get(6);
        if existing.as_ref().is_some_and(|r| r != reference) {
            return Err(Error::Conflict("original reference is immutable".into()));
        }
        if chain == "near-intents" {
            if reference.is_empty()
                || reference.len() > 160
                || !reference
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            {
                return Err(invalid("invalid NEAR Intents transaction reference"));
            }
        } else {
            let bytes = wallet::unhex(reference)?;
            if bytes.len() != 32 || (chain == "ethereum" && !reference.starts_with("0x")) {
                return Err(invalid("invalid chain reference"));
            }
        }
        client
            .execute(
                "UPDATE otc_p2p_attempts SET reference=$2,state=CASE WHEN state='prepared' THEN 'submitted' ELSE state END,updated_at=now() WHERE id=$1",
                &[&attempt, &reference],
            )
            .await?;
        if chain == "near-intents" {
            if let Some(provider) = &self.near_intents {
                if let Err(error) = provider
                    .submit_deposit_tx_with_memo(
                        reference,
                        instructions["deposit_address"].as_str().unwrap_or_default(),
                        instructions["deposit_memo"].as_str(),
                    )
                    .await
                {
                    tracing::warn!(attempt=%attempt, error=%error, "NEAR Intents deposit notification failed");
                    return Err(Error::RouteUnavailable);
                }
            }
        }
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
                "deal can only be cancelled before the stablecoin leg is sent".into(),
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
            if chain == "near-intents" {
                let Some(provider) = &self.near_intents else {
                    continue;
                };
                let deposit_address = instructions["deposit_address"]
                    .as_str()
                    .ok_or_else(|| invalid("NEAR Intents deposit address is missing"))?;
                let deposit_memo = instructions["deposit_memo"].as_str();
                let status = match provider
                    .status_with_memo(deposit_address, deposit_memo)
                    .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        tracing::warn!(trade=%id, attempt=%attempt, error=%error, "NEAR Intents status poll failed");
                        continue;
                    }
                };
                match status.status.to_ascii_uppercase().as_str() {
                    "SUCCESS" => {
                        let output = status.output_amount.clone().or_else(|| {
                            crate::route_engine::first_string(
                                &status.raw,
                                &["amountOut", "amount_out", "outputAmount"],
                            )
                        });
                        let Some(output) = output else {
                            tracing::warn!(trade=%id, attempt=%attempt, "NEAR Intents success has no output amount");
                            continue;
                        };
                        let Some(output_units) = provider_amount_units(&output, 6) else {
                            tracing::warn!(trade=%id, attempt=%attempt, "NEAR Intents success has invalid output amount");
                            continue;
                        };
                        let Some(minimum_units) = instructions["minimum_output"]
                            .as_str()
                            .and_then(|value| amount::units(value, 6).ok())
                        else {
                            return Err(invalid("NEAR Intents minimum output is malformed"));
                        };
                        if output_units != minimum_units {
                            tracing::warn!(trade=%id, attempt=%attempt, output_units, minimum_units, "cross-network output does not match OTC amount");
                            continue;
                        }
                        let identity = reference
                            .clone()
                            .or_else(|| {
                                crate::route_engine::first_string(
                                    &status.raw,
                                    &["destinationTxHash", "destination_tx_hash", "txHash"],
                                )
                            })
                            .unwrap_or_else(|| {
                                instructions["quote_id"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string()
                            });
                        let evidence = Evidence {
                            chain: "near-intents".into(),
                            identity,
                            leg,
                            included_at: Utc::now(),
                            canonical: true,
                            finalized: true,
                            successful: true,
                            raw: json!({
                                "provider": "near-intents",
                                "status": status.raw,
                                "submitted_reference": reference,
                                "actual_output_units": output_units.to_string(),
                                "minimum_output_units": minimum_units.to_string(),
                            }),
                        };
                        self.credit_p2p(attempt, id, &kind, &evidence).await?;
                    }
                    "REFUNDED" => {
                        client
                            .execute(
                                "UPDATE otc_p2p_attempts SET state='failed',evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('prepared','submitted','confirming')",
                                &[&attempt, &json!({"provider":"near-intents","status":status.raw,"outcome":"refunded"})],
                            )
                            .await?;
                    }
                    _ => {
                        client
                            .execute(
                                "UPDATE otc_p2p_attempts SET state='confirming',evidence=$2,updated_at=now() WHERE id=$1 AND state IN ('submitted','confirming')",
                                &[&attempt, &json!({"provider":"near-intents","status":status.raw})],
                            )
                            .await?;
                    }
                }
                continue;
            }
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
    pub(crate) async fn credit_p2p(
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
        let body: Value = row.get("body");
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
            // Both chain legs settle gross. The 3% service fee is accrued once,
            // in the same transaction that completes the deal, for later collection.
            let basis = amount::integer(
                body["usdt_units"]
                    .as_str()
                    .ok_or_else(|| invalid("deal terms are malformed"))?,
            )?;
            let policy = body
                .get("fee_policy")
                .filter(|policy| policy.is_object())
                .cloned()
                .unwrap_or_else(|| super::public_offers::fee_policy(basis));
            let fee_asset = policy
                .get("asset")
                .and_then(Value::as_str)
                .filter(|asset| matches!(*asset, "USDT" | "USDC"))
                .ok_or_else(|| invalid("deal fee asset is malformed"))?;
            let payer_actor = body
                .get("fee_payer_actor")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("stablecoin fee payer is missing"))?;
            let text = |key: &str| -> Result<String> {
                Ok(policy
                    .get(key)
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("deal fee terms are malformed"))?
                    .to_string())
            };
            tx.execute(
                    "INSERT INTO otc_p2p_fees(id,trade_id,offer_id,asset,precision,basis_units,fee_units,net_units,rate,version,state,payer_actor) VALUES($1,$2,$3,$4,6,$5::text::numeric,$6::text::numeric,$7::text::numeric,$8::text::numeric,$9,'accrued',$10) ON CONFLICT (trade_id) DO NOTHING",
                    &[&Uuid::new_v4(), &id, &offer_id, &fee_asset, &text("basis_units")?, &text("fee_units")?, &text("net_units")?, &text("rate")?, &text("version")?, &payer_actor],
            )
            .await?;
        }
        tracing::info!(trade=%id, kind, state=next, "otc.p2p.leg_verified");
        tx.commit().await?;
        Ok(())
    }
}
fn provider_amount_units(value: &str, decimals: u32) -> Option<u128> {
    if value.contains('.') {
        amount::units(value, decimals).ok()
    } else {
        value.parse().ok()
    }
}
fn deal(row: &Row, actor: &str) -> Value {
    let body: Value = row.get("body");
    let state: String = row.get("state");
    let owner_actor: String = row.get("owner_actor");
    let taker_actor: String = row.get("taker_actor");
    let is_owner = owner_actor == actor;
    let direction = body["direction"].as_str().unwrap_or("sell").to_string();
    let ever_payer = body["ever_payer"].as_str().unwrap_or_default().to_string();
    let ever_payee = body["ever_payee"].as_str().unwrap_or_default().to_string();
    let (owner_ever, taker_ever) = if direction == "sell" {
        (ever_payer, ever_payee)
    } else {
        (ever_payee, ever_payer)
    };
    let (my_ever, counterparty_ever) = if is_owner {
        (owner_ever, taker_ever)
    } else {
        (taker_ever, owner_ever)
    };
    let (my_leg, counterparty_leg) = match (is_owner, direction.as_str()) {
        (true, "buy") | (false, "sell") => ("payment", "payout"),
        _ => ("payout", "payment"),
    };
    let owner_confirmed = row
        .get::<_, Option<DateTime<Utc>>>("owner_confirmed_at")
        .is_some();
    let taker_confirmed = row
        .get::<_, Option<DateTime<Utc>>>("taker_confirmed_at")
        .is_some();
    let (my_confirmed, counterparty_confirmed) = if is_owner {
        (owner_confirmed, taker_confirmed)
    } else {
        (taker_confirmed, owner_confirmed)
    };
    let my_avatar = super::avatar::identicon(&my_ever);
    let counterparty_avatar = super::avatar::identicon(&counterparty_ever);
    let counterparty = if is_owner { taker_actor } else { owner_actor };
    let fee_policy = body
        .get("fee_policy")
        .filter(|policy| policy.is_object())
        .cloned()
        .or_else(|| {
            body["usdt_units"]
                .as_str()
                .and_then(|units| units.parse::<u128>().ok())
                .map(super::public_offers::fee_policy)
        })
        .unwrap_or(Value::Null);
    let fee_accrued = state == "completed";
    json!({
        "id": row.get::<_, Uuid>("id"),
        "offer_id": row.get::<_, Uuid>("offer_id"),
        "state": state,
        "role": if is_owner { "owner" } else { "taker" },
        "counterparty": counterparty,
        "my_leg": my_leg,
        "counterparty_leg": counterparty_leg,
        "my_ever": my_ever,
        "counterparty_ever": counterparty_ever,
        "my_avatar": my_avatar,
        "counterparty_avatar": counterparty_avatar,
        "my_confirmed": my_confirmed,
        "counterparty_confirmed": counterparty_confirmed,
        "owner_confirmed": owner_confirmed,
        "taker_confirmed": taker_confirmed,
        "terms": body,
        "fee_policy": fee_policy,
        "fee_accrued": fee_accrued,
        "pay_by": row.get::<_, DateTime<Utc>>("pay_by"),
        "payout_by": row.get::<_, Option<DateTime<Utc>>>("payout_by"),
        "created_at": row.get::<_, DateTime<Utc>>("created_at"),
    })
}

use super::{
    amount,
    error::{invalid, Error, Result},
    model::{Direction, FeePolicy, Terms},
    protocol, repo, Service,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RfqRequest {
    pub direction: Direction,
    pub input: String,
    pub listing_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QuoteRequest {
    pub rfq_id: Uuid,
    pub output: String,
    pub network_costs: String,
    pub ever_receiving_cost: String,
}
impl Service {
    pub(crate) async fn rfq(&self, actor: &str, key: &str, request: RfqRequest) -> Result<Value> {
        self.quoting().await?;
        if actor == self.cfg.desk_actor {
            return Err(invalid("desk cannot trade with itself"));
        }
        let precision = if request.direction == Direction::Buy {
            6
        } else {
            9
        };
        amount::units(&request.input, precision)?;
        self.wallet(actor, "ethereum").await?;
        self.wallet(actor, "everscale").await?;
        let listings = self.listings().await?;
        let listing = listings
            .as_array()
            .and_then(|items| items.iter().find(|p| p["id"] == request.listing_id))
            .ok_or_else(|| invalid("listing is not a current desk Proposal"))?;
        let expected = if request.direction == Direction::Buy {
            &self.cfg.ever_resource
        } else {
            &self.cfg.usdt_resource
        };
        if listing["publishes"]["resourceConformsTo"] != *expected {
            return Err(invalid("listing direction mismatch"));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(row) = tx
            .query_opt(
                "SELECT id,body FROM otc_rfq WHERE actor=$1 AND idempotency_key=$2",
                &[&actor, &key],
            )
            .await?
        {
            let body: Value = row.get(1);
            if body["inReplyTo"] != request.listing_id
                || body["reciprocal"]["resourceQuantity"]["hasNumericalValue"] != request.input
            {
                return Err(Error::Conflict("RFQ key reused with changed terms".into()));
            }
            return Ok(json!({"id":row.get::<_,Uuid>(0),"state":"waiting"}));
        }
        if tx.query_opt("SELECT id FROM otc_trades WHERE customer_actor=$1 AND state='booking_pending' LIMIT 1",&[&actor]).await?.is_some() {return Err(Error::Conflict("unknown booking must reconcile before another selection".into()));}
        let id = Uuid::new_v4();
        let iri = format!("{}/rfqs/{id}", actor.trim_end_matches('/'));
        let mut body = listing.clone();
        body["id"] = json!(iri);
        body["purpose"] = json!("request");
        body["attributedTo"] = json!(actor);
        body["to"] = json!([self.cfg.desk_actor]);
        body["inReplyTo"] = json!(request.listing_id);
        body["publishes"]["id"] = json!(format!("{iri}#primary"));
        body["reciprocal"]["id"] = json!(format!("{iri}#reciprocal"));
        body["reciprocal"]["resourceQuantity"]["hasNumericalValue"] = json!(request.input);
        let direction = if request.direction == Direction::Buy {
            "buy"
        } else {
            "sell"
        };
        tx.execute("INSERT INTO otc_rfq(id,actor,direction,input,listing_id,body,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&id,&actor,&direction,&request.input,&request.listing_id,&body,&key]).await?;
        repo::enqueue(&tx, actor, &body, "rfq").await?;
        repo::event(
            &tx,
            None,
            Some(id),
            "rfq_requested",
            &json!({"direction":direction}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"id":id,"state":"waiting"}))
    }
    pub(crate) async fn quote(&self, actor: &str, request: QuoteRequest) -> Result<Value> {
        self.desk(actor)?;
        self.quoting().await?;
        let customer_ever;
        let customer_usdt;
        {
            let client = self.pool.get().await?;
            let row = client
                .query_opt("SELECT actor FROM otc_rfq WHERE id=$1", &[&request.rfq_id])
                .await?
                .ok_or(Error::NotFound)?;
            let customer: String = row.get(0);
            customer_ever = self.wallet(&customer, "everscale").await?;
            customer_usdt = self.wallet(&customer, "ethereum").await?;
        }
        if self.wallet(actor, "ethereum").await? != self.cfg.usdt_wallet
            || self.wallet(actor, "everscale").await? != self.cfg.ever_wallet
        {
            return Err(invalid(
                "desk wallet proofs do not match configured inventory wallets",
            ));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let rfq = tx
            .query_opt(
                "SELECT * FROM otc_rfq WHERE id=$1 FOR UPDATE",
                &[&request.rfq_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        if let Some(existing) = tx
            .query_opt(
                "SELECT * FROM otc_trades WHERE rfq_id=$1",
                &[&request.rfq_id],
            )
            .await?
        {
            let trade = repo::trade(&existing)?;
            if trade.terms.output != request.output
                || trade.terms.network_costs != request.network_costs
                || trade.terms.ever_receiving_cost_units
                    != amount::units_or_zero(&request.ever_receiving_cost, 9)?.to_string()
            {
                return Err(Error::Conflict(
                    "quote already fixed; create a new RFQ".into(),
                ));
            }
            return Ok(serde_json::to_value(trade)?);
        }
        let rfq_body: Value = rfq.get("body");
        if tx
            .query_opt(
                "SELECT activity_id FROM otc_inbox WHERE actor=$1 AND activity_id=$2",
                &[&actor, &protocol::text(&rfq_body, "id")?],
            )
            .await?
            .is_none()
        {
            return Err(Error::Conflict("RFQ has not arrived through fmatch".into()));
        }
        let direction = if rfq.get::<_, String>("direction") == "buy" {
            Direction::Buy
        } else {
            Direction::Sell
        };
        let input: String = rfq.get("input");
        let input_units = amount::units(&input, if direction == Direction::Buy { 6 } else { 9 })?;
        let output_units = amount::units(
            &request.output,
            if direction == Direction::Buy { 9 } else { 6 },
        )?;
        let basis = if direction == Direction::Buy {
            input_units
        } else {
            output_units
        };
        if basis < amount::units(&self.cfg.min_usdt, 6)?
            || basis > amount::units(&self.cfg.max_usdt, 6)?
        {
            return Err(invalid("quote exceeds desk trade limits"));
        }
        if request.network_costs.trim().is_empty() {
            return Err(invalid("separate network costs disclosure required"));
        }
        let now = Utc::now();
        let quote_by = now + Duration::seconds(i64::from(self.cfg.quote_seconds));
        let pay_by = quote_by + Duration::seconds(i64::from(self.cfg.payment_seconds));
        let payout_by = pay_by + Duration::seconds(i64::from(self.cfg.payout_seconds));
        let id = Uuid::new_v4();
        let mut terms = Terms {
            demo: self.cfg.demo,
            direction,
            input,
            output: request.output,
            input_units: input_units.to_string(),
            output_units: output_units.to_string(),
            customer_actor: rfq.get("actor"),
            desk_actor: actor.into(),
            customer_ever,
            customer_usdt,
            desk_ever: self.cfg.ever_wallet.clone(),
            desk_usdt: self.cfg.usdt_wallet.clone(),
            quote_by,
            pay_by,
            payout_by,
            network_costs: request.network_costs,
            ever_receiving_cost_units: amount::units_or_zero(&request.ever_receiving_cost, 9)?
                .to_string(),
            refund_policy: self.cfg.refund_policy.clone(),
            fee_policy: FeePolicy {
                version: "desk-0.25pct-floor5-v1".into(),
                rate: "0.0025".into(),
                floor: "5".into(),
                basis_units: basis.to_string(),
                fee_units: amount::fee(basis, 6)?.to_string(),
                precision: 6,
            },
            proposal: Value::Null,
        };
        terms.leg("payment")?;
        terms.leg("payout")?;
        terms.proposal = protocol::proposal(&format!("{actor}/quotes/{id}"), &terms, &self.cfg);
        let serialized = serde_json::to_value(&terms)?;
        tx.execute("INSERT INTO otc_trades(id,rfq_id,customer_actor,desk_actor,terms,state) VALUES($1,$2,$3,$4,$5,'quoted')",&[&id,&request.rfq_id,&terms.customer_actor,&actor,&serialized]).await?;
        tx.execute(
            "UPDATE otc_rfq SET state='quoted' WHERE id=$1",
            &[&request.rfq_id],
        )
        .await?;
        repo::enqueue(&tx, actor, &terms.proposal, "quote").await?;
        repo::event(&tx, Some(id), Some(request.rfq_id), "quoted", &json!({})).await?;
        tx.commit().await?;
        self.details(actor, id).await
    }
    pub(crate) async fn apply(&self, actor: &str, id: Uuid) -> Result<Value> {
        self.ready().await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Serialize all selections for a customer, including different trade rows.
        tx.execute("SELECT pg_advisory_xact_lock(hashtext($1))", &[&actor])
            .await?;
        let row = tx
            .query_opt(
                "SELECT * FROM otc_trades WHERE id=$1 AND customer_actor=$2 FOR UPDATE",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let trade = repo::trade(&row)?;
        if trade.offer.is_some() {
            return Ok(serde_json::to_value(trade)?);
        }
        if trade.state != "quoted" || Utc::now() > trade.terms.quote_by {
            return Err(invalid("quote expired or unavailable"));
        }
        if tx
            .query_opt(
                "SELECT id FROM otc_trades WHERE customer_actor=$1 AND state='booking_pending'",
                &[&actor],
            )
            .await?
            .is_some()
        {
            return Err(Error::Conflict("original booking is unresolved".into()));
        }
        if tx
            .query_opt(
                "SELECT activity_id FROM otc_inbox WHERE actor=$1 AND activity_id=$2",
                &[&actor, &protocol::text(&trade.terms.proposal, "id")?],
            )
            .await?
            .is_none()
        {
            return Err(Error::Conflict(
                "private quote has not arrived through fmatch".into(),
            ));
        }
        let offer = protocol::offer(&format!("{actor}/offers/{id}"), &trade.terms);
        protocol::validate_offer(&offer, &trade.terms, Utc::now())?;
        tx.execute(
            "UPDATE otc_trades SET state='booking_pending',offer=$2,updated_at=now() WHERE id=$1",
            &[&id, &offer],
        )
        .await?;
        repo::enqueue(&tx, actor, &offer, "booking").await?;
        repo::event(
            &tx,
            Some(id),
            Some(trade.rfq_id),
            "booking_requested",
            &json!({}),
        )
        .await?;
        tx.commit().await?;
        self.details(actor, id).await
    }
    pub(crate) async fn decide(&self, actor: &str, id: Uuid, accept: bool) -> Result<Value> {
        self.desk(actor)?;
        if accept {
            self.ready().await?;
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_opt(
                "SELECT * FROM otc_trades WHERE id=$1 AND desk_actor=$2 FOR UPDATE",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let trade = repo::trade(&row)?;
        if trade.decision.is_some() {
            return Ok(serde_json::to_value(trade)?);
        }
        let offer = trade
            .offer
            .as_ref()
            .ok_or_else(|| invalid("no customer offer"))?;
        if tx
            .query_opt(
                "SELECT activity_id FROM otc_inbox WHERE actor=$1 AND activity_id=$2",
                &[&actor, &protocol::text(offer, "id")?],
            )
            .await?
            .is_none()
        {
            return Err(Error::Conflict(
                "customer offer has not arrived through fmatch".into(),
            ));
        }
        protocol::validate_offer(
            offer,
            &trade.terms,
            if accept {
                Utc::now()
            } else {
                trade.terms.quote_by
            },
        )?;
        let decision = if accept {
            let chain = trade.terms.direction.output_chain();
            let row=tx.query_opt("SELECT balance::text,gas_reserve::text FROM otc_inventory WHERE chain=$1 AND observed_at>now()-interval '60 seconds' FOR UPDATE",&[&chain]).await?.ok_or(Error::Disabled)?;
            let balance = amount::integer(&row.get::<_, String>(0))?;
            let gas = amount::integer(&row.get::<_, String>(1))?;
            let reserved:String=tx.query_one("SELECT COALESCE(sum(amount),0)::text FROM otc_reservations WHERE chain=$1 AND status!='released'",&[&chain]).await?.get(0);
            let output = amount::integer(&trade.terms.leg("payout")?.transfer_amount)?;
            if balance
                .saturating_sub(gas)
                .saturating_sub(amount::integer(&reserved)?)
                < output
            {
                return Err(Error::Conflict(
                    "insufficient reconciled desk inventory".into(),
                ));
            }
            tx.execute("INSERT INTO otc_reservations(trade_id,chain,amount,status) VALUES($1,$2,$3::text::numeric,'active')",&[&id,&chain,&output.to_string()]).await?;
            let value =
                protocol::acceptance(&format!("{actor}/decisions/{id}"), offer, &trade.terms);
            protocol::validate_acceptance(&value, offer, &trade.terms)?;
            value
        } else {
            json!({"@context":protocol::context(),"id":format!("{actor}/decisions/{id}"),"type":"RejectAgreement","actor":actor,"to":[trade.terms.customer_actor],"object":offer["id"]})
        };
        if accept && Utc::now() > trade.terms.quote_by {
            return Err(invalid("quote expired while reserving inventory"));
        }
        tx.execute(
            "UPDATE otc_trades SET decision=$2,updated_at=now() WHERE id=$1",
            &[&id, &decision],
        )
        .await?;
        repo::enqueue(&tx, actor, &decision, "decision").await?;
        repo::event(
            &tx,
            Some(id),
            None,
            "booking_decided",
            &json!({"accepted":accept}),
        )
        .await?;
        tx.commit().await?;
        self.details(actor, id).await
    }
}

use super::{
    config::PROFILE,
    error::{invalid, Error, Result},
    protocol, repo, Service,
};
use serde_json::{json, Value};
impl Service {
    pub(crate) fn relay_actor_url(&self, actor: &str) -> Result<String> {
        let prefix = format!("{}/actors/", self.cfg.relay_origin.trim_end_matches('/'));
        let handle = actor
            .strip_prefix(&prefix)
            .ok_or_else(|| invalid("actor must be hosted by the configured LF relay"))?;
        if handle.is_empty()
            || handle.len() > 63
            || !handle
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(invalid("invalid LF actor IRI"));
        }
        Ok(actor.into())
    }
    pub(crate) async fn listings(&self) -> Result<Value> {
        if self.cfg.desk_actor.is_empty() {
            return Ok(json!([]));
        }
        let response = self
            .http
            .get(format!(
                "{}/outbox",
                self.relay_actor_url(&self.cfg.desk_actor)?
            ))
            .send()
            .await?;
        if !self.cfg.demo
            && response
                .headers()
                .get("x-relay-signatures")
                .and_then(|v| v.to_str().ok())
                != Some("required")
        {
            return Err(Error::Disabled);
        }
        if response
            .headers()
            .get("x-relay-profile")
            .and_then(|v| v.to_str().ok())
            != Some(PROFILE)
        {
            return Err(Error::Disabled);
        }
        let body: Value = response.error_for_status()?.json().await?;
        let mut proposals = Vec::new();
        for p in body["orderedItems"].as_array().into_iter().flatten() {
            let p = if p["type"] == "Create" {
                &p["object"]
            } else {
                p
            };
            if p["type"] != "Proposal"
                || p["purpose"] != "offer"
                || p["attributedTo"] != self.cfg.desk_actor
                || protocol::private(p)
            {
                continue;
            }
            let a = p["publishes"]["resourceConformsTo"].as_str();
            let b = p["reciprocal"]["resourceConformsTo"].as_str();
            if (a == Some(&self.cfg.ever_resource) && b == Some(&self.cfg.usdt_resource))
                || (a == Some(&self.cfg.usdt_resource) && b == Some(&self.cfg.ever_resource))
            {
                proposals.push(p.clone());
            }
        }
        Ok(json!(proposals))
    }
    pub(crate) async fn publish_listings(&self, actor: &str) -> Result<Value> {
        self.desk(actor)?;
        self.quoting().await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        for (slug, direction) in [
            ("buy", super::model::Direction::Buy),
            ("sell", super::model::Direction::Sell),
        ] {
            let p = protocol::listing(&format!("{actor}/listings/{slug}"), direction, &self.cfg);
            repo::enqueue(&tx, actor, &p, "listing").await?;
        }
        let follow = json!({"@context":"https://www.w3.org/ns/activitystreams","id":format!("{actor}/follows/otc"),"type":"Follow","actor":actor,"object":self.cfg.matcher_actor,"to":[self.cfg.matcher_actor]});
        repo::enqueue(&tx, actor, &follow, "subscription").await?;
        tx.commit().await?;
        Ok(json!({"queued":true}))
    }
    pub(crate) async fn ingest(&self, actor: &str, body: &Value, sequence: i64) -> Result<()> {
        let id = protocol::text(body, "id")?;
        let author = protocol::author(body)?;
        self.relay_actor_url(author)?;
        if !protocol::addressed(body, actor) || !protocol::private(body) {
            return Err(invalid("private OTC activity has the wrong audience"));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if let Some(existing) = tx
            .query_opt(
                "SELECT body FROM otc_inbox WHERE actor=$1 AND activity_id=$2",
                &[&actor, &id],
            )
            .await?
        {
            if existing.get::<_, Value>(0) != *body {
                return Err(Error::Conflict(
                    "incoming activity ID reused with changed body".into(),
                ));
            }
        } else {
            match body["type"].as_str() {
                Some("Proposal") => {
                    let known=tx.query_opt("SELECT body FROM otc_rfq WHERE body->>'id'=$1 AND actor=$2 UNION ALL SELECT terms->'proposal' FROM otc_trades WHERE terms->'proposal'->>'id'=$1 AND desk_actor=$2",&[&id,&author]).await?;
                    if known.as_ref().is_none_or(|r| r.get::<_, Value>(0) != *body) {
                        return Err(invalid("unsolicited or altered OTC Proposal"));
                    }
                }
                Some("OfferAgreement") => {
                    let row=tx.query_opt("SELECT * FROM otc_trades WHERE offer->>'id'=$1 AND desk_actor=$2 FOR UPDATE",&[&id,&actor]).await?.ok_or(Error::NotFound)?;
                    let trade = repo::trade(&row)?;
                    protocol::validate_offer(body, &trade.terms, trade.terms.quote_by)?;
                    if trade.offer.as_ref() != Some(body) {
                        return Err(invalid("relayed offer body differs"));
                    }
                }
                Some("AcceptAgreement") | Some("RejectAgreement") => {
                    let reference = protocol::text(body, "object")?;
                    let row=tx.query_opt("SELECT * FROM otc_trades WHERE offer->>'id'=$1 AND customer_actor=$2 FOR UPDATE",&[&reference,&actor]).await?.ok_or(Error::NotFound)?;
                    let trade = repo::trade(&row)?;
                    let offer = trade
                        .offer
                        .as_ref()
                        .ok_or_else(|| invalid("missing offer"))?;
                    if author != trade.terms.desk_actor || trade.decision.as_ref() != Some(body) {
                        return Err(invalid("decision author or persisted decision differs"));
                    }
                    let next = if body["type"] == "AcceptAgreement" {
                        protocol::validate_acceptance(body, offer, &trade.terms)?;
                        "accepted"
                    } else {
                        "rejected"
                    };
                    if trade.state == "booking_pending" {
                        tx.execute(
                            "UPDATE otc_trades SET state=$2,updated_at=now() WHERE id=$1",
                            &[&trade.id, &next],
                        )
                        .await?;
                        repo::event(
                            &tx,
                            Some(trade.id),
                            Some(trade.rfq_id),
                            next,
                            &json!({"decision":id}),
                        )
                        .await?;
                    }
                }
                Some("Create") if body["object"]["type"] == "Document" => {}
                Some("Accept") => {}
                _ => return Err(invalid("unsupported OTC activity")),
            }
            tx.execute(
                "INSERT INTO otc_inbox(actor,activity_id,body) VALUES($1,$2,$3)",
                &[&actor, &id, &body],
            )
            .await?;
        }
        tx.execute(
            "UPDATE otc_outbox SET delivered_at=now() WHERE activity_id=$1 AND actor=$2",
            &[&id, &author],
        )
        .await?;
        tx.execute("INSERT INTO otc_relay_cursors(actor,sequence) VALUES($1,$2) ON CONFLICT(actor) DO UPDATE SET sequence=GREATEST(otc_relay_cursors.sequence,$2)",&[&actor,&sequence]).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn relay_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let jobs=client.query("UPDATE otc_outbox SET lease_until=now()+interval '60 seconds' WHERE activity_id IN (SELECT activity_id FROM otc_outbox WHERE delivered_at IS NULL AND queued_at IS NULL AND next_attempt_at<=now() AND (lease_until IS NULL OR lease_until<now()) ORDER BY CASE purpose WHEN 'subscription' THEN 0 ELSE 1 END,created_at FOR UPDATE SKIP LOCKED LIMIT 10) RETURNING activity_id,actor,body",&[]).await?;
        for job in jobs {
            let id: String = job.get(0);
            let actor: String = job.get(1);
            let body: Value = job.get(2);
            let result = async {
                let token = self.credential(&actor).await?;
                let response = self
                    .http
                    .post(format!("{}/outbox", self.relay_actor_url(&actor)?))
                    .bearer_auth(token)
                    .json(&body)
                    .send()
                    .await?
                    .error_for_status()?;
                if !self.cfg.demo
                    && response
                        .headers()
                        .get("x-relay-signatures")
                        .and_then(|v| v.to_str().ok())
                        != Some("required")
                {
                    return Err(Error::Disabled);
                }
                let ack: Value = response.json().await?;
                if ack["accepted"] != true {
                    return Err(invalid("relay did not accept the activity"));
                }
                Ok(())
            }
            .await;
            match result {
                Ok(()) => {
                    client.execute("UPDATE otc_outbox SET queued_at=now(),delivered_at=CASE WHEN purpose IN ('listing','subscription') THEN now() ELSE delivered_at END,lease_until=NULL WHERE activity_id=$1",&[&id]).await?;
                }
                Err(error) => {
                    tracing::warn!(activity_id=%id,error=%error,"otc.relay.retry");
                    client.execute("UPDATE otc_outbox SET attempts=attempts+1,next_attempt_at=now()+interval '15 seconds',lease_until=NULL WHERE activity_id=$1",&[&id]).await?;
                }
            }
        }
        let actors = client
            .query("SELECT actor FROM otc_actor_links WHERE NOT revoked", &[])
            .await?;
        for row in actors {
            let actor: String = row.get(0);
            let cursor = client
                .query_opt(
                    "SELECT sequence FROM otc_relay_cursors WHERE actor=$1",
                    &[&actor],
                )
                .await?
                .map(|r| r.get::<_, i64>(0))
                .unwrap_or(0);
            let token = self.credential(&actor).await?;
            let response = self
                .http
                .get(format!(
                    "{}/inbox?cursor={cursor}&wait=0",
                    self.relay_actor_url(&actor)?
                ))
                .bearer_auth(token)
                .send()
                .await?;
            if !response.status().is_success() {
                continue;
            }
            if !self.cfg.demo
                && response
                    .headers()
                    .get("x-relay-signatures")
                    .and_then(|v| v.to_str().ok())
                    != Some("required")
            {
                return Err(Error::Disabled);
            }
            let page: Value = response.json().await?;
            for item in page["orderedItems"].as_array().into_iter().flatten() {
                let sequence = item["relaySequence"]
                    .as_i64()
                    .ok_or_else(|| invalid("missing relay sequence"))?;
                if sequence <= cursor {
                    continue;
                }
                if let Err(error) = self.ingest(&actor, &item["object"], sequence).await {
                    self.incident(
                        &format!("relay-invalid:{}", protocol::digest(item)),
                        "P2",
                        None,
                        "federation",
                        json!({"actor":actor,"sequence":sequence}),
                    )
                    .await?;
                    tracing::warn!(error=%error,"otc.relay.invalid");
                    break;
                }
            }
        }
        Ok(())
    }
}

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
        // A relay that does not expose the OTC signature/profile contract is
        // not a source of public desk listings. Treat it as an empty
        // collection instead of failing the whole public OTC page.
        if !response.status().is_success()
            || (!self.cfg.demo
                && response
                    .headers()
                    .get("x-relay-signatures")
                    .and_then(|v| v.to_str().ok())
                    != Some("required"))
            || response
                .headers()
                .get("x-relay-profile")
                .and_then(|v| v.to_str().ok())
                != Some(PROFILE)
        {
            return Ok(json!([]));
        }
        let body: Value = response.json().await?;
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
        if let Some(existing) = repo::federation::inbox_activity(&tx, actor, &id).await? {
            if existing != *body {
                return Err(Error::Conflict(
                    "incoming activity ID reused with changed body".into(),
                ));
            }
        } else {
            match body["type"].as_str() {
                Some("Proposal") => {
                    let known = repo::federation::known_proposal(&tx, &id, &author).await?;
                    if known.as_ref().is_none_or(|stored| *stored != *body) {
                        return Err(invalid("unsolicited or altered OTC Proposal"));
                    }
                }
                Some("OfferAgreement") => {
                    let row = repo::federation::lock_desk_offer(&tx, &id, actor)
                        .await?
                        .ok_or(Error::NotFound)?;
                    let trade = repo::trade(&row)?;
                    protocol::validate_offer(body, &trade.terms, trade.terms.quote_by)?;
                    if trade.offer.as_ref() != Some(body) {
                        return Err(invalid("relayed offer body differs"));
                    }
                }
                Some("AcceptAgreement") | Some("RejectAgreement") => {
                    let reference = protocol::text(body, "object")?;
                    let row = repo::federation::lock_customer_offer(&tx, &reference, actor)
                        .await?
                        .ok_or(Error::NotFound)?;
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
                        repo::federation::set_trade_state(&tx, &trade.id, next).await?;
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
            repo::federation::insert_inbox(&tx, actor, &id, body).await?;
        }
        repo::federation::mark_delivered(&tx, &id, &author).await?;
        repo::federation::advance_cursor(&tx, &actor, sequence).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn relay_tick(&self) -> Result<()> {
        let client = self.pool.get().await?;
        let jobs = repo::federation::lease_jobs(&client).await?;
        for (id, actor, body) in jobs {
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
                    repo::federation::mark_queued(&client, &id).await?;
                }
                Err(error) => {
                    tracing::warn!(activity_id=%id,error=%error,"otc.relay.retry");
                    repo::federation::reschedule_job(&client, &id).await?;
                }
            }
        }
        let actors = repo::federation::provisioned_actors(&client).await?;
        for actor in actors {
            let cursor = repo::federation::actor_sequence(&client, &actor)
                .await?
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

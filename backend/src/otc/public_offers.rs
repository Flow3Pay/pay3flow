//! Persistent buy and sell advertisements. Publication creates no settlement obligation.
use super::{
    amount,
    error::{invalid, Error, Result},
    model::Direction,
    wallet, Service,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_postgres::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicOfferDraft {
    #[serde(default = "legacy_sell_direction")]
    pub direction: Direction,
    pub ever_amount: String,
    pub usdt_amount: String,
    pub usdt_address: String,
}
fn legacy_sell_direction() -> Direction {
    Direction::Sell
}
impl PublicOfferDraft {
    fn terms(&self, cfg: &super::config::Config) -> Result<Value> {
        let ever = amount::units(&self.ever_amount, 9)?;
        let usdt = amount::units(&self.usdt_amount, 6)?;
        if ever > 100_000 * 1_000_000_000u128 || usdt > 1_000_000 * 1_000_000u128 {
            return Err(invalid("offer amount exceeds the publication limit"));
        }
        let recipient = wallet::address("ethereum", &self.usdt_address)?;
        if recipient == format!("0x{}", "0".repeat(40)) {
            return Err(invalid("Ethereum wallet address must be nonzero"));
        }
        Ok(json!({
            "direction":self.direction, "ever_amount":amount::decimal(ever,9),
            "usdt_amount":amount::decimal(usdt,6), "ever_units":ever.to_string(),
            "usdt_units":usdt.to_string(), "usdt_address":recipient,
            "ever_network_id":42, "ethereum_chain_id":cfg.p2p_chain(),
            "usdt_contract":cfg.p2p_usdt(),
        }))
    }
}
fn offer(row: &Row) -> Value {
    let expires: DateTime<Utc> = row.get("expires_at");
    let state: &str = row.get("state");
    json!({"id":row.get::<_,Uuid>("id"), "seller_ever":row.get::<_,String>("seller_ever"),
        "owner_ever":row.get::<_,String>("seller_ever"),
        "terms":row.get::<_,Value>("body"),
        "state":if state == "open" && expires <= Utc::now() {"expired"} else {state},
        "created_at":row.get::<_,DateTime<Utc>>("created_at"), "expires_at":expires})
}
impl Service {
    pub(crate) fn public_offers_available(&self) -> bool {
        self.cfg.public_offers_enabled
            && !self.cfg.demo
            && self.wallet_login_available()
            && self.wallet_login_chain_available("everscale")
    }
    pub(crate) async fn public_offers(&self, owner: Option<&str>) -> Result<Value> {
        let client = self.pool.get().await?;
        let rows = match owner {
            Some(actor) => client.query("SELECT * FROM otc_public_offers WHERE actor=$1 ORDER BY created_at DESC LIMIT 100", &[&actor]).await?,
            None => client.query("SELECT * FROM otc_public_offers WHERE state='open' AND expires_at>now() ORDER BY created_at DESC LIMIT 100", &[]).await?,
        };
        Ok(Value::Array(rows.iter().map(offer).collect()))
    }
    /// Public chart history contains publication terms only; closed offers stay
    /// in history without exposing their wallet addresses or current state.
    pub(crate) async fn public_offer_history(&self) -> Result<Value> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, body, created_at FROM otc_public_offers ORDER BY created_at ASC",
                &[],
            )
            .await?;
        Ok(Value::Array(
            rows.iter()
                .map(|row| {
                    let terms: Value = row.get("body");
                    json!({
                        "id": row.get::<_, Uuid>("id"),
                        "created_at": row.get::<_, DateTime<Utc>>("created_at"),
                        "terms": {
                            "direction": terms.get("direction").cloned().unwrap_or(json!("sell")),
                            "ever_amount": terms.get("ever_amount").cloned().unwrap_or(Value::Null),
                            "usdt_amount": terms.get("usdt_amount").cloned().unwrap_or(Value::Null),
                        }
                    })
                })
                .collect(),
        ))
    }
    pub(crate) async fn post_public_offer(
        &self,
        actor: &str,
        key: &str,
        draft: PublicOfferDraft,
    ) -> Result<Value> {
        if !self.public_offers_available() {
            return Err(Error::Disabled);
        }
        let terms = draft.terms(&self.cfg)?;
        let seller = self.wallet(actor, "everscale").await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Serialize replays and the account's active-offer limit.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("otc-offers:{actor}")],
        )
        .await?;
        if let Some(row) = tx
            .query_opt(
                "SELECT * FROM otc_public_offers WHERE actor=$1 AND idempotency_key=$2",
                &[&actor, &key],
            )
            .await?
        {
            if row.get::<_, Value>("body") != terms || row.get::<_, String>("seller_ever") != seller
            {
                return Err(Error::Conflict(
                    "publication key reused with different terms".into(),
                ));
            }
            return Ok(offer(&row));
        }
        let count:i64 = tx.query_one("SELECT count(*) FROM otc_public_offers WHERE actor=$1 AND state='open' AND expires_at>now()", &[&actor]).await?.get(0);
        if count >= 10 {
            return Err(invalid(
                "cancel an existing offer before publishing another",
            ));
        }
        let row = tx.query_one("INSERT INTO otc_public_offers(id,actor,seller_ever,body,idempotency_key) VALUES($1,$2,$3,$4,$5) RETURNING *", &[&Uuid::new_v4(),&actor,&seller,&terms,&key]).await?;
        let result = offer(&row);
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn cancel_public_offer(&self, actor: &str, id: Uuid) -> Result<Value> {
        let client = self.pool.get().await?;
        let row = client.query_opt("UPDATE otc_public_offers SET state='cancelled' WHERE id=$1 AND actor=$2 AND state='open' RETURNING *", &[&id,&actor]).await?.ok_or(Error::NotFound)?;
        Ok(offer(&row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draft() -> PublicOfferDraft {
        PublicOfferDraft {
            direction: Direction::Sell,
            ever_amount: "250".into(),
            usdt_amount: "5".into(),
            usdt_address: format!("0x{}", "a".repeat(40)),
        }
    }
    #[test]
    fn sale_has_exact_mainnet_terms_and_never_needs_usdt_inventory() {
        let cfg = super::super::config::Config::default();
        let terms = draft().terms(&cfg).unwrap();
        assert_eq!(terms["ever_units"], "250000000000");
        assert_eq!(terms["usdt_units"], "5000000");
        assert_eq!(terms["ethereum_chain_id"], 1);
        for value in ["0", "-1", "0.0000000001", "100001", "NaN"] {
            let mut item = draft();
            item.ever_amount = value.into();
            assert!(item.terms(&cfg).is_err(), "{value}");
        }
        for value in ["0", "-1", "0.0000001", "1000001"] {
            let mut item = draft();
            item.usdt_amount = value.into();
            assert!(item.terms(&cfg).is_err(), "{value}");
        }
        let mut item = draft();
        item.usdt_address = format!("0x{}", "0".repeat(40));
        assert!(item.terms(&cfg).is_err());
    }

    #[test]
    fn buy_and_legacy_sell_directions_are_explicit_and_strict() {
        let cfg = super::super::config::Config {
            ethereum_chain_id: 11155111,
            usdt_contract: format!("0x{}", "c".repeat(40)),
            ..Default::default()
        };
        let body = json!({"ever_amount":"100","usdt_amount":"11.889","usdt_address":format!("0x{}","b".repeat(40))});
        let legacy: PublicOfferDraft = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(legacy.terms(&cfg).unwrap()["direction"], "sell");
        let mut buy = body.clone();
        buy["direction"] = json!("buy");
        let buy: PublicOfferDraft = serde_json::from_value(buy).unwrap();
        assert_eq!(buy.terms(&cfg).unwrap()["direction"], "buy");
        assert_eq!(buy.terms(&cfg).unwrap()["ethereum_chain_id"], 11155111);
        assert_eq!(
            buy.terms(&cfg).unwrap()["usdt_contract"],
            format!("0x{}", "c".repeat(40))
        );
        let mut invalid = body;
        invalid["direction"] = json!("anything");
        assert!(serde_json::from_value::<PublicOfferDraft>(invalid).is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn buy_publication_preserves_direction_replay_ownership_and_cancellation() {
        let (mut service, task, _) = super::super::tests::fixture().await;
        let cfg = std::sync::Arc::make_mut(&mut service.cfg);
        cfg.enabled = false;
        cfg.public_offers_enabled = true;
        cfg.everwallet_code_hashes = vec![super::super::everwallet::CODE_HASH.into()];
        let client = service.pool.get().await.unwrap();
        let owner = format!("0:{}", "b".repeat(64));
        client.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES('buyer','everscale',$1,now())", &[&owner]).await.unwrap();
        let mut buy = draft();
        buy.direction = Direction::Buy;
        let offer = service
            .post_public_offer("buyer", "buy-key", buy)
            .await
            .unwrap();
        assert_eq!(offer["terms"]["direction"], "buy");
        assert_eq!(offer["owner_ever"], owner);
        let mut replay = draft();
        replay.direction = Direction::Buy;
        assert_eq!(
            service
                .post_public_offer("buyer", "buy-key", replay)
                .await
                .unwrap(),
            offer
        );
        assert!(matches!(
            service.post_public_offer("buyer", "buy-key", draft()).await,
            Err(Error::Conflict(_))
        ));
        let id = Uuid::parse_str(offer["id"].as_str().unwrap()).unwrap();
        assert!(matches!(
            service.cancel_public_offer("other", id).await,
            Err(Error::NotFound)
        ));
        assert_eq!(
            service.cancel_public_offer("buyer", id).await.unwrap()["state"],
            "cancelled"
        );
        let history = service.public_offer_history().await.unwrap();
        assert_eq!(history.as_array().unwrap().len(), 1);
        assert_eq!(history[0]["id"], offer["id"]);
        assert_eq!(history[0]["terms"]["direction"], "buy");
        assert!(history[0]["terms"].get("usdt_address").is_none());
        assert!(service
            .public_offers(None)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            service.public_offers(Some("buyer")).await.unwrap()[0]["terms"]["direction"],
            "buy"
        );
        for table in ["otc_trades", "otc_attempts", "otc_outbox"] {
            assert_eq!(
                client
                    .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                0
            );
        }
        task.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn publication_is_durable_idempotent_owned_and_separate_from_settlement() {
        let (mut service, task, _) = super::super::tests::fixture().await;
        let cfg = std::sync::Arc::make_mut(&mut service.cfg);
        cfg.enabled = false;
        cfg.public_offers_enabled = true;
        cfg.everwallet_code_hashes = vec![super::super::everwallet::CODE_HASH.into()];
        let client = service.pool.get().await.unwrap();
        client.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES('seller','everscale',$1,now())", &[&format!("0:{}","a".repeat(64))]).await.unwrap();
        let (a, b) = tokio::join!(
            service.post_public_offer("seller", "retry-key", draft()),
            service.post_public_offer("seller", "retry-key", draft())
        );
        let a = a.unwrap();
        assert_eq!(a, b.unwrap());
        assert_eq!(
            service
                .public_offers(None)
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let id = Uuid::parse_str(a["id"].as_str().unwrap()).unwrap();
        let mut changed = draft();
        changed.usdt_amount = "6".into();
        assert!(matches!(
            service
                .post_public_offer("seller", "retry-key", changed)
                .await,
            Err(Error::Conflict(_))
        ));
        assert!(matches!(
            service.cancel_public_offer("other", id).await,
            Err(Error::NotFound)
        ));
        for table in ["otc_trades", "otc_attempts", "otc_outbox"] {
            let count: i64 = client
                .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                .await
                .unwrap()
                .get(0);
            assert_eq!(count, 0, "publication wrote to {table}");
        }
        client
            .execute(
                "UPDATE otc_wallets SET verified_at=now()-make_interval(days=>8) WHERE actor='seller'",
                &[],
            )
            .await
            .unwrap();
        assert!(matches!(
            service
                .post_public_offer("seller", "expired-proof", draft())
                .await,
            Err(Error::VerificationRequired("everscale"))
        ));
        std::sync::Arc::make_mut(&mut service.cfg).public_offers_enabled = false;
        assert!(matches!(
            service
                .post_public_offer("seller", "disabled", draft())
                .await,
            Err(Error::Disabled)
        ));
        assert_eq!(
            service.cancel_public_offer("seller", id).await.unwrap()["state"],
            "cancelled"
        );
        assert!(matches!(
            service.cancel_public_offer("seller", id).await,
            Err(Error::NotFound)
        ));
        assert!(service
            .public_offers(None)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            service
                .public_offers(Some("seller"))
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        task.abort();
    }
}

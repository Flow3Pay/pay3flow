//! Persistent buy and sell advertisements. Publication creates no settlement obligation.
use super::{
    amount,
    error::{invalid, Error, Result},
    model::Direction,
    wallet, Service,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_postgres::Row;
use uuid::Uuid;

/// Wider than `escrow::MAX_EVER_UNITS` on purpose: offers settle peer-to-peer and never reach the escrow factory.
const MAX_EVER_OFFER_UNITS: u128 = 1_000_000_000_000 * 1_000_000_000;
const MAX_USDT_OFFER_UNITS: u128 = 1_000_000 * 1_000_000;
/// One service fee per peer-to-peer deal, denominated in USDT and accrued for later collection.
pub(crate) const P2P_FEE_VERSION: &str = "p2p-3pct-usdt-v1";
pub(crate) const P2P_FEE_RATE: &str = "0.03";
pub(crate) const P2P_FEE_PRECISION: u32 = 6;
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Stablecoin {
    #[default]
    Usdt,
    Usdc,
}
impl Stablecoin {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::Usdt => "USDT",
            Self::Usdc => "USDC",
        }
    }
    pub(crate) fn contract(self) -> &'static str {
        match self {
            Self::Usdt => super::config::USDT,
            Self::Usdc => super::config::USDC,
        }
    }
    pub(crate) fn from_terms(terms: &Value) -> Result<Self> {
        Ok(match terms.get("stablecoin").and_then(Value::as_str) {
            None | Some("USDT") => Self::Usdt,
            Some("USDC") => Self::Usdc,
            _ => return Err(invalid("unsupported offer token")),
        })
    }
}
/// The public chart polls `/offers/history` every few seconds, so the feed is
/// bounded by rows rather than by how many advertisements an account may post.
const MAX_HISTORY_ROWS: i64 = 500;

/// Fee basis uses the six-decimal stablecoin amount; the obligation is always USDT.
/// Rounding up ensures the service never accrues less than three percent.
pub(crate) fn fee_policy(basis: u128) -> Value {
    let fee = basis.saturating_mul(3).saturating_add(99) / 100;
    json!({
        "version": P2P_FEE_VERSION, "rate": P2P_FEE_RATE, "floor": "0",
        "basis_units": basis.to_string(), "fee_units": fee.to_string(),
        "net_units": basis.saturating_sub(fee).to_string(), "precision": P2P_FEE_PRECISION,
        "asset": "USDT", "payer": "stablecoin_sender", "withheld": true,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublicOfferDraft {
    #[serde(default = "legacy_sell_direction")]
    pub direction: Direction,
    #[serde(default)]
    pub stablecoin: Stablecoin,
    pub ever_amount: String,
    pub usdt_amount: String,
    pub usdt_address: String,
    #[serde(default = "default_stablecoin_network")]
    pub stablecoin_network: String,
    #[serde(default)]
    pub stablecoin_asset_id: Option<String>,
}
fn default_stablecoin_network() -> String {
    "ethereum".into()
}
fn legacy_sell_direction() -> Direction {
    Direction::Sell
}
impl PublicOfferDraft {
    fn terms(&self, cfg: &super::config::Config) -> Result<Value> {
        if !cfg.p2p_offers_mainnet() {
            return Err(invalid(
                "public offers support ERC-20 tokens on Ethereum mainnet only",
            ));
        }
        if self.stablecoin == Stablecoin::Usdt && cfg.p2p_usdt() != self.stablecoin.contract() {
            return Err(invalid(
                "Ethereum mainnet must use the canonical USDT contract",
            ));
        }
        let ever = amount::units(&self.ever_amount, 9)?;
        let usdt = amount::units(&self.usdt_amount, 6)?;
        if ever > MAX_EVER_OFFER_UNITS {
            return Err(invalid(
                "EVER offer amount exceeds the 1,000,000,000,000 EVER limit",
            ));
        }
        if usdt > MAX_USDT_OFFER_UNITS {
            return Err(invalid("USDT offer total exceeds the 1,000,000 USDT limit"));
        }
        let network = crate::route_engine::canonical_network_id(&self.stablecoin_network);
        if !crate::route_execution::supports_otc_wallet_network(&network) {
            return Err(invalid("unsupported stablecoin receiving network"));
        }
        let recipient = if network == "ethereum" {
            wallet::address("ethereum", &self.usdt_address)?
        } else {
            self.usdt_address.trim().to_string()
        };
        crate::route_execution::validate_otc_wallet_address(&network, &self.usdt_address)
            .map_err(|_| invalid("invalid stablecoin receiving address"))?;
        if network == "ethereum" && self.usdt_address == format!("0x{}", "0".repeat(40)) {
            return Err(invalid("Ethereum wallet address must be nonzero"));
        }
        if network != "ethereum"
            && self
                .stablecoin_asset_id
                .as_deref()
                .is_none_or(|asset_id| asset_id.trim().is_empty())
        {
            return Err(invalid("stablecoin asset identity is required"));
        }
        let mut policy = fee_policy(usdt);
        if self.stablecoin == Stablecoin::Usdc {
            policy["conversion"] = json!("nominal_usdc_usdt_1_to_1");
        }
        Ok(json!({
            "direction":self.direction, "ever_amount":amount::decimal(ever,9),
            "stablecoin":self.stablecoin.symbol(),
            "usdt_amount":amount::decimal(usdt,6), "ever_units":ever.to_string(),
            "usdt_units":usdt.to_string(), "usdt_address":recipient,
            "stablecoin_network":network,
            "stablecoin_asset_id":self.stablecoin_asset_id,
            "ever_network_id":42, "ethereum_chain_id":1,
            "usdt_contract":self.stablecoin.contract(),
            "stablecoin_contract":self.stablecoin.contract(),
            "fee_policy":policy,
        }))
    }
}
fn offer(row: &Row) -> Value {
    let expires: DateTime<Utc> = row.get("expires_at");
    let state: &str = row.get("state");
    let seller: String = row.get("seller_ever");
    let avatar = super::avatar::identicon(&seller);
    json!({"id":row.get::<_,Uuid>("id"), "seller_ever":seller,
        "seller_avatar":avatar,
        "owner_ever":row.get::<_,String>("seller_ever"),
        "terms":row.get::<_,Value>("body"),
        "state":if state == "open" && expires <= Utc::now() {"expired"} else {state},
        "created_at":row.get::<_,DateTime<Utc>>("created_at"), "expires_at":expires})
}
impl Service {
    /// Whether the desk publishes offers the route engine may quote as a venue.
    pub fn public_offers_available(&self) -> bool {
        self.cfg.public_offers_enabled
            && !self.cfg.demo
            && self.cfg.p2p_offers_mainnet()
            && self.wallet_login_available()
            && self.wallet_login_chain_available("everscale")
    }
    pub(crate) async fn public_offers(&self, owner: Option<&str>) -> Result<Value> {
        let client = self.pool.get().await?;
        let rows = match owner {
            Some(actor) => client.query("SELECT * FROM otc_public_offers WHERE actor=$1 ORDER BY created_at DESC LIMIT 100", &[&actor]).await?,
            None => client.query("SELECT * FROM otc_public_offers WHERE state='open' AND expires_at>now() AND body->>'ethereum_chain_id'='1' AND COALESCE(body->>'stablecoin','USDT') IN ('USDT','USDC') ORDER BY created_at DESC LIMIT 100", &[]).await?,
        };
        Ok(Value::Array(rows.iter().map(offer).collect()))
    }
    /// Public chart history contains publication terms only; closed offers stay
    /// in history without exposing their wallet addresses or current state.
    pub(crate) async fn public_offer_history(&self) -> Result<Value> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, body, created_at FROM (
                   SELECT id, body, created_at FROM otc_public_offers
                   ORDER BY created_at DESC LIMIT $1
                 ) recent ORDER BY created_at ASC",
                &[&MAX_HISTORY_ROWS],
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
                            "stablecoin": terms.get("stablecoin").cloned().unwrap_or(json!("USDT")),
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
        let mut terms = draft.terms(&self.cfg)?;
        let receiving_network = terms["stablecoin_network"]
            .as_str()
            .ok_or_else(|| invalid("offer receiving network is malformed"))?
            .to_string();
        let asset_id = terms["stablecoin_asset_id"].as_str().map(str::to_string);
        let provider = self.near_intents.as_ref().ok_or(Error::RouteUnavailable)?;
        let asset = crate::route_engine::Asset::new(
            terms["stablecoin"].as_str().unwrap_or("USDT"),
            Some(&receiving_network),
        )?;
        let token_result = match asset_id {
            Some(asset_id) => provider.token_for_asset_id(&asset, &asset_id).await,
            None => provider.unique_token_for_asset(&asset).await,
        };
        let token = token_result.map_err(|_| {
            invalid("stablecoin is unavailable or ambiguous on the selected network")
        })?;
        if !token
            .symbol
            .eq_ignore_ascii_case(terms["stablecoin"].as_str().unwrap_or_default())
            || token.decimals != Some(6)
            || (receiving_network == "ethereum"
                && token.contract_address.as_deref().is_none_or(|address| {
                    !address.eq_ignore_ascii_case(
                        terms["stablecoin_contract"].as_str().unwrap_or_default(),
                    )
                }))
        {
            return Err(invalid("selected network token does not match the offer"));
        }
        let contract = token
            .contract_address
            .as_deref()
            .filter(|contract| !contract.trim().is_empty())
            .ok_or_else(|| invalid("selected network token has no wallet transfer contract"))?;
        if super::config::evm_chain_id(&receiving_network).is_some()
            && crate::route_execution::validate_otc_wallet_address(&receiving_network, contract)
                .is_err()
        {
            return Err(invalid("selected network token contract is invalid"));
        }
        terms["stablecoin_contract"] = json!(contract);
        terms["usdt_contract"] = json!(contract);
        terms["stablecoin_asset_id"] = json!(token.asset_id);

        let fee_recipient = self
            .cfg
            .fee_recipients
            .get(&receiving_network)
            .ok_or_else(|| invalid("service-fee recipient is not configured for this network"))?;
        crate::route_execution::validate_otc_wallet_address(&receiving_network, fee_recipient)
            .map_err(|_| invalid("invalid service-fee recipient for the offer network"))?;
        let fee_asset = crate::route_engine::Asset::new("USDT", Some(&receiving_network))?;
        let fee_token = provider
            .unique_token_for_asset(&fee_asset)
            .await
            .map_err(|_| Error::RouteUnavailable)?;
        if fee_token.decimals != Some(6)
            || fee_token
                .contract_address
                .as_deref()
                .is_none_or(|address| address.trim().is_empty())
        {
            return Err(Error::RouteUnavailable);
        }
        let direct_network = self.p2p_direct_evm_network_available(&receiving_network);
        let mut route_available = false;
        for source_token in provider.supported_tokens().await {
            let source_network =
                crate::route_engine::canonical_network_id(&source_token.blockchain);
            if !matches!(
                source_token.symbol.to_ascii_uppercase().as_str(),
                "USDT" | "USDC"
            ) || !crate::route_execution::supports_otc_wallet_network(&source_network)
            {
                continue;
            }
            let Ok(source) = source_token.asset() else {
                continue;
            };
            let payment_route =
                (source == asset && source_token.asset_id == token.asset_id && direct_network)
                    || (source != asset && provider.route_available(&source, &asset).await);
            let fee_route = if source == fee_asset && source_token.asset_id == fee_token.asset_id {
                direct_network
            } else {
                provider.route_available(&source, &fee_asset).await
            };
            if payment_route && fee_route {
                route_available = true;
                break;
            }
        }
        if !route_available {
            return Err(invalid(
                "no supported payment and service-fee route reaches this network",
            ));
        }
        let seller = self.wallet(actor, "everscale").await?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Serialize idempotent replays for this account.
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
            stablecoin: Stablecoin::Usdt,
            ever_amount: "250".into(),
            usdt_amount: "5".into(),
            usdt_address: format!("0x{}", "a".repeat(40)),
            stablecoin_network: "ethereum".into(),
            stablecoin_asset_id: None,
        }
    }
    #[test]
    fn sale_has_exact_mainnet_terms_and_never_needs_usdt_inventory() {
        let cfg = super::super::config::Config::default();
        let terms = draft().terms(&cfg).unwrap();
        assert_eq!(terms["ever_units"], "250000000000");
        assert_eq!(terms["usdt_units"], "5000000");
        assert_eq!(terms["ethereum_chain_id"], 1);
        assert_eq!(terms["fee_policy"]["version"], P2P_FEE_VERSION);
        assert_eq!(terms["fee_policy"]["rate"], "0.03");
        assert_eq!(terms["fee_policy"]["floor"], "0");
        assert_eq!(terms["fee_policy"]["precision"], 6);
        assert_eq!(terms["fee_policy"]["basis_units"], "5000000");
        assert_eq!(terms["fee_policy"]["fee_units"], "150000");
        assert_eq!(terms["fee_policy"]["net_units"], "4850000");
        assert_eq!(terms["fee_policy"]["asset"], "USDT");
        assert_eq!(terms["fee_policy"]["payer"], "stablecoin_sender");
        assert_eq!(terms["fee_policy"]["withheld"], true);
        for value in ["0", "-1", "0.0000000001", "1000000000001", "NaN"] {
            let mut item = draft();
            item.ever_amount = value.into();
            assert!(item.terms(&cfg).is_err(), "{value}");
        }
        let mut item = draft();
        item.ever_amount = "1000000000000".into();
        assert_eq!(
            item.terms(&cfg).unwrap()["ever_units"],
            "1000000000000000000000"
        );
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
    fn offer_terms_bind_the_receiving_network_and_token_identity() {
        let cfg = super::super::config::Config::default();
        let mut item = draft();
        item.stablecoin_network = "NEAR".into();
        item.stablecoin_asset_id = Some("nep141:usdt.near".into());
        item.usdt_address = "alice.near".into();
        let terms = item.terms(&cfg).unwrap();
        assert_eq!(terms["stablecoin_network"], "near");
        assert_eq!(terms["stablecoin_asset_id"], "nep141:usdt.near");
        assert_eq!(terms["usdt_address"], "alice.near");

        item.stablecoin_asset_id = None;
        assert!(item.terms(&cfg).is_err());
        item.stablecoin_asset_id = Some("nep141:usdt.near".into());
        item.usdt_address = "not an account".into();
        assert!(item.terms(&cfg).is_err());
    }

    #[test]
    fn p2p_fee_policy_rounds_up_at_six_decimals_without_a_floor() {
        let tiny = fee_policy(1);
        assert_eq!(tiny["fee_units"], "1");
        assert_eq!(tiny["net_units"], "0");
        let policy = fee_policy(11_889_000);
        assert_eq!(policy["basis_units"], "11889000");
        assert_eq!(policy["fee_units"], "356670");
        assert_eq!(policy["net_units"], "11532330");
        assert_eq!(policy["floor"], "0");
        let mut one = draft();
        one.usdt_amount = "0.000001".into();
        let terms = one.terms(&super::super::config::Config::default()).unwrap();
        assert_eq!(terms["fee_policy"]["basis_units"], "1");
        assert_eq!(terms["fee_policy"]["fee_units"], "1");
        assert_eq!(terms["fee_policy"]["net_units"], "0");
    }

    #[test]
    fn buy_and_legacy_sell_directions_are_explicit_and_strict() {
        let cfg = super::super::config::Config::default();
        let body = json!({"ever_amount":"100","usdt_amount":"11.889","usdt_address":format!("0x{}","b".repeat(40))});
        let legacy: PublicOfferDraft = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(legacy.terms(&cfg).unwrap()["direction"], "sell");
        let mut buy = body.clone();
        buy["direction"] = json!("buy");
        let buy: PublicOfferDraft = serde_json::from_value(buy).unwrap();
        assert_eq!(buy.terms(&cfg).unwrap()["direction"], "buy");
        assert_eq!(buy.terms(&cfg).unwrap()["ethereum_chain_id"], 1);
        assert_eq!(
            buy.terms(&cfg).unwrap()["usdt_contract"],
            super::super::config::USDT
        );
        let mut usdc = body.clone();
        usdc["stablecoin"] = json!("USDC");
        let usdc: PublicOfferDraft = serde_json::from_value(usdc).unwrap();
        assert_eq!(usdc.terms(&cfg).unwrap()["stablecoin"], "USDC");
        assert_eq!(
            usdc.terms(&cfg).unwrap()["stablecoin_contract"],
            super::super::config::USDC
        );
        assert_eq!(usdc.terms(&cfg).unwrap()["fee_policy"]["asset"], "USDT");
        assert_eq!(
            usdc.terms(&cfg).unwrap()["fee_policy"]["conversion"],
            "nominal_usdc_usdt_1_to_1"
        );
        assert_eq!(
            usdc.terms(&cfg).unwrap()["fee_policy"]["fee_units"],
            "356670"
        );
        let mut invalid = body;
        invalid["direction"] = json!("anything");
        assert!(serde_json::from_value::<PublicOfferDraft>(invalid).is_err());
    }

    #[test]
    fn offers_reject_sepolia_even_when_the_peer_contour_is_configured_for_it() {
        let cfg = super::super::config::Config {
            p2p_ethereum_chain_id: Some(11155111),
            p2p_usdt_contract: Some(format!("0x{}", "c".repeat(40))),
            ..Default::default()
        };
        assert!(draft().terms(&cfg).is_err());
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
    async fn one_account_publishes_as_many_offers_as_it_wants() {
        let (mut service, task, _) = super::super::tests::fixture().await;
        let cfg = std::sync::Arc::make_mut(&mut service.cfg);
        cfg.enabled = false;
        cfg.public_offers_enabled = true;
        cfg.everwallet_code_hashes = vec![super::super::everwallet::CODE_HASH.into()];
        let client = service.pool.get().await.unwrap();
        let owner = format!("0:{}", "e".repeat(64));
        client.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES('many','everscale',$1,now())", &[&owner]).await.unwrap();
        let mut ids = Vec::new();
        for index in 0..12 {
            let offer = service
                .post_public_offer("many", &format!("bulk-key-{index}"), draft())
                .await
                .unwrap();
            ids.push(offer["id"].as_str().unwrap().to_string());
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 12);
        assert_eq!(
            service
                .public_offers(Some("many"))
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            12
        );
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

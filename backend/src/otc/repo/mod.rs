pub(crate) mod accounting;
pub(crate) mod avatar;
pub(crate) mod booking;
pub(crate) mod chain;
pub(crate) mod escrow;
pub(crate) mod escrow_observer;
pub(crate) mod federation;
pub(crate) mod history;
pub(crate) mod login;
pub(crate) mod metrics;
pub(crate) mod multisig;
pub(crate) mod operations;
pub(crate) mod p2p;
pub(crate) mod public_offers;
pub(crate) mod reporting;
pub(crate) mod review;
pub(crate) mod route_provider;
pub(crate) mod transfers;
pub(crate) mod volumes;
pub(crate) mod workers;

use super::{
    error::{Error, Result},
    model::Trade,
    Service,
};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use serde_json::{json, Value};
use tokio_postgres::Row;
use uuid::Uuid;

/// How long a wallet signature keeps granting authority over an OTC account.
pub(crate) const WALLET_VERIFICATION_DAYS: i32 = 7;

const INSERT_EVENT: &str = "INSERT INTO otc_events(trade_id,rfq_id,kind,body) VALUES($1,$2,$3,$4)";
const INSERT_OUTBOX: &str = "INSERT INTO otc_outbox(activity_id,actor,body,purpose) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING";
const OUTBOX_BODY: &str = "SELECT body FROM otc_outbox WHERE activity_id=$1";
const SESSION_ACTOR: &str =
    "SELECT actor FROM otc_sessions WHERE id=$1 AND NOT revoked AND expires_at>now()";
const ACTOR_CREDENTIAL: &str =
    "SELECT credential FROM otc_actor_links WHERE actor=$1 AND NOT revoked AND provisioned";
const VERIFIED_WALLET: &str = "SELECT address FROM otc_wallets WHERE actor=$1 AND chain=$2 AND verified_at>now()-make_interval(days=>$3)";
const OWNED_TRADE: &str =
    "SELECT * FROM otc_trades WHERE id=$1 AND (customer_actor=$2 OR desk_actor=$2)";
const PARTICIPANT_TRADES: &str = "SELECT * FROM otc_trades t WHERE customer_actor=$1 OR desk_actor=$1 OR (state NOT IN ('quoted','booking_pending','rejected') AND EXISTS(SELECT 1 FROM otc_login_wallets w WHERE w.actor=$1 AND w.chain='ethereum' AND w.address=t.terms->'escrow'->>'arbitrator')) ORDER BY created_at DESC LIMIT 100";
const ACTOR_RFQS: &str = "SELECT id,actor,direction,input,state,created_at FROM otc_rfq WHERE actor=$1 OR $1=$2 ORDER BY created_at DESC LIMIT 100";
const TRADE_ATTEMPTS: &str = "SELECT id,kind,state,instructions,reference FROM otc_attempts WHERE trade_id=$1 ORDER BY created_at";
const TRADE_EVIDENCE: &str = "SELECT evidence FROM otc_evidence WHERE trade_id=$1";
const TRADE_UPDATES: &str = "SELECT customer_updates FROM otc_incidents WHERE trade_id=$1";

pub(crate) fn trade(row: &Row) -> Result<Trade> {
    Ok(Trade {
        id: row.get("id"),
        rfq_id: row.get("rfq_id"),
        state: row.get("state"),
        terms: serde_json::from_value(row.get("terms"))?,
        offer: row.get("offer"),
        decision: row.get("decision"),
        created_at: row.get("created_at"),
    })
}
pub(crate) async fn event(
    client: &(impl GenericClient + Sync),
    id: Option<Uuid>,
    rfq: Option<Uuid>,
    kind: &str,
    body: &Value,
) -> Result<()> {
    client
        .execute(INSERT_EVENT, &[&id, &rfq, &kind, &body])
        .await?;
    Ok(())
}
pub(crate) async fn enqueue(
    client: &(impl GenericClient + Sync),
    actor: &str,
    body: &Value,
    purpose: &str,
) -> Result<()> {
    let id = super::protocol::text(body, "id")?;
    let inserted = client
        .execute(INSERT_OUTBOX, &[&id, &actor, &body, &purpose])
        .await?;
    if inserted == 0 {
        let existing: Value = client.query_one(OUTBOX_BODY, &[&id]).await?.get(0);
        if existing != *body {
            return Err(Error::Conflict(
                "activity ID reused with different body".into(),
            ));
        }
    }
    Ok(())
}
impl Service {
    pub(crate) async fn session(&self, token: &str) -> Result<(Uuid, String)> {
        let sub = self.jwt.verify(token).map_err(|_| Error::Unauthorized)?;
        let id = sub
            .strip_prefix("otc:")
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(Error::Unauthorized)?;
        let client = self.pool.get().await?;
        let row = client
            .query_opt(SESSION_ACTOR, &[&id])
            .await?
            .ok_or(Error::Unauthorized)?;
        Ok((id, row.get(0)))
    }
    pub(crate) async fn credential(&self, actor: &str) -> Result<String> {
        self.provision_login_actor(actor).await?;
        let client = self.pool.get().await?;
        let row = client
            .query_opt(ACTOR_CREDENTIAL, &[&actor])
            .await?
            .ok_or(Error::Unauthorized)?;
        Ok(self.secrets.decrypt(&row.get::<_, String>(0))?)
    }
    pub(crate) async fn wallet(&self, actor: &str, chain: &'static str) -> Result<String> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                VERIFIED_WALLET,
                &[&actor, &chain, &WALLET_VERIFICATION_DAYS],
            )
            .await?
            .ok_or(Error::VerificationRequired(chain))?;
        Ok(row.get(0))
    }
    pub(crate) fn desk(&self, actor: &str) -> Result<()> {
        if actor != self.cfg.desk_actor {
            Err(Error::Unauthorized)
        } else {
            Ok(())
        }
    }
    pub(crate) async fn owned_trade(&self, actor: &str, id: Uuid) -> Result<Trade> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(OWNED_TRADE, &[&id, &actor])
            .await?
            .ok_or(Error::NotFound)?;
        trade(&row)
    }
    pub(crate) async fn trades(&self, actor: &str) -> Result<Value> {
        let client = self.pool.get().await?;
        let trades = client
            .query(PARTICIPANT_TRADES, &[&actor])
            .await?
            .iter()
            .map(trade)
            .collect::<Result<Vec<_>>>()?;
        let rfqs: Vec<Value> = client.query(ACTOR_RFQS, &[&actor, &self.cfg.desk_actor]).await?.iter().map(|r| json!({"id":r.get::<_,Uuid>(0),"actor":r.get::<_,String>(1),"direction":r.get::<_,String>(2),"input":r.get::<_,String>(3),"state":r.get::<_,String>(4),"created_at":r.get::<_,DateTime<Utc>>(5)})).collect();
        Ok(json!({"trades":trades,"rfqs":rfqs}))
    }
    pub(crate) async fn details(&self, actor: &str, id: Uuid) -> Result<Value> {
        let trade = self.escrow_trade(actor, id).await?;
        let client = self.pool.get().await?;
        let attempts: Vec<Value> = client.query(TRADE_ATTEMPTS, &[&id]).await?.iter().map(|r|json!({"id":r.get::<_,Uuid>(0),"kind":r.get::<_,String>(1),"state":r.get::<_,String>(2),"instructions":r.get::<_,Value>(3),"reference":r.get::<_,Option<String>>(4)})).collect();
        let evidence: Vec<Value> = client
            .query(TRADE_EVIDENCE, &[&id])
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let updates: Vec<Value> = client
            .query(TRADE_UPDATES, &[&id])
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        Ok(
            json!({"trade":trade,"attempts":attempts,"evidence":evidence,"customer_updates":updates}),
        )
    }
}

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
        .execute(
            "INSERT INTO otc_events(trade_id,rfq_id,kind,body) VALUES($1,$2,$3,$4)",
            &[&id, &rfq, &kind, &body],
        )
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
    let inserted = client.execute("INSERT INTO otc_outbox(activity_id,actor,body,purpose) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",&[&id,&actor,&body,&purpose]).await?;
    if inserted == 0 {
        let existing: Value = client
            .query_one("SELECT body FROM otc_outbox WHERE activity_id=$1", &[&id])
            .await?
            .get(0);
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
            .query_opt(
                "SELECT actor FROM otc_sessions WHERE id=$1 AND NOT revoked AND expires_at>now()",
                &[&id],
            )
            .await?
            .ok_or(Error::Unauthorized)?;
        Ok((id, row.get(0)))
    }
    pub(crate) async fn credential(&self, actor: &str) -> Result<String> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT credential FROM otc_actor_links WHERE actor=$1 AND NOT revoked",
                &[&actor],
            )
            .await?
            .ok_or(Error::Unauthorized)?;
        Ok(self.secrets.decrypt(&row.get::<_, String>(0))?)
    }
    pub(crate) async fn wallet(&self, actor: &str, chain: &str) -> Result<String> {
        let client = self.pool.get().await?;
        let row = client.query_opt("SELECT address FROM otc_wallets WHERE actor=$1 AND chain=$2 AND verified_at>now()-interval '24 hours'",&[&actor,&chain]).await?.ok_or(Error::Unauthorized)?;
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
            .query_opt(
                "SELECT * FROM otc_trades WHERE id=$1 AND (customer_actor=$2 OR desk_actor=$2)",
                &[&id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        trade(&row)
    }
    pub(crate) async fn trades(&self, actor: &str) -> Result<Value> {
        let client = self.pool.get().await?;
        let trades = client.query("SELECT * FROM otc_trades WHERE customer_actor=$1 OR desk_actor=$1 ORDER BY created_at DESC LIMIT 100",&[&actor]).await?.iter().map(trade).collect::<Result<Vec<_>>>()?;
        let rfqs: Vec<Value> = client.query("SELECT id,actor,direction,input,state,created_at FROM otc_rfq WHERE actor=$1 OR $1=$2 ORDER BY created_at DESC LIMIT 100",&[&actor,&self.cfg.desk_actor]).await?.iter().map(|r| json!({"id":r.get::<_,Uuid>(0),"actor":r.get::<_,String>(1),"direction":r.get::<_,String>(2),"input":r.get::<_,String>(3),"state":r.get::<_,String>(4),"created_at":r.get::<_,DateTime<Utc>>(5)})).collect();
        Ok(json!({"trades":trades,"rfqs":rfqs}))
    }
    pub(crate) async fn details(&self, actor: &str, id: Uuid) -> Result<Value> {
        let trade = self.owned_trade(actor, id).await?;
        let client = self.pool.get().await?;
        let attempts: Vec<Value> = client.query("SELECT id,kind,state,instructions,reference FROM otc_attempts WHERE trade_id=$1 ORDER BY created_at",&[&id]).await?.iter().map(|r|json!({"id":r.get::<_,Uuid>(0),"kind":r.get::<_,String>(1),"state":r.get::<_,String>(2),"instructions":r.get::<_,Value>(3),"reference":r.get::<_,Option<String>>(4)})).collect();
        let evidence: Vec<Value> = client
            .query(
                "SELECT evidence FROM otc_evidence WHERE trade_id=$1",
                &[&id],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let updates: Vec<Value> = client
            .query(
                "SELECT customer_updates FROM otc_incidents WHERE trade_id=$1",
                &[&id],
            )
            .await?
            .iter()
            .map(|r| r.get(0))
            .collect();
        Ok(
            json!({"trade":trade,"attempts":attempts,"evidence":evidence,"customer_updates":updates}),
        )
    }
}

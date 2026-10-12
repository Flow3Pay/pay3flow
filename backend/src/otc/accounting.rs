use super::{
    amount,
    error::{invalid, Error, Result},
    Service,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntryRequest {
    pub trade_id: Uuid,
    pub kind: String,
    pub amount: String,
    pub reference: String,
    pub idempotency_key: String,
}
impl Service {
    pub(crate) async fn accounting_entry(
        &self,
        actor: &str,
        request: EntryRequest,
    ) -> Result<Value> {
        self.desk(actor)?;
        if request.reference.trim().is_empty()
            || request.idempotency_key.is_empty()
            || request.idempotency_key.len() > 128
        {
            return Err(invalid("accounting evidence and idempotency key required"));
        }
        let (currency, units) = match request.kind.as_str() {
            "fee_collected" | "fee_compensation" | "fee_refunded" | "support_cost"
            | "monitoring_cost" | "exception_cost" => ("USDT", amount::units(&request.amount, 6)?),
            "operator_minutes" => ("minutes", amount::units(&request.amount, 0)?),
            _ => return Err(invalid("unsupported accounting entry")),
        };
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let trade = tx
            .query_opt(
                "SELECT state FROM otc_trades WHERE id=$1 AND desk_actor=$2 FOR UPDATE",
                &[&request.trade_id, &actor],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let policy =
            json!({"reference":request.reference,"idempotency_key":request.idempotency_key});
        if let Some(row)=tx.query_opt("SELECT id,kind,amount::text,policy FROM otc_ledger WHERE trade_id=$1 AND policy->>'idempotency_key'=$2",&[&request.trade_id,&request.idempotency_key]).await? {
            if row.get::<_,String>(1)!=request.kind || row.get::<_,String>(2)!=units.to_string() || row.get::<_,Value>(3)!=policy {return Err(Error::Conflict("accounting key reused with different entry".into()));}
            return Ok(json!({"id":row.get::<_,Uuid>(0)}));
        }
        if ["fee_collected", "fee_compensation", "fee_refunded"].contains(&request.kind.as_str()) {
            let row=tx.query_one("SELECT COALESCE(sum(amount) FILTER(WHERE kind='fee_accrued'),0)::text, COALESCE(sum(amount) FILTER(WHERE kind='fee_collected'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_compensation'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_refunded'),0)::text FROM otc_ledger WHERE trade_id=$1",&[&request.trade_id]).await?;
            let accrued = amount::integer(&row.get::<_, String>(0))?;
            let collected = amount::integer(&row.get::<_, String>(1))?;
            let compensated = amount::integer(&row.get::<_, String>(2))?;
            let refunded = amount::integer(&row.get::<_, String>(3))?;
            let outstanding = match request.kind.as_str() {
                "fee_collected" => {
                    if trade.get::<_, String>(0) != "completed" {
                        return Err(invalid(
                            "only verified completed trades may be invoiced or collected",
                        ));
                    }
                    let valid:i64=tx.query_one("SELECT count(*) FROM otc_evidence WHERE trade_id=$1 AND valid AND kind IN ('payment','payout')",&[&request.trade_id]).await?.get(0);
                    if valid != 2 {
                        return Err(invalid("both valid receipts required for fee collection"));
                    }
                    accrued
                        .saturating_sub(compensated)
                        .saturating_sub(collected.saturating_sub(refunded))
                }
                "fee_compensation" => accrued.saturating_sub(compensated),
                _ => collected.saturating_sub(refunded),
            };
            if units > outstanding {
                return Err(invalid(
                    "entry exceeds unpaid accrued fee; reconcile collections first",
                ));
            }
        }
        let id = Uuid::new_v4();
        tx.execute("INSERT INTO otc_ledger(id,trade_id,kind,currency,amount,policy,authorized_by) VALUES($1,$2,$3,$4,$5::text::numeric,$6,$7)",&[&id,&request.trade_id,&request.kind,&currency,&units.to_string(),&policy,&actor]).await?;
        super::repo::event(
            &tx,
            Some(request.trade_id),
            None,
            "accounting_recorded",
            &json!({"entry_id":id,"kind":request.kind}),
        )
        .await?;
        tx.commit().await?;
        Ok(json!({"id":id}))
    }
}

use super::{
    amount,
    error::{invalid, Error, Result},
    repo, Service,
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
        let trade_state = repo::accounting::lock_desk_trade(&tx, &request.trade_id, actor).await?;
        let policy =
            json!({"reference":request.reference,"idempotency_key":request.idempotency_key});
        if let Some((id, kind, stored_units, stored_policy)) =
            repo::accounting::entry_by_key(&tx, &request.trade_id, &request.idempotency_key).await?
        {
            if kind != request.kind || stored_units != units.to_string() || stored_policy != policy
            {
                return Err(Error::Conflict(
                    "accounting key reused with different entry".into(),
                ));
            }
            return Ok(json!({"id":id}));
        }
        if ["fee_collected", "fee_compensation", "fee_refunded"].contains(&request.kind.as_str()) {
            let (accrued, collected, compensated, refunded) =
                repo::accounting::fee_totals(&tx, &request.trade_id).await?;
            let accrued = amount::integer(&accrued)?;
            let collected = amount::integer(&collected)?;
            let compensated = amount::integer(&compensated)?;
            let refunded = amount::integer(&refunded)?;
            let outstanding = match request.kind.as_str() {
                "fee_collected" => {
                    if trade_state != "completed" {
                        return Err(invalid(
                            "only verified completed trades may be invoiced or collected",
                        ));
                    }
                    let valid =
                        repo::accounting::valid_payment_receipts(&tx, &request.trade_id).await?;
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
        repo::accounting::insert_entry(
            &tx,
            &id,
            &request.trade_id,
            &request.kind,
            currency,
            &units.to_string(),
            &policy,
            actor,
        )
        .await?;
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

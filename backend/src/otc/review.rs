use super::{
    amount,
    error::{invalid, Error, Result},
    model::{Evidence, Leg},
    repo, Service,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

impl Service {
    pub(crate) async fn refund_leg(&self, id: Uuid, mut leg: Leg) -> Result<Leg> {
        let client = self.pool.get().await?;
        if client
            .query_opt(
                "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='payment' AND valid",
                &[&id],
            )
            .await?
            .is_some()
        {
            return Ok(leg);
        }
        let row=client.query_opt("SELECT evidence FROM otc_evidence WHERE trade_id=$1 AND kind='review_payment' AND valid",&[&id]).await?.ok_or_else(||invalid("verified customer receipt required for refund"))?;
        let actual: Evidence = serde_json::from_value(row.get(0))?;
        let allowance =
            amount::integer(&leg.transfer_amount)?.saturating_sub(amount::integer(&leg.amount)?);
        leg.amount = actual.leg.amount;
        leg.transfer_amount = amount::integer(&leg.amount)?
            .checked_add(allowance)
            .ok_or_else(|| invalid("refund amount overflow"))?
            .to_string();
        if actual.leg.chain != leg.chain
            || actual.leg.sender != leg.recipient
            || actual.leg.recipient != leg.sender
        {
            return Err(invalid(
                "review receipt differs from bound participant wallets",
            ));
        }
        Ok(leg)
    }
    pub(crate) async fn review_payment(
        &self,
        id: Uuid,
        attempt: Uuid,
        evidence: &Evidence,
    ) -> Result<()> {
        if !evidence.finalized || !evidence.canonical || !evidence.successful {
            return Err(invalid("review receipt must be finalized and successful"));
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let row = tx
            .query_one("SELECT * FROM otc_trades WHERE id=$1 FOR UPDATE", &[&id])
            .await?;
        let trade = repo::trade(&row)?;
        let booked = trade.terms.leg("payment")?;
        if evidence.chain != booked.chain
            || evidence.leg.chain != booked.chain
            || evidence.leg.sender != booked.sender
            || evidence.leg.recipient != booked.recipient
            || amount::integer(&evidence.leg.amount)? == 0
        {
            return Err(invalid(
                "review receipt does not belong to this customer and desk",
            ));
        }
        if let Some(existing) = tx
            .query_opt(
                "SELECT trade_id,kind,evidence FROM otc_evidence WHERE chain=$1 AND identity=$2",
                &[&evidence.chain, &evidence.identity],
            )
            .await?
        {
            if existing.get::<_, Uuid>(0) != id || existing.get::<_, String>(1) != "review_payment"
            {
                return Err(Error::Conflict(
                    "review evidence already belongs to another leg".into(),
                ));
            }
            repo::event(
                &tx,
                Some(id),
                None,
                "review_evidence_reconciled",
                &json!({"previous":existing.get::<_,Value>(2),"current":evidence}),
            )
            .await?;
            tx.execute("UPDATE otc_evidence SET valid=TRUE,evidence=$3,included_at=$4 WHERE trade_id=$1 AND kind=$2",&[&id,&"review_payment",&serde_json::to_value(evidence)?,&evidence.included_at]).await?;
        } else {
            tx.execute("INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES($1,$2,$3,'review_payment',$4,$5)",&[&evidence.chain,&evidence.identity,&id,&serde_json::to_value(evidence)?,&evidence.included_at]).await?;
        }
        tx.execute(
            "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1",
            &[&id],
        )
        .await?;
        tx.execute(
            "UPDATE otc_attempts SET state='review',updated_at=now() WHERE id=$1",
            &[&attempt],
        )
        .await?;
        repo::event(
            &tx,
            Some(id),
            Some(trade.rfq_id),
            "payment_review",
            &json!({"actual_units":evidence.leg.amount,"chain":evidence.chain,"at":Utc::now()}),
        )
        .await?;
        tx.commit().await?;
        self.incident(&format!("mismatched-payment:{id}"),"P1",Some(id),"payment",json!({"attempt":attempt,"evidence":evidence.identity,"actual_units":evidence.leg.amount})).await
    }
    pub(crate) async fn ethereum_review(
        &self,
        reference: &str,
        booked: &Leg,
        nonce: Option<u128>,
    ) -> Result<Option<Evidence>> {
        let observed = self
            .evm_agree("eth_getTransactionReceipt", json!([reference]))
            .await?;
        let sender = format!("0x{:0>64}", booked.sender.trim_start_matches("0x"));
        let recipient = format!("0x{:0>64}", booked.recipient.trim_start_matches("0x"));
        let logs: Vec<_> = observed["logs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|log| {
                log["address"]
                    .as_str()
                    .is_some_and(|a| a.eq_ignore_ascii_case(super::config::USDT))
                    && log["topics"][0]
                        == "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
                    && log["topics"][1] == sender
                    && log["topics"][2] == recipient
            })
            .collect();
        if logs.len() != 1 {
            return Ok(None);
        }
        let actual = super::chain::number(&logs[0]["data"])?;
        if actual == 0 {
            return Ok(None);
        }
        let leg = Leg {
            amount: actual.to_string(),
            transfer_amount: actual.to_string(),
            ..booked.clone()
        };
        self.ethereum_evidence(reference, &leg, nonce).await
    }
    pub(crate) async fn everscale_review(
        &self,
        reference: &str,
        booked: &Leg,
    ) -> Result<Option<Evidence>> {
        let fields = "id account_addr in_message{id src dst value} out_messages{id src dst value}";
        let query = format!(
            "query($id:String!){{transactions(filter:{{id:{{eq:$id}}}},limit:1){{{fields}}}}}"
        );
        let data = self.ever_agree(&query, json!({"id":reference})).await?;
        let Some(tx) = data["transactions"].as_array().and_then(|a| a.first()) else {
            return Ok(None);
        };
        let receiving = if tx["account_addr"].as_str() == Some(&booked.recipient) {
            reference.to_string()
        } else {
            let messages: Vec<_> = tx["out_messages"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|m| {
                    m["src"].as_str() == Some(&booked.sender)
                        && m["dst"].as_str() == Some(&booked.recipient)
                })
                .collect();
            if tx["account_addr"].as_str() != Some(&booked.sender) || messages.len() != 1 {
                return Ok(None);
            }
            let rows = self
                .ever_agree(
                    "query($id:String!){transactions(filter:{in_msg:{eq:$id}},limit:2){id}}",
                    json!({"id":messages[0]["id"]}),
                )
                .await?;
            let rows = rows["transactions"]
                .as_array()
                .ok_or_else(|| invalid("missing receiving trace"))?;
            if rows.len() != 1 {
                return Ok(None);
            }
            rows[0]["id"]
                .as_str()
                .ok_or_else(|| invalid("missing receiving ID"))?
                .to_string()
        };
        let data=self.ever_agree("query($id:String!){transactions(filter:{id:{eq:$id}},limit:1){balance_delta in_message{value}}}",json!({"id":receiving})).await?;
        let amount = super::chain::number(&data["transactions"][0]["balance_delta"])?;
        let gross = super::chain::number(&data["transactions"][0]["in_message"]["value"])?;
        if amount == 0 || amount > gross {
            return Ok(None);
        }
        self.everscale_evidence(
            &receiving,
            &Leg {
                amount: amount.to_string(),
                transfer_amount: gross.to_string(),
                ..booked.clone()
            },
        )
        .await
    }
}

use super::{
    super::{
        error::{invalid, Result},
        model::{Evidence, Leg},
        Service,
    },
    number,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
const FIELDS:&str="id now status aborted balance_delta total_fees account_addr in_message{id src dst value bounce bounced} credit{credit} compute{success exit_code skipped_reason} action{success result_code} bounce{bounce_type}";
pub(crate) fn receipt(transaction: &Value, leg: &Leg) -> Result<Evidence> {
    let message = &transaction["in_message"];
    if message["src"].as_str() != Some(&leg.sender)
        || message["dst"].as_str() != Some(&leg.recipient)
        || transaction["account_addr"].as_str() != Some(&leg.recipient)
        || number(&message["value"])? != super::super::amount::integer(&leg.transfer_amount)?
    {
        return Err(invalid("EVER receiving message differs from booked leg"));
    }
    let success = message["bounced"] != true
        && transaction["aborted"] != true
        && transaction["bounce"].is_null()
        && transaction["compute"]["success"].as_bool() == Some(true)
        && [Some(0), Some(1)].contains(&transaction["compute"]["exit_code"].as_i64())
        && (transaction["action"].is_null() || transaction["action"]["success"] == true);
    // Net delivery is the actual recipient balance increase, including receiving-chain costs.
    if success
        && number(&transaction["balance_delta"])? != super::super::amount::integer(&leg.amount)?
    {
        return Err(invalid(
            "EVER net delivery differs; receiving-chain costs require review",
        ));
    }
    let identity = message["id"]
        .as_str()
        .ok_or_else(|| invalid("missing EVER message identity"))?;
    let at = Utc
        .timestamp_opt(
            transaction["now"]
                .as_i64()
                .ok_or_else(|| invalid("missing EVER inclusion time"))?,
            0,
        )
        .single()
        .ok_or_else(|| invalid("invalid EVER time"))?;
    Ok(Evidence {
        chain: "everscale".into(),
        identity: identity.into(),
        leg: leg.clone(),
        included_at: at,
        canonical: true,
        finalized: transaction["status"] == 3,
        successful: success,
        raw: transaction.clone(),
    })
}
impl Service {
    pub(crate) async fn everscale_evidence(
        &self,
        reference: &str,
        leg: &Leg,
    ) -> Result<Option<Evidence>> {
        if reference.len() != 64 || !reference.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("invalid EVER transaction hash"));
        }
        let query = format!(
            "query($id:String!){{transactions(filter:{{id:{{eq:$id}}}},limit:1){{{FIELDS}}}}}"
        );
        let data = self.ever_agree(&query, json!({"id":reference})).await?;
        let Some(transaction) = data["transactions"].as_array().and_then(|a| a.first()) else {
            return Ok(None);
        };
        if transaction["account_addr"].as_str() == Some(&leg.recipient) {
            return receipt(transaction, leg).map(Some);
        }
        if transaction["account_addr"].as_str() != Some(&leg.sender) {
            return Err(invalid("EVER reference is not a participant transaction"));
        }
        let messages=self.ever_agree("query($id:String!){transactions(filter:{id:{eq:$id}},limit:1){out_messages{id src dst value bounced}}}",json!({"id":reference})).await?;
        let candidates: Vec<_> = messages["transactions"][0]["out_messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                m["src"].as_str() == Some(&leg.sender)
                    && m["dst"].as_str() == Some(&leg.recipient)
                    && number(&m["value"]).ok()
                        == super::super::amount::integer(&leg.transfer_amount).ok()
            })
            .collect();
        if candidates.len() != 1 {
            return Err(invalid("EVER outgoing message missing or ambiguous"));
        }
        let message = candidates[0]["id"]
            .as_str()
            .ok_or_else(|| invalid("missing EVER outgoing message ID"))?;
        let query = format!(
            "query($id:String!){{transactions(filter:{{in_msg:{{eq:$id}}}},limit:2){{{FIELDS}}}}}"
        );
        let receiving = self.ever_agree(&query, json!({"id":message})).await?;
        let rows = receiving["transactions"]
            .as_array()
            .ok_or_else(|| invalid("missing EVER receiving transactions"))?;
        if rows.len() > 1 {
            return Err(invalid("ambiguous EVER receiving evidence"));
        }
        rows.first().map(|tx| receipt(tx, leg)).transpose()
    }
    pub(crate) async fn locate_everscale(
        &self,
        leg: &Leg,
        instructions: &Value,
    ) -> Result<Option<String>> {
        let at = chrono::DateTime::parse_from_rfc3339(
            instructions["prepared_at"]
                .as_str()
                .ok_or_else(|| invalid("missing handoff time"))?,
        )
        .map_err(|_| invalid("invalid handoff time"))?
        .timestamp()
            + 1;
        let query=format!("query($address:String!,$after:Float!){{transactions(filter:{{account_addr:{{eq:$address}},now:{{ge:$after}}}},orderBy:[{{path:\"now\",direction:ASC}}],limit:100){{{FIELDS}}}}}");
        let rows = self
            .ever_agree(&query, json!({"address":leg.recipient,"after":at}))
            .await?;
        let candidates: Vec<_> = rows["transactions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|tx| {
                tx["in_message"]["src"].as_str() == Some(&leg.sender)
                    && tx["in_message"]["dst"].as_str() == Some(&leg.recipient)
                    && number(&tx["in_message"]["value"]).ok()
                        == super::super::amount::integer(&leg.transfer_amount).ok()
            })
            .collect();
        if candidates.len() == 1 {
            Ok(candidates[0]["id"].as_str().map(str::to_string))
        } else {
            Ok(None)
        }
    }
    pub(crate) async fn everscale_scan(
        &self,
    ) -> Result<(Value, String, chrono::DateTime<Utc>, u64)> {
        let data=self.ever_agree("query{blocks(filter:{workchain_id:{eq:-1}},orderBy:[{path:\"seq_no\",direction:DESC}],limit:1){id seq_no gen_utime global_id}}",json!({})).await?;
        let block = data["blocks"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| invalid("missing Everscale masterchain head"))?;
        if block["global_id"].as_i64() != Some(self.cfg.everscale_global_id) {
            return Err(invalid("wrong Everscale network identity"));
        }
        let at = Utc
            .timestamp_opt(
                block["gen_utime"]
                    .as_i64()
                    .ok_or_else(|| invalid("missing masterchain time"))?,
                0,
            )
            .single()
            .ok_or_else(|| invalid("invalid masterchain time"))?;
        let lag = u64::try_from((Utc::now() - at).num_seconds().max(0))
            .map_err(|_| invalid("Everscale lag overflow"))?;
        if lag > self.cfg.everscale_lag_seconds {
            return Err(invalid("Everscale observer exceeds lag limit"));
        }
        let balance=self.ever_agree("query($address:String!){accounts(filter:{id:{eq:$address}},limit:1){id balance acc_type}}",json!({"address":self.cfg.ever_wallet})).await?;
        let account = balance["accounts"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| invalid("desk EVER wallet missing"))?;
        if account["acc_type"] != 1 {
            return Err(invalid("desk EVER wallet inactive"));
        }
        let seq = block["seq_no"]
            .as_u64()
            .ok_or_else(|| invalid("missing masterchain sequence"))?;
        let client = self.pool.get().await?;
        let previous: Value = client
            .query_one(
                "SELECT checkpoint FROM otc_observers WHERE chain='everscale'",
                &[],
            )
            .await?
            .get(0);
        let start = previous["next_seq"]
            .as_u64()
            .unwrap_or(seq.saturating_sub(1));
        let end = previous["end_seq"]
            .as_u64()
            .unwrap_or((start + 16).min(seq + 1));
        let mut cursor = previous["cursor"].as_str().map(str::to_string);
        let mut pages = 0;
        loop {
            let query="query($start:Int!,$end:Int!,$after:String){blockchain{transactions(master_seq_no_range:{start:$start,end:$end},workchain:0,archive:true,first:100,after:$after){edges{cursor node{id now}}pageInfo{endCursor hasNextPage}}}}";
            let page = self
                .ever_agree(query, json!({"start":start,"end":end,"after":cursor}))
                .await?;
            let info = &page["blockchain"]["transactions"]["pageInfo"];
            if info["hasNextPage"] == false {
                cursor = None;
                break;
            }
            let next = info["endCursor"]
                .as_str()
                .ok_or_else(|| invalid("missing Everscale pagination cursor"))?;
            if cursor.as_deref() == Some(next) {
                return Err(invalid("Everscale pagination stalled"));
            }
            cursor = Some(next.into());
            pages += 1;
            if pages >= 10 {
                break;
            }
        }
        let next_seq = if cursor.is_none() { end } else { start };
        if next_seq <= seq {
            client
                .execute(
                    "UPDATE otc_observers SET checkpoint=$1 WHERE chain='everscale'",
                    &[&json!({"next_seq":next_seq,"cursor":cursor,"end_seq":if cursor.is_some(){Some(end)}else{None}})],
                )
                .await?;
            return Err(invalid("Everscale complete scan has not reached head"));
        }
        Ok((
            json!({"next_seq":next_seq,"cursor":cursor,"hash":block["id"],"timestamp":at}),
            number(&account["balance"])?.to_string(),
            at,
            lag,
        ))
    }
}

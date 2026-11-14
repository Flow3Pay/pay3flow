use super::{
    super::{
        error::{invalid, Result},
        model::{Evidence, Leg},
        wallet, Service,
    },
    number, EvmNet,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Map, Value};
const TRANSFER: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

fn normalize_rpc_value(value: &Value) -> Value {
    match value {
        Value::String(value) if value.starts_with("0x") => {
            Value::String(value.to_ascii_lowercase())
        }
        Value::Array(values) => Value::Array(values.iter().map(normalize_rpc_value).collect()),
        _ => value.clone(),
    }
}

fn project_fields(value: &Value, fields: &[&str]) -> Result<Value> {
    let value = value
        .as_object()
        .ok_or_else(|| invalid("Ethereum RPC observation is malformed"))?;
    let mut projected = Map::new();
    for field in fields {
        let field_value = value
            .get(*field)
            .ok_or_else(|| invalid("Ethereum RPC observation is incomplete"))?;
        projected.insert((*field).to_string(), normalize_rpc_value(field_value));
    }
    Ok(Value::Object(projected))
}

fn receipt_projection(receipt: &Value) -> Result<Value> {
    let mut projected = project_fields(
        receipt,
        &["status", "transactionHash", "blockHash", "blockNumber"],
    )?;
    let logs = receipt["logs"]
        .as_array()
        .ok_or_else(|| invalid("Ethereum receipt logs are missing"))?
        .iter()
        .map(|log| {
            let mut projected = project_fields(log, &["address", "topics", "data", "logIndex"])?;
            projected
                .as_object_mut()
                .ok_or_else(|| invalid("Ethereum receipt log is malformed"))?
                .insert(
                    "removed".into(),
                    Value::Bool(log["removed"].as_bool() == Some(true)),
                );
            Ok(projected)
        })
        .collect::<Result<Vec<_>>>()?;
    projected
        .as_object_mut()
        .ok_or_else(|| invalid("Ethereum receipt is malformed"))?
        .insert("logs".into(), Value::Array(logs));
    Ok(projected)
}

fn transaction_projection(transaction: &Value) -> Result<Value> {
    project_fields(
        transaction,
        &["hash", "from", "to", "input", "nonce", "value"],
    )
}

fn block_projection(block: &Value) -> Result<Value> {
    project_fields(block, &["hash", "number", "timestamp"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_projection_ignores_provider_specific_fields() {
        let first = json!({
            "status":"0x1",
            "transactionHash":"0xAB",
            "blockHash":"0xCD",
            "blockNumber":"0x10",
            "blobGasUsed":"0x0",
            "logs":[{
                "address":"0xAB",
                "topics":["0xCD"],
                "data":"0xEF",
                "logIndex":"0x0",
                "removed":false,
                "transactionIndex":"0x1"
            }]
        });
        let second = json!({
            "status":"0x1",
            "transactionHash":"0xab",
            "blockHash":"0xcd",
            "blockNumber":"0x10",
            "logs":[{
                "address":"0xab",
                "topics":["0xcd"],
                "data":"0xef",
                "logIndex":"0x0"
            }]
        });

        assert_eq!(
            receipt_projection(&first).unwrap(),
            receipt_projection(&second).unwrap()
        );
    }

    #[test]
    fn transaction_projection_ignores_provider_timestamp_but_keeps_transfer_fields() {
        let first = json!({
            "hash":"0xAB",
            "from":"0xCD",
            "to":"0xEF",
            "input":"0x1234",
            "nonce":"0x0",
            "value":"0x0",
            "blockTimestamp":"2026-01-01T00:00:00Z"
        });
        let second = json!({
            "hash":"0xab",
            "from":"0xcd",
            "to":"0xef",
            "input":"0x1234",
            "nonce":"0x0",
            "value":"0x0"
        });

        assert_eq!(
            transaction_projection(&first).unwrap(),
            transaction_projection(&second).unwrap()
        );
    }

    #[test]
    fn receipt_projection_keeps_transfer_logs_in_agreement() {
        let first = json!({
            "status":"0x1",
            "transactionHash":"0xAB",
            "blockHash":"0xCD",
            "blockNumber":"0x10",
            "logs":[{
                "address":"0xAB",
                "topics":["0xCD"],
                "data":"0xEF",
                "logIndex":"0x0"
            }]
        });
        let mut second = first.clone();
        second["logs"][0]["data"] = json!("0x00");

        assert_ne!(
            receipt_projection(&first).unwrap(),
            receipt_projection(&second).unwrap()
        );
    }
}

pub(crate) fn transfer_data(leg: &Leg) -> Result<String> {
    let amount = super::super::amount::integer(&leg.amount)?;
    Ok(format!(
        "0xa9059cbb{:0>64}{amount:064x}",
        leg.recipient.trim_start_matches("0x")
    ))
}
pub(crate) fn receipt(
    receipt: &Value,
    transaction: &Value,
    block: &Value,
    finalized: u128,
    leg: &Leg,
    nonce: Option<u128>,
    contract: &str,
) -> Result<Option<Evidence>> {
    if receipt.is_null() {
        return Ok(None);
    }
    if number(&receipt["status"])? != 1
        || wallet::address("ethereum", transaction["from"].as_str().unwrap_or(""))? != leg.sender
        || wallet::address("ethereum", transaction["to"].as_str().unwrap_or(""))? != contract
        || transaction["input"].as_str() != Some(&transfer_data(leg)?)
        || nonce.is_some_and(|n| number(&transaction["nonce"]).ok() != Some(n))
    {
        return Err(invalid(
            "Ethereum transaction differs from the authorized transfer",
        ));
    }
    if receipt["blockHash"] != block["hash"]
        || transaction["hash"] != receipt["transactionHash"]
        || number(&receipt["blockNumber"])? != number(&block["number"])?
        || transaction
            .get("value")
            .is_some_and(|value| number(value).ok() != Some(0))
    {
        return Err(invalid("Ethereum observation is not canonical"));
    }
    let expected = super::super::amount::integer(&leg.amount)?;
    let sender = format!("0x{:0>64}", leg.sender.trim_start_matches("0x"));
    let recipient = format!("0x{:0>64}", leg.recipient.trim_start_matches("0x"));
    let matching: Vec<&Value> = receipt["logs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|log| {
            log["address"]
                .as_str()
                .is_some_and(|a| a.eq_ignore_ascii_case(contract))
                && log["topics"][0] == TRANSFER
                && log["topics"][1] == sender
                && log["topics"][2] == recipient
                && number(&log["data"]).ok() == Some(expected)
                && log["removed"] != true
        })
        .collect();
    if matching.len() != 1 {
        return Err(invalid("exact USDT Transfer evidence missing or ambiguous"));
    }
    let log = matching[0];
    let hash = receipt["transactionHash"]
        .as_str()
        .ok_or_else(|| invalid("missing transaction hash"))?;
    let index = number(&log["logIndex"])?;
    let block_number = number(&receipt["blockNumber"])?;
    let timestamp =
        i64::try_from(number(&block["timestamp"])?).map_err(|_| invalid("block time overflow"))?;
    Ok(Some(Evidence {
        chain: "ethereum".into(),
        identity: format!("{hash}:{index}"),
        leg: leg.clone(),
        included_at: Utc
            .timestamp_opt(timestamp, 0)
            .single()
            .ok_or_else(|| invalid("invalid block time"))?,
        canonical: true,
        finalized: block_number <= finalized,
        successful: true,
        raw: json!({"receipt":receipt,"transaction":transaction,"block":block}),
    }))
}
impl Service {
    pub(crate) async fn ethereum_evidence(
        &self,
        reference: &str,
        leg: &Leg,
        nonce: Option<u128>,
        net: &EvmNet,
    ) -> Result<Option<Evidence>> {
        if wallet::unhex(reference)?.len() != 32 {
            return Err(invalid("invalid Ethereum hash"));
        }
        self.evm_chain_matches(net).await?;
        let observed = self
            .evm_agree_by(
                net,
                "eth_getTransactionReceipt",
                json!([reference]),
                receipt_projection,
            )
            .await?;
        if observed.is_null() {
            return Ok(None);
        }
        let transaction = self
            .evm_agree_by(
                net,
                "eth_getTransactionByHash",
                json!([reference]),
                transaction_projection,
            )
            .await?;
        let block = self
            .evm_agree_by(
                net,
                "eth_getBlockByNumber",
                json!([observed["blockNumber"], false]),
                block_projection,
            )
            .await?;
        let finalized = self.evm_finalized_height(net).await?;
        receipt(
            &observed,
            &transaction,
            &block,
            finalized,
            leg,
            nonce,
            &net.usdt,
        )
    }
    /// ERC-20 `balanceOf` for any holder on a contour, agreed across that
    /// contour's RPC pair. The block is pinned by the caller so a settlement
    /// check can reuse the exact block it already scanned.
    pub(crate) async fn erc20_balance_of(
        &self,
        net: &EvmNet,
        holder: &str,
        block: &Value,
    ) -> Result<u128> {
        let data = format!(
            "0x70a08231{:0>64}",
            holder.trim_start_matches("0x").to_ascii_lowercase()
        );
        number(
            &self
                .evm_agree(
                    net,
                    "eth_call",
                    json!([{"to": net.usdt, "data": data}, block]),
                )
                .await?,
        )
    }
    pub(crate) async fn ethereum_scan(
        &self,
    ) -> Result<(Value, String, chrono::DateTime<Utc>, u64)> {
        let net = &self.desk_evm();
        self.evm_chain_matches(net).await?;
        let finalized_height = self.evm_finalized_height(net).await?;
        let finalized_tag = json!(format!("0x{finalized_height:x}"));
        let finalized = self
            .evm_agree(net, "eth_getBlockByNumber", json!([finalized_tag, false]))
            .await?;
        let precision = self
            .evm_agree(
                net,
                "eth_call",
                json!([{"to":net.usdt,"data":"0x313ce567"},finalized_tag]),
            )
            .await?;
        if number(&precision)? != 6 {
            return Err(invalid("USDT precision mismatch"));
        }
        let head = self.evm_head(net).await?;
        let lag = u64::try_from(head.saturating_sub(number(&finalized["number"])?))
            .map_err(|_| invalid("Ethereum lag overflow"))?;
        if lag > self.cfg.ethereum_lag_blocks {
            return Err(invalid("Ethereum observer exceeds configured lag"));
        }
        let balance = self
            .erc20_balance_of(net, &self.cfg.usdt_wallet, &finalized["number"])
            .await?;
        let gas = number(
            &self
                .evm_agree(
                    net,
                    "eth_getBalance",
                    json!([self.cfg.usdt_wallet, finalized["number"]]),
                )
                .await?,
        )?;
        if gas < super::super::amount::units(&self.cfg.eth_gas_reserve, 18)? {
            return Err(invalid("desk lacks Ethereum gas reserve"));
        }
        let at = Utc
            .timestamp_opt(
                i64::try_from(number(&finalized["timestamp"])?)
                    .map_err(|_| invalid("block time overflow"))?,
                0,
            )
            .single()
            .ok_or_else(|| invalid("invalid block time"))?;
        Ok((
            json!({"block":finalized["number"],"hash":finalized["hash"],"timestamp":at}),
            balance.to_string(),
            at,
            lag,
        ))
    }
    /// Reads an EVM token balance from the first endpoint that answers. Unlike
    /// settlement reads this is display-only, so one healthy RPC is enough.
    pub(crate) async fn token_balance(
        &self,
        rpcs: &[String],
        contract: &str,
        holder: &str,
    ) -> Result<u128> {
        let data = format!(
            "0x70a08231{:0>64}",
            holder.trim_start_matches("0x").to_ascii_lowercase()
        );
        let mut last = None;
        for url in rpcs.iter().take(2) {
            match self
                .evm_read(
                    url,
                    "eth_call",
                    json!([{"to": contract, "data": data}, "latest"]),
                )
                .await
            {
                Ok(value) => return number(&value),
                Err(error) => last = Some(error),
            }
        }

        Err(last.unwrap_or_else(|| invalid("no EVM endpoint configured for balance reads")))
    }
    pub(crate) async fn locate_ethereum(
        &self,
        leg: &Leg,
        instructions: &Value,
        net: &EvmNet,
    ) -> Result<Option<String>> {
        let from = instructions["checkpoint"]["block"]
            .as_str()
            .ok_or_else(|| invalid("missing payment checkpoint"))?;
        let logs=self.evm_agree(net, "eth_getLogs",json!([{"address":net.usdt,"fromBlock":from,"toBlock":"finalized","topics":[TRANSFER,format!("0x{:0>64}",leg.sender.trim_start_matches("0x")),format!("0x{:0>64}",leg.recipient.trim_start_matches("0x"))]}])).await?;
        let mut references = Vec::new();
        for log in logs.as_array().into_iter().flatten() {
            if number(&log["data"]).ok() != Some(super::super::amount::integer(&leg.amount)?) {
                continue;
            }
            if let Some(hash) = log["transactionHash"].as_str() {
                let tx = self
                    .evm_agree(net, "eth_getTransactionByHash", json!([hash]))
                    .await?;
                if number(&tx["nonce"]).ok()
                    == instructions["nonce"]
                        .as_str()
                        .and_then(|n| u128::from_str_radix(n.trim_start_matches("0x"), 16).ok())
                {
                    references.push(hash.to_string());
                }
            }
        }
        references.sort();
        references.dedup();
        if references.len() == 1 {
            Ok(references.pop())
        } else {
            Ok(None)
        }
    }
}

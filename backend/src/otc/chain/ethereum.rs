use super::{
    super::{
        config::USDT,
        error::{invalid, Result},
        model::{Evidence, Leg},
        wallet, Service,
    },
    number,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
const TRANSFER: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
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
) -> Result<Option<Evidence>> {
    if receipt.is_null() {
        return Ok(None);
    }
    if number(&receipt["status"])? != 1
        || wallet::address("ethereum", transaction["from"].as_str().unwrap_or(""))? != leg.sender
        || wallet::address("ethereum", transaction["to"].as_str().unwrap_or(""))? != USDT
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
                .is_some_and(|a| a.eq_ignore_ascii_case(USDT))
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
    ) -> Result<Option<Evidence>> {
        if wallet::unhex(reference)?.len() != 32 {
            return Err(invalid("invalid Ethereum hash"));
        }
        if number(&self.evm_agree("eth_chainId", json!([])).await?)? != 1 {
            return Err(invalid("wrong Ethereum network"));
        }
        let observed = self
            .evm_agree("eth_getTransactionReceipt", json!([reference]))
            .await?;
        if observed.is_null() {
            return Ok(None);
        }
        let transaction = self
            .evm_agree("eth_getTransactionByHash", json!([reference]))
            .await?;
        let block = self
            .evm_agree(
                "eth_getBlockByNumber",
                json!([observed["blockNumber"], false]),
            )
            .await?;
        let finalized = self
            .evm_agree("eth_getBlockByNumber", json!(["finalized", false]))
            .await?;
        receipt(
            &observed,
            &transaction,
            &block,
            number(&finalized["number"])?,
            leg,
            nonce,
        )
    }
    pub(crate) async fn ethereum_scan(
        &self,
    ) -> Result<(Value, String, chrono::DateTime<Utc>, u64)> {
        if number(&self.evm_agree("eth_chainId", json!([])).await?)? != 1 {
            return Err(invalid("wrong Ethereum network"));
        }
        let precision = self
            .evm_agree(
                "eth_call",
                json!([{"to":USDT,"data":"0x313ce567"},"finalized"]),
            )
            .await?;
        if number(&precision)? != 6 {
            return Err(invalid("USDT precision mismatch"));
        }
        let finalized = self
            .evm_agree("eth_getBlockByNumber", json!(["finalized", false]))
            .await?;
        let head = self.evm_agree("eth_blockNumber", json!([])).await?;
        let lag = u64::try_from(number(&head)?.saturating_sub(number(&finalized["number"])?))
            .map_err(|_| invalid("Ethereum lag overflow"))?;
        if lag > self.cfg.ethereum_lag_blocks {
            return Err(invalid("Ethereum observer exceeds configured lag"));
        }
        let data = format!(
            "0x70a08231{:0>64}",
            self.cfg.usdt_wallet.trim_start_matches("0x")
        );
        let balance = number(
            &self
                .evm_agree(
                    "eth_call",
                    json!([{"to":USDT,"data":data},finalized["number"]]),
                )
                .await?,
        )?;
        let gas = number(
            &self
                .evm_agree(
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
    pub(crate) async fn locate_ethereum(
        &self,
        leg: &Leg,
        instructions: &Value,
    ) -> Result<Option<String>> {
        let from = instructions["checkpoint"]["block"]
            .as_str()
            .ok_or_else(|| invalid("missing payment checkpoint"))?;
        let logs=self.evm_agree("eth_getLogs",json!([{"address":USDT,"fromBlock":from,"toBlock":"finalized","topics":[TRANSFER,format!("0x{:0>64}",leg.sender.trim_start_matches("0x")),format!("0x{:0>64}",leg.recipient.trim_start_matches("0x"))]}])).await?;
        let mut references = Vec::new();
        for log in logs.as_array().into_iter().flatten() {
            if number(&log["data"]).ok() != Some(super::super::amount::integer(&leg.amount)?) {
                continue;
            }
            if let Some(hash) = log["transactionHash"].as_str() {
                let tx = self
                    .evm_agree("eth_getTransactionByHash", json!([hash]))
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

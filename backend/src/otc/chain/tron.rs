//! TRON direct settlement readers.
//!
//! A payment is only credited after a solidity-node transaction info shows one
//! exact TRC-20 `Transfer` log from the booked sender to the booked recipient.
//! Both configured providers must return the same frozen transaction info.
use super::{
    super::{
        amount,
        error::{invalid, Result},
        model::{Evidence, Leg},
        wallet, Service,
    },
    number,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

const TRANSFER: &str = "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

pub(crate) fn call(leg: &Leg, contract: &str) -> Value {
    json!({
        "family": "tron",
        "contract": contract,
        "from": leg.sender,
        "to": leg.recipient,
        "amount": leg.transfer_amount,
        "decimals": 6,
        "fee_limit": 100_000_000u64,
    })
}

/// Raw 20-byte account body used by TRON logs and topics.
pub(crate) fn account_hex(address: &str) -> Result<String> {
    if address.len() == 40 && address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(address.to_ascii_lowercase());
    }
    if let Some(body) = address.strip_prefix("0x") {
        if body.len() == 40 && body.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(body.to_ascii_lowercase());
        }
    }
    crate::route_execution::validate_otc_wallet_address("tron", address)
        .map_err(|_| invalid("invalid TRON address"))?;
    let bytes = bs58::decode(address)
        .into_vec()
        .map_err(|_| invalid("invalid TRON address"))?;
    if bytes.len() != 25 || bytes[0] != 0x41 {
        return Err(invalid("invalid TRON address"));
    }
    Ok(wallet::hex(&bytes[1..21]))
}

/// The last 20 bytes of an ABI topic.
fn topic_account(topic: &Value) -> Option<String> {
    let topic = topic.as_str()?.trim_start_matches("0x");
    (topic.len() == 64 && topic.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| topic[24..].to_ascii_lowercase())
}

fn transfer_amount(log: &Value) -> Option<u128> {
    let data = log["data"].as_str()?.trim_start_matches("0x");
    (data.len() == 64)
        .then(|| u128::from_str_radix(data, 16).ok())
        .flatten()
}

fn matching_log(info: &Value, leg: &Leg, contract: &str) -> Result<(usize, Value)> {
    let sender = account_hex(&leg.sender)?;
    let recipient = account_hex(&leg.recipient)?;
    let contract = account_hex(contract)?;
    let expected = amount::integer(&leg.transfer_amount)?;
    let logs = info["log"]
        .as_array()
        .or_else(|| info["logs"].as_array())
        .cloned()
        .unwrap_or_default();
    let matching: Vec<(usize, Value)> = logs
        .into_iter()
        .enumerate()
        .filter(|(_, log)| {
            log["address"]
                .as_str()
                .is_some_and(|address| account_hex(address).ok().as_deref() == Some(&contract))
                && log["topics"][0].as_str().is_some_and(|topic| {
                    topic
                        .trim_start_matches("0x")
                        .eq_ignore_ascii_case(TRANSFER)
                })
                && topic_account(&log["topics"][1]).as_deref() == Some(&sender)
                && topic_account(&log["topics"][2]).as_deref() == Some(&recipient)
                && transfer_amount(log) == Some(expected)
        })
        .collect();
    if matching.len() != 1 {
        return Err(invalid(
            "exact TRC-20 Transfer evidence missing or ambiguous",
        ));
    }
    Ok(matching.into_iter().next().expect("one match"))
}

fn observed(info: &Value, leg: &Leg, contract: &str, reference: &str) -> Result<Evidence> {
    let identity = info["id"]
        .as_str()
        .filter(|id| id.eq_ignore_ascii_case(reference))
        .ok_or_else(|| invalid("TRON transaction identifier mismatch"))?;
    let timestamp = i64::try_from(number(&info["blockTimeStamp"])?)
        .map_err(|_| invalid("TRON block time overflow"))?;
    let (log_index, log) = matching_log(info, leg, contract)?;
    let receipt = info["receipt"]["result"]
        .as_str()
        .or_else(|| info["result"].as_str())
        .ok_or_else(|| invalid("TRON receipt is missing"))?;
    if !receipt.eq_ignore_ascii_case("SUCCESS") {
        return Ok(Evidence {
            chain: "tron".into(),
            identity: identity.to_ascii_lowercase(),
            leg: leg.clone(),
            included_at: Utc
                .timestamp_opt(timestamp, 0)
                .single()
                .ok_or_else(|| invalid("invalid TRON block time"))?,
            canonical: true,
            finalized: true,
            successful: false,
            raw: json!({"provider":"tron","transaction":info}),
        });
    }
    Ok(Evidence {
        chain: "tron".into(),
        identity: identity.to_ascii_lowercase(),
        leg: leg.clone(),
        included_at: Utc
            .timestamp_opt(timestamp, 0)
            .single()
            .ok_or_else(|| invalid("invalid TRON block time"))?,
        canonical: true,
        finalized: true,
        successful: true,
        raw: json!({"provider":"tron","transaction":info,"log_index":log_index,"log":log}),
    })
}

fn core(evidence: &Evidence) -> Value {
    json!({
        "identity": evidence.identity,
        "included_at": evidence.included_at,
        "successful": evidence.successful,
        "block": evidence.raw["transaction"]["blockNumber"],
        "log_index": evidence.raw["log_index"],
    })
}

impl Service {
    /// Reads a TRC-20 balance with a constant-call trigger from the first
    /// Solidity endpoint that answers.
    pub(crate) async fn tron_token_balance(
        &self,
        endpoints: &[String],
        contract: &str,
        holder: &str,
    ) -> Result<u128> {
        if endpoints.is_empty() {
            return Err(invalid("no TRON endpoint configured for balance reads"));
        }
        account_hex(holder)?;
        account_hex(contract)?;
        let parameter = format!("{:0>64}", holder.trim_start_matches("0x"));
        let mut last = None;
        for endpoint in endpoints.iter().take(2) {
            let response = self
                .http
                .post(format!(
                    "{}/walletsolidity/triggerconstantcontract",
                    endpoint.trim_end_matches('/')
                ))
                .json(&json!({
                    "owner_address": holder,
                    "contract_address": contract,
                    "function_selector": "balanceOf(address)",
                    "parameter": parameter,
                    "visible": true,
                }))
                .send()
                .await;
            match response {
                Ok(response) => match response.error_for_status() {
                    Ok(response) => {
                        let body: Value = response.json().await?;
                        let result = &body["constant_result"][0];
                        if let Some(result) = result.as_str() {
                            return number(&json!(format!("0x{result}")));
                        }
                        last = Some(invalid("TRON balance call returned no result"));
                    }
                    Err(error) => last = Some(error.into()),
                },
                Err(error) => last = Some(error.into()),
            }
        }
        Err(last.unwrap_or_else(|| invalid("TRON balance read failed")))
    }
    pub(crate) async fn tron_evidence(
        &self,
        reference: &str,
        leg: &Leg,
        contract: &str,
        endpoints: &[String],
    ) -> Result<Option<Evidence>> {
        let reference = reference.trim().to_ascii_lowercase();
        if wallet::unhex(&reference)?.len() != 32 {
            return Err(invalid("invalid TRON transaction reference"));
        }
        if endpoints.len() < 2 {
            return Err(invalid("two independent TRON endpoints required"));
        }
        let mut readings = Vec::new();
        for endpoint in endpoints.iter().take(2) {
            let response = self
                .http
                .post(format!(
                    "{}/walletsolidity/gettransactioninfobyid",
                    endpoint.trim_end_matches('/')
                ))
                .json(&json!({"value": reference}))
                .send()
                .await?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let body: Value = response.error_for_status()?.json().await?;
            if body
                .as_object()
                .is_none_or(|object| object.is_empty() || body.get("id").is_none())
            {
                return Ok(None);
            }
            readings.push(observed(&body, leg, contract, &reference)?);
        }
        let first = readings.remove(0);
        if readings.iter().any(|reading| core(reading) != core(&first)) {
            return Err(invalid("independent TRON readings disagree"));
        }
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otc::model::Leg;

    fn leg() -> Leg {
        Leg {
            chain: "tron".into(),
            amount: "1000000".into(),
            transfer_amount: "1000000".into(),
            sender: "TLa2f6VPqDgRE67v1736s7bJ8Ray5wYjU7".into(),
            recipient: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".into(),
        }
    }

    #[test]
    fn tron_logs_use_the_abi_padded_addresses() {
        let value = leg();
        let sender = account_hex(&value.sender).unwrap();
        let recipient = account_hex(&value.recipient).unwrap();
        let info = json!({
            "id": "a".repeat(64),
            "blockNumber": 1,
            "blockTimeStamp": 1_700_000_000_000i64,
            "receipt": {"result": "SUCCESS"},
            "log": [{
                "address": "a".repeat(40),
                "topics": [
                    TRANSFER,
                    format!("0x{:0>64}", sender),
                    format!("0x{:0>64}", recipient),
                ],
                "data": "0x00000000000000000000000000000000000000000000000000000000000f4240",
            }],
        });
        assert!(observed(&info, &value, &"a".repeat(40), &"a".repeat(64)).is_ok());
    }

    #[test]
    fn tron_rejects_ambiguous_or_wrong_logs() {
        let value = leg();
        let sender = account_hex(&value.sender).unwrap();
        let recipient = account_hex(&value.recipient).unwrap();
        let log = json!({
            "address": "a".repeat(40),
            "topics": [TRANSFER, format!("0x{:0>64}", sender), format!("0x{:0>64}", recipient)],
            "data": "0x00000000000000000000000000000000000000000000000000000000000f4241",
        });
        let info = json!({
            "id": "a".repeat(64),
            "blockNumber": 1,
            "blockTimeStamp": 1_700_000_000_000i64,
            "receipt": {"result": "SUCCESS"},
            "log": [log],
        });
        assert!(observed(&info, &value, &"a".repeat(40), &"a".repeat(64)).is_err());
    }

    #[test]
    fn tron_call_carries_the_contract_and_base_units() {
        let value = call(&leg(), &"b".repeat(40));
        assert_eq!(value["family"], "tron");
        assert_eq!(value["amount"], "1000000");
        assert_eq!(value["to"], leg().recipient);
    }
}

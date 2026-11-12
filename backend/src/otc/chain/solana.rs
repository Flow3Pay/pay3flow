//! Solana direct settlement readers.
//!
//! A transfer is credited only when a finalized transaction contains exactly one
//! `transferChecked` of the booked mint, amount and owner pair. Two independent
//! RPCs must agree on the frozen transaction core.
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

pub(crate) fn reference_valid(reference: &str) -> bool {
    bs58::decode(reference)
        .into_vec()
        .is_ok_and(|bytes| bytes.len() == 64)
}

pub(crate) fn call(leg: &Leg, mint: &str) -> Value {
    json!({
        "family": "solana",
        "mint": mint,
        "from": leg.sender,
        "to": leg.recipient,
        "amount": leg.transfer_amount,
        "decimals": 6,
    })
}

fn account_key(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| value["pubkey"].as_str())
}

fn transfer(tx: &Value, leg: &Leg, mint: &str) -> Result<Value> {
    let message = &tx["transaction"]["message"];
    let keys: Vec<&str> = message["accountKeys"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(account_key)
        .collect();
    let post = tx["meta"]["postTokenBalances"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let instructions = message["instructions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut matches = Vec::new();
    for instruction in instructions {
        if instruction["program"].as_str() != Some("spl-token")
            || instruction["parsed"]["type"].as_str() != Some("transferChecked")
        {
            continue;
        }
        let parsed = &instruction["parsed"]["info"];
        if parsed["authority"].as_str() != Some(&leg.sender)
            || parsed["mint"].as_str() != Some(mint)
        {
            continue;
        }
        let amount = parsed["tokenAmount"]["amount"]
            .as_str()
            .or_else(|| parsed["amount"].as_str());
        if amount != Some(&leg.transfer_amount) {
            continue;
        }
        let Some(destination) = parsed["destination"].as_str() else {
            continue;
        };
        let Some(index) = keys.iter().position(|key| *key == destination) else {
            continue;
        };
        let owner = post
            .iter()
            .find(|balance| {
                balance["accountIndex"].as_u64() == Some(index as u64)
                    && balance["mint"].as_str() == Some(mint)
            })
            .and_then(|balance| balance["owner"].as_str());
        if owner != Some(&leg.recipient) {
            continue;
        }
        matches.push(json!({
            "sender": leg.sender,
            "recipient": leg.recipient,
            "destination": destination,
            "mint": mint,
            "amount": amount,
        }));
    }
    if matches.len() != 1 {
        return Err(invalid(
            "exact SPL transferChecked evidence missing or ambiguous",
        ));
    }
    Ok(matches.remove(0))
}

fn observed(
    tx: &Value,
    slot: u128,
    block_time: i64,
    leg: &Leg,
    mint: &str,
    signature: &str,
) -> Result<Evidence> {
    let transfer = transfer(tx, leg, mint)?;
    let failed = !tx["meta"]["err"].is_null();
    Ok(Evidence {
        chain: "solana".into(),
        identity: signature.to_string(),
        leg: leg.clone(),
        included_at: Utc
            .timestamp_opt(block_time, 0)
            .single()
            .ok_or_else(|| invalid("invalid Solana block time"))?,
        canonical: true,
        finalized: true,
        successful: !failed,
        raw: json!({
            "provider": "solana",
            "slot": slot,
            "block_time": block_time,
            "transaction": tx,
            "transfer": transfer,
        }),
    })
}

fn core(evidence: &Evidence) -> Value {
    json!({
        "identity": evidence.identity,
        "included_at": evidence.included_at,
        "successful": evidence.successful,
        "slot": evidence.raw["slot"],
        "transfer": evidence.raw["transfer"],
    })
}

impl Service {
    async fn solana_read(&self, endpoint: &str, method: &str, params: Value) -> Result<Value> {
        let body: Value = self
            .http
            .post(endpoint)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if body.get("error").is_some() || body.get("result").is_none() {
            return Err(invalid("Solana RPC failed"));
        }
        Ok(body["result"].clone())
    }

    /// Reads an SPL token balance from the first RPC that answers, summing the
    /// finalized ATA balances the owner holds for this mint.
    pub(crate) async fn solana_token_balance(
        &self,
        endpoints: &[String],
        owner: &str,
        mint: &str,
    ) -> Result<u128> {
        if endpoints.is_empty() {
            return Err(invalid("no Solana endpoint configured for balance reads"));
        }
        let mut last = None;
        for endpoint in endpoints.iter().take(2) {
            match self
                .solana_read(
                    endpoint,
                    "getTokenAccountsByOwner",
                    json!([owner, {"mint": mint}, {"encoding": "jsonParsed", "commitment": "finalized"}]),
                )
                .await
            {
                Ok(value) => {
                    let mut total = 0u128;
                    for account in value["value"].as_array().into_iter().flatten() {
                        let amount = &account["account"]["data"]["parsed"]["info"]["tokenAmount"]["amount"];
                        if let Some(amount) = amount.as_str() {
                            total = total.saturating_add(amount.parse().unwrap_or(0));
                        }
                    }
                    return Ok(total);
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last.unwrap_or_else(|| invalid("Solana balance read failed")))
    }

    pub(crate) async fn solana_evidence(
        &self,
        reference: &str,
        leg: &Leg,
        mint: &str,
        endpoints: &[String],
    ) -> Result<Option<Evidence>> {
        if !reference_valid(reference) {
            return Err(invalid("invalid Solana signature"));
        }
        if endpoints.len() < 2 {
            return Err(invalid("two independent Solana endpoints required"));
        }
        if !bs58::decode(mint)
            .into_vec()
            .is_ok_and(|bytes| bytes.len() == 32)
        {
            return Err(invalid("invalid SPL token mint"));
        }
        let mut readings = Vec::new();
        for endpoint in endpoints.iter().take(2) {
            let tx = self
                .solana_read(
                    endpoint,
                    "getTransaction",
                    json!([reference, {
                        "encoding": "jsonParsed",
                        "commitment": "finalized",
                        "maxSupportedTransactionVersion": 0,
                    }]),
                )
                .await?;
            if tx.is_null() {
                return Ok(None);
            }
            let signature = tx["transaction"]["signatures"][0]
                .as_str()
                .filter(|signature| *signature == reference)
                .ok_or_else(|| invalid("Solana transaction signature mismatch"))?;
            let slot = number(&tx["slot"])?;
            let finalized = number(
                &self
                    .solana_read(endpoint, "getSlot", json!([{"commitment":"finalized"}]))
                    .await?,
            )?;
            if slot > finalized {
                return Ok(None);
            }
            let block_time = tx["blockTime"]
                .as_i64()
                .ok_or_else(|| invalid("Solana block time is missing"))?;
            readings.push(observed(&tx, slot, block_time, leg, mint, signature)?);
        }
        let first = readings.remove(0);
        if readings.iter().any(|reading| core(reading) != core(&first)) {
            return Err(invalid("independent Solana readings disagree"));
        }
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg() -> Leg {
        Leg {
            chain: "solana".into(),
            amount: "1000000".into(),
            transfer_amount: "1000000".into(),
            sender: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".into(),
            recipient: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into(),
        }
    }

    fn transaction() -> Value {
        let value = leg();
        json!({
            "slot": 42,
            "blockTime": 1_700_000_000,
            "meta": {
                "err": null,
                "postTokenBalances": [{
                    "accountIndex": 2,
                    "mint": "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
                    "owner": value.recipient,
                }],
            },
            "transaction": {
                "signatures": ["5".repeat(64)],
                "message": {
                    "accountKeys": [
                        {"pubkey": value.sender},
                        {"pubkey": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"},
                        {"pubkey": "7YttLkHDoNj9wyDur5P8Qv7h8m8wZk7v8Y5Y5Y5Y5Y5Y"},
                    ],
                    "instructions": [{
                        "program": "spl-token",
                        "parsed": {
                            "type": "transferChecked",
                            "info": {
                                "authority": value.sender,
                                "destination": "7YttLkHDoNj9wyDur5P8Qv7h8m8wZk7v8Y5Y5Y5Y5Y5Y",
                                "mint": "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
                                "tokenAmount": {"amount": "1000000", "decimals": 6},
                            },
                        },
                    }],
                },
            },
        })
    }

    #[test]
    fn solana_requires_exactly_one_matching_checked_transfer() {
        let value = leg();
        let tx = transaction();
        assert!(observed(
            &tx,
            42,
            1_700_000_000,
            &value,
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
            &"5".repeat(64),
        )
        .is_ok());
        let mut wrong = tx.clone();
        wrong["transaction"]["message"]["instructions"][0]["parsed"]["info"]["tokenAmount"]
            ["amount"] = json!("999999");
        assert!(observed(
            &wrong,
            42,
            1_700_000_000,
            &value,
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
            &"5".repeat(64),
        )
        .is_err());
    }

    #[test]
    fn solana_references_are_64_byte_base58_signatures() {
        assert!(reference_valid(&bs58::encode([1u8; 64]).into_string()));
        assert!(!reference_valid(&bs58::encode([1u8; 32]).into_string()));
        assert!(!reference_valid("not-base58"));
    }

    #[test]
    fn solana_call_keeps_the_mint_identity() {
        let value = call(&leg(), "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
        assert_eq!(value["family"], "solana");
        assert_eq!(
            value["mint"],
            "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
        );
    }
}

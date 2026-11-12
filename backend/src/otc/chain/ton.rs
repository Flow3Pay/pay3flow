//! TON direct settlement readers.
//!
//! The wallet returns the hash of the signed external message. The reader walks
//! the wallet -> jetton wallet -> recipient jetton wallet trace, checks the
//! exact jetton amount and owner pair, and only credits once the trace is older
//! than the finality window on two independent TonAPI endpoints.
use super::{
    super::{
        amount,
        error::{invalid, Result},
        model::{Evidence, Leg},
        wallet, Service,
    },
    number,
};
use base64::Engine;
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

const FINALITY_SECONDS: i64 = 300;

pub(crate) fn call(leg: &Leg, jetton: &str) -> Value {
    json!({
        "family": "ton",
        "jetton": jetton,
        "from": leg.sender,
        "to": leg.recipient,
        "amount": leg.transfer_amount,
        "decimals": 6,
    })
}

fn canonical_address(address: &str) -> Option<String> {
    if let Some((workchain, hash)) = address.split_once(':') {
        let workchain: i8 = workchain.parse().ok()?;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        return Some(format!("{workchain}:{}", hash.to_ascii_lowercase()));
    }
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let bytes = URL_SAFE_NO_PAD.decode(address).ok()?;
    if bytes.len() != 36 || !matches!(bytes[0], 0x11 | 0x51) {
        return None;
    }
    Some(format!("{}:{}", bytes[1] as i8, wallet::hex(&bytes[2..34])))
}

fn address(value: &Value) -> Option<&str> {
    value["address"].as_str()
}

fn decoded<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value["decoded_body"].get(key)
}

fn body_amount(value: &Value) -> Option<u128> {
    number(decoded(value, "amount")?).ok()
}

impl Service {
    async fn ton_get(&self, endpoint: &str, path: &str) -> Result<Option<Value>> {
        let response = self
            .http
            .get(format!("{}{path}", endpoint.trim_end_matches('/')))
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(response.error_for_status()?.json().await?))
    }

    async fn ton_transaction(&self, endpoint: &str, hash: &str) -> Result<Option<Value>> {
        self.ton_get(
            endpoint,
            &format!("/v2/blockchain/messages/{hash}/transaction"),
        )
        .await
    }

    async fn ton_jetton_wallet(
        &self,
        endpoint: &str,
        owner: &str,
        master: &str,
    ) -> Result<Option<String>> {
        let Some(body) = self
            .ton_get(endpoint, &format!("/v2/accounts/{owner}/jettons"))
            .await?
        else {
            return Ok(None);
        };
        let expected = canonical_address(master).ok_or_else(|| invalid("invalid jetton master"))?;
        let wallet = body["balances"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|balance| {
                address(&balance["jetton"])
                    .and_then(canonical_address)
                    .as_deref()
                    == Some(expected.as_str())
            })
            .and_then(|balance| address(&balance["wallet_address"]))
            .and_then(canonical_address);
        Ok(wallet)
    }

    fn ton_outgoing<'a>(tx: &'a Value, op: &str) -> Option<&'a Value> {
        tx["out_msgs"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|message| message["decoded_op_name"].as_str() == Some(op))
    }

    async fn ton_observed(
        &self,
        endpoint: &str,
        reference: &str,
        leg: &Leg,
        master: &str,
    ) -> Result<Option<Evidence>> {
        let Some(sender_wallet) = self
            .ton_jetton_wallet(endpoint, &leg.sender, master)
            .await?
        else {
            return Ok(None);
        };
        let Some(wallet_tx) = self.ton_transaction(endpoint, reference).await? else {
            return Ok(None);
        };
        let sender =
            canonical_address(&leg.sender).ok_or_else(|| invalid("invalid TON sender address"))?;
        let recipient = canonical_address(&leg.recipient)
            .ok_or_else(|| invalid("invalid TON recipient address"))?;
        let expected = amount::integer(&leg.transfer_amount)?;

        if canonical_address(address(&wallet_tx["account"]).unwrap_or_default()).as_deref()
            != Some(sender.as_str())
            || wallet_tx["in_msg"]["msg_type"].as_str() != Some("ext_in_msg")
        {
            return Err(invalid("TON handoff is not a signed wallet transaction"));
        }
        let Some(transfer) = Self::ton_outgoing(&wallet_tx, "jetton_transfer") else {
            return Ok(None);
        };
        if address(&transfer["decoded_body"]["destination"])
            .and_then(canonical_address)
            .as_deref()
            != Some(recipient.as_str())
            || body_amount(transfer) != Some(expected)
        {
            return Err(invalid(
                "TON jetton transfer differs from the authorized leg",
            ));
        }
        let Some(transfer_hash) = transfer["hash"].as_str() else {
            return Ok(None);
        };
        let Some(transfer_tx) = self.ton_transaction(endpoint, transfer_hash).await? else {
            return Ok(None);
        };
        if canonical_address(address(&transfer_tx["account"]).unwrap_or_default()).as_deref()
            != Some(sender_wallet.as_str())
            || transfer_tx["in_msg"]["decoded_op_name"].as_str() != Some("jetton_transfer")
            || address(&transfer_tx["in_msg"]["decoded_body"]["destination"])
                .and_then(canonical_address)
                .as_deref()
                != Some(recipient.as_str())
            || body_amount(&transfer_tx["in_msg"]) != Some(expected)
        {
            return Err(invalid(
                "TON jetton wallet transaction differs from the authorized leg",
            ));
        }
        let Some(internal) = Self::ton_outgoing(&transfer_tx, "jetton_internal_transfer") else {
            return Ok(None);
        };
        if address(&internal["decoded_body"]["from"])
            .and_then(canonical_address)
            .as_deref()
            != Some(sender_wallet.as_str())
            || body_amount(internal) != Some(expected)
        {
            return Err(invalid(
                "TON internal jetton transfer differs from the authorized leg",
            ));
        }
        let Some(internal_hash) = internal["hash"].as_str() else {
            return Ok(None);
        };
        let Some(recipient_tx) = self.ton_transaction(endpoint, internal_hash).await? else {
            return Ok(None);
        };
        let Some(recipient_wallet) = self
            .ton_jetton_wallet(endpoint, &leg.recipient, master)
            .await?
        else {
            return Ok(None);
        };
        if canonical_address(address(&recipient_tx["account"]).unwrap_or_default()).as_deref()
            != Some(recipient_wallet.as_str())
            || recipient_tx["in_msg"]["decoded_op_name"].as_str()
                != Some("jetton_internal_transfer")
            || body_amount(&recipient_tx["in_msg"]) != Some(expected)
        {
            return Err(invalid(
                "TON recipient jetton transaction differs from the authorized leg",
            ));
        }

        let times: Vec<i64> = [&wallet_tx, &transfer_tx, &recipient_tx]
            .iter()
            .map(|tx| {
                tx["utime"]
                    .as_i64()
                    .ok_or_else(|| invalid("TON time is missing"))
            })
            .collect::<Result<_>>()?;
        let included = *times
            .iter()
            .min()
            .ok_or_else(|| invalid("TON trace is empty"))?;
        let finalized = times
            .iter()
            .max()
            .is_some_and(|last| Utc::now().timestamp() - *last >= FINALITY_SECONDS);
        let successful = wallet_tx["success"].as_bool() == Some(true)
            && wallet_tx["aborted"].as_bool() != Some(true)
            && transfer_tx["success"].as_bool() == Some(true)
            && recipient_tx["success"].as_bool() == Some(true);
        Ok(Some(Evidence {
            chain: "ton".into(),
            identity: wallet_tx["hash"].as_str().unwrap_or(reference).to_string(),
            leg: leg.clone(),
            included_at: Utc
                .timestamp_opt(included, 0)
                .single()
                .ok_or_else(|| invalid("invalid TON trace time"))?,
            canonical: true,
            finalized,
            successful,
            raw: json!({
                "provider": "ton",
                "wallet_transaction": wallet_tx,
                "transfer_transaction": transfer_tx,
                "recipient_transaction": recipient_tx,
            }),
        }))
    }

    pub(crate) async fn ton_evidence(
        &self,
        reference: &str,
        leg: &Leg,
        master: &str,
        endpoints: &[String],
    ) -> Result<Option<Evidence>> {
        if wallet::unhex(reference)?.len() != 32 {
            return Err(invalid("invalid TON message reference"));
        }
        if endpoints.len() < 2 {
            return Err(invalid("two independent TON endpoints required"));
        }
        if crate::route_execution::validate_otc_wallet_address("ton", master).is_err() {
            return Err(invalid("invalid jetton master"));
        }
        let mut readings = Vec::new();
        for endpoint in endpoints.iter().take(2) {
            let Some(evidence) = self.ton_observed(endpoint, reference, leg, master).await? else {
                return Ok(None);
            };
            readings.push(evidence);
        }
        let first = readings.remove(0);
        let core = |evidence: &Evidence| {
            json!({
                "identity": evidence.identity,
                "included_at": evidence.included_at,
                "finalized": evidence.finalized,
                "successful": evidence.successful,
                "wallet": evidence.raw["wallet_transaction"]["hash"],
                "transfer": evidence.raw["transfer_transaction"]["hash"],
                "recipient": evidence.raw["recipient_transaction"]["hash"],
            })
        };
        if readings.iter().any(|reading| core(reading) != core(&first)) {
            return Err(invalid("independent TON readings disagree"));
        }
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ton_addresses_compare_in_raw_form() {
        let raw = "0:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(canonical_address(raw).as_deref(), Some(raw));
        let friendly = "UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ";
        assert_eq!(
            canonical_address(friendly).as_deref(),
            Some("0:0000000000000000000000000000000000000000000000000000000000000000")
        );
    }

    #[test]
    fn ton_body_amounts_are_quantum_strings() {
        let message = json!({"decoded_body": {"amount": "1000000"}});
        assert_eq!(body_amount(&message), Some(1_000_000));
        assert_eq!(body_amount(&json!({"decoded_body": {}})), None);
    }
}

pub(crate) mod ethereum;
pub(crate) mod everscale;
use super::{
    error::{invalid, Result},
    Service,
};
use serde_json::{json, Value};

pub(crate) fn number(value: &Value) -> Result<u128> {
    if let Some(n) = value.as_u64() {
        return Ok(u128::from(n));
    }
    let value = value
        .as_str()
        .ok_or_else(|| invalid("chain amount is missing"))?;
    if let Some(hex) = value.strip_prefix("0x") {
        u128::from_str_radix(hex, 16).map_err(|_| invalid("chain amount overflow"))
    } else {
        value.parse().map_err(|_| invalid("invalid chain amount"))
    }
}
impl Service {
    pub(crate) async fn evm_agree(&self, method: &str, params: Value) -> Result<Value> {
        if self.cfg.ethereum_rpc.len() < 2 {
            return Err(invalid("two independent Ethereum RPCs required"));
        }
        let query = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
        let mut values = Vec::new();
        for url in self.cfg.ethereum_rpc.iter().take(2) {
            let body: Value = self
                .http
                .post(url)
                .json(&query)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if body.get("error").is_some() || body.get("result").is_none() {
                return Err(invalid("Ethereum RPC failed"));
            }
            values.push(body["result"].clone());
        }
        if values[0] != values[1] {
            return Err(invalid("independent Ethereum RPC readings disagree"));
        }
        Ok(values.remove(0))
    }
    pub(crate) async fn ever_agree(&self, query: &str, variables: Value) -> Result<Value> {
        if self.cfg.everscale_graphql.len() < 2 {
            return Err(invalid("two independent Everscale endpoints required"));
        }
        let mut values = Vec::new();
        for url in self.cfg.everscale_graphql.iter().take(2) {
            let body: Value = self
                .http
                .post(url)
                .json(&json!({"query":query,"variables":variables}))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if body.get("errors").is_some() || body.get("data").is_none() {
                return Err(invalid("Everscale observer query failed"));
            }
            values.push(body["data"].clone());
        }
        if values[0] != values[1] {
            return Err(invalid("independent Everscale readings disagree"));
        }
        Ok(values.remove(0))
    }
}

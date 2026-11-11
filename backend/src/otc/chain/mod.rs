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
/// One Ethereum contour: the RPC pair that reads it and its ERC-20 token.
/// The desk and peer settlement may point at different networks.
#[derive(Clone)]
pub(crate) struct EvmNet {
    pub rpcs: Vec<String>,
    pub chain_id: u64,
    pub usdt: String,
}
impl Service {
    pub(crate) fn desk_evm(&self) -> EvmNet {
        EvmNet {
            rpcs: self.cfg.ethereum_rpc.clone(),
            chain_id: self.cfg.ethereum_chain_id,
            usdt: self.cfg.usdt_contract.clone(),
        }
    }
    pub(crate) fn p2p_evm(&self) -> EvmNet {
        EvmNet {
            rpcs: self.cfg.p2p_rpc().to_vec(),
            chain_id: self.cfg.p2p_chain(),
            usdt: self.cfg.p2p_usdt().to_string(),
        }
    }
    async fn evm_read(&self, url: &str, method: &str, params: Value) -> Result<Value> {
        let query = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
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
        Ok(body["result"].clone())
    }
    pub(crate) async fn evm_agree(
        &self,
        net: &EvmNet,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        if net.rpcs.len() < 2 {
            return Err(invalid("two independent Ethereum RPCs required"));
        }
        let mut values = Vec::new();
        for url in net.rpcs.iter().take(2) {
            values.push(self.evm_read(url, method, params.clone()).await?);
        }
        if values[0] != values[1] {
            return Err(invalid("independent Ethereum RPC readings disagree"));
        }
        Ok(values.remove(0))
    }
    pub(crate) async fn evm_chain_matches(&self, net: &EvmNet) -> Result<()> {
        if number(&self.evm_agree(net, "eth_chainId", json!([])).await?)?
            != u128::from(net.chain_id)
        {
            return Err(invalid("wrong Ethereum network"));
        }
        Ok(())
    }
    /// Finalized heights legitimately differ while providers sit at different heads, so the
    /// most conservative reading is used instead of requiring agreement.
    pub(crate) async fn evm_finalized_height(&self, net: &EvmNet) -> Result<u128> {
        let mut height: Option<u128> = None;
        for url in net.rpcs.iter().take(2) {
            let block = self
                .evm_read(url, "eth_getBlockByNumber", json!(["finalized", false]))
                .await?;
            let seen = number(&block["number"])?;
            height = Some(height.map_or(seen, |current| current.min(seen)));
        }
        height.ok_or_else(|| invalid("no Ethereum RPC endpoint configured"))
    }
    /// Heads also differ between providers. The highest observed head is used so the
    /// lag guard stays conservative instead of failing on a one-block difference.
    pub(crate) async fn evm_head(&self, net: &EvmNet) -> Result<u128> {
        let mut head: Option<u128> = None;
        for url in net.rpcs.iter().take(2) {
            let seen = number(&self.evm_read(url, "eth_blockNumber", json!([])).await?)?;
            head = Some(head.map_or(seen, |current| current.max(seen)));
        }
        head.ok_or_else(|| invalid("no Ethereum RPC endpoint configured"))
    }
    /// A concrete finalized block tag shared by every provider. Reads pinned to the
    /// `finalized` tag can disagree while providers advance at different speeds.
    pub(crate) async fn evm_finalized_tag(&self, net: &EvmNet) -> Result<Value> {
        Ok(json!(format!(
            "0x{:x}",
            self.evm_finalized_height(net).await?
        )))
    }
    /// Pending nonces depend on which transactions each provider has admitted to its mempool.
    pub(crate) async fn evm_pending_nonce(&self, net: &EvmNet, address: &str) -> Result<u128> {
        let mut nonce: Option<u128> = None;
        for url in net.rpcs.iter().take(2) {
            let seen = number(
                &self
                    .evm_read(url, "eth_getTransactionCount", json!([address, "pending"]))
                    .await?,
            )?;
            nonce = Some(nonce.map_or(seen, |current| current.max(seen)));
        }
        nonce.ok_or_else(|| invalid("no Ethereum RPC endpoint configured"))
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

use super::error::{invalid, Error, Result};
use serde::{Deserialize, Serialize};

pub(crate) const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub(crate) const PROFILE: &str = "pay3flow-otc-fep0837-v1";
fn default_evm_chain() -> u64 {
    1
}
fn default_usdt_contract() -> String {
    USDT.into()
}
fn default_ever_fee_reserve() -> String {
    "0.05".into()
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub enabled: bool,
    pub public_offers_enabled: bool,
    #[serde(default)]
    pub p2p_enabled: bool,
    pub demo: bool,
    #[serde(default = "default_evm_chain")]
    pub ethereum_chain_id: u64,
    #[serde(default = "default_usdt_contract")]
    pub usdt_contract: String,
    #[serde(default = "default_ever_fee_reserve")]
    pub p2p_ever_fee_reserve: String,
    /// Peer settlement network. Defaults to the desk network when unset, so a
    /// deployment can trade offers on a testnet while the desk keeps mainnet.
    #[serde(default)]
    pub p2p_ethereum_chain_id: Option<u64>,
    #[serde(default)]
    pub p2p_usdt_contract: Option<String>,
    #[serde(default)]
    pub p2p_ethereum_rpc: Vec<String>,
    pub origin: String,
    pub relay_origin: String,
    pub matcher_actor: String,
    pub desk_actor: String,
    pub desk_name: String,
    pub ethereum_rpc: Vec<String>,
    pub everscale_graphql: Vec<String>,
    pub everwallet_code_hashes: Vec<String>,
    /// Server-owned path to ever-cli. Enables the native EVER multisig workspace.
    pub everscale_multisig_backend: Option<String>,
    pub ever_resource: String,
    pub usdt_resource: String,
    pub ever_wallet: String,
    pub usdt_wallet: String,
    pub ever_gas_reserve: String,
    pub eth_gas_reserve: String,
    pub min_usdt: String,
    pub max_usdt: String,
    pub quote_seconds: u32,
    pub payment_seconds: u32,
    pub payout_seconds: u32,
    pub ethereum_lag_blocks: u64,
    pub everscale_lag_seconds: u64,
    pub everscale_global_id: i64,
    pub staffed_utc_start: u32,
    pub staffed_utc_end: u32,
    pub support_owner: String,
    pub incident_owner: String,
    pub incident_backup: String,
    pub refund_policy: String,
    pub net_delivery_policy: String,
    pub operating_requirements_evidence: String,
    pub alert_webhook: Option<String>,
    pub heartbeat_url: Option<String>,
    pub live_gate_evidence: Option<String>,
    pub escrow: Option<super::escrow::EscrowPolicy>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            public_offers_enabled: false,
            p2p_enabled: false,
            demo: false,
            ethereum_chain_id: default_evm_chain(),
            usdt_contract: default_usdt_contract(),
            p2p_ever_fee_reserve: default_ever_fee_reserve(),
            p2p_ethereum_chain_id: None,
            p2p_usdt_contract: None,
            p2p_ethereum_rpc: Vec::new(),
            origin: String::new(),
            relay_origin: String::new(),
            matcher_actor: String::new(),
            desk_actor: String::new(),
            desk_name: String::new(),
            ethereum_rpc: Vec::new(),
            everscale_graphql: Vec::new(),
            everwallet_code_hashes: Vec::new(),
            everscale_multisig_backend: None,
            ever_resource: String::new(),
            usdt_resource: String::new(),
            ever_wallet: String::new(),
            usdt_wallet: String::new(),
            ever_gas_reserve: "1".into(),
            eth_gas_reserve: "0.01".into(),
            min_usdt: "100".into(),
            max_usdt: "10000".into(),
            quote_seconds: 120,
            payment_seconds: 600,
            payout_seconds: 900,
            ethereum_lag_blocks: 100,
            everscale_lag_seconds: 60,
            everscale_global_id: 42,
            staffed_utc_start: 9,
            staffed_utc_end: 18,
            support_owner: String::new(),
            incident_owner: String::new(),
            incident_backup: String::new(),
            refund_policy: String::new(),
            net_delivery_policy: String::new(),
            operating_requirements_evidence: String::new(),
            alert_webhook: None,
            heartbeat_url: None,
            live_gate_evidence: None,
            escrow: None,
        }
    }
}
impl Config {
    pub fn load(origin: &str) -> anyhow::Result<Self> {
        let mut cfg: Self = match std::env::var("OTC_CONFIG_FILE") {
            Ok(path) => serde_json::from_slice(&std::fs::read(path)?)?,
            Err(_) => Self::default(),
        };
        cfg.origin = origin.trim_end_matches('/').to_string();
        cfg.usdt_contract = cfg.usdt_contract.to_ascii_lowercase();
        cfg.p2p_usdt_contract = cfg.p2p_usdt_contract.map(|c| c.to_ascii_lowercase());
        // A view-only offers deployment must keep booting with whatever network
        // material it already has; only the flags that move funds are enforced.
        if cfg.enabled || cfg.p2p_enabled {
            cfg.validate_network()?;
        }
        if cfg.enabled {
            cfg.validate()?;
        }
        Ok(cfg)
    }
    pub fn validate_network(&self) -> Result<()> {
        self.check_evm_network(self.ethereum_chain_id, &self.usdt_contract)?;
        if let Some(contract) = &self.p2p_usdt_contract {
            self.check_evm_network(self.p2p_chain(), contract)?;
        }
        super::amount::units(&self.p2p_ever_fee_reserve, 9)?;
        Ok(())
    }
    fn check_evm_network(&self, chain_id: u64, contract: &str) -> Result<()> {
        let canonical = super::wallet::address("ethereum", contract)?;
        if canonical != contract || chain_id == 0 {
            return Err(invalid("Ethereum network settings must be canonical"));
        }
        if chain_id == 1 && canonical != USDT {
            return Err(invalid(
                "Ethereum mainnet must use the canonical USDT contract",
            ));
        }
        Ok(())
    }
    pub fn p2p_chain(&self) -> u64 {
        self.p2p_ethereum_chain_id.unwrap_or(self.ethereum_chain_id)
    }
    pub fn p2p_usdt(&self) -> &str {
        self.p2p_usdt_contract
            .as_deref()
            .unwrap_or(&self.usdt_contract)
    }
    pub fn p2p_rpc(&self) -> &[String] {
        if self.p2p_ethereum_rpc.is_empty() {
            &self.ethereum_rpc
        } else {
            &self.p2p_ethereum_rpc
        }
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(policy) = &self.escrow {
            policy.validate()?;
        }
        self.validate_network()?;
        for value in [
            &self.relay_origin,
            &self.matcher_actor,
            &self.desk_actor,
            &self.desk_name,
            &self.ever_resource,
            &self.usdt_resource,
            &self.support_owner,
            &self.incident_owner,
            &self.incident_backup,
            &self.refund_policy,
            &self.net_delivery_policy,
        ] {
            if value.trim().is_empty() {
                return Err(invalid("OTC configuration is incomplete"));
            }
        }
        super::wallet::address("ethereum", &self.usdt_wallet)?;
        super::wallet::address("everscale", &self.ever_wallet)?;
        if self.ever_resource == self.usdt_resource
            || self.quote_seconds == 0
            || self.payment_seconds == 0
            || self.payout_seconds == 0
            || self.staffed_utc_start >= self.staffed_utc_end
            || self.staffed_utc_end > 24
        {
            return Err(invalid("invalid OTC route policy"));
        }
        let min = super::amount::units(&self.min_usdt, 6)?;
        if min > super::amount::units(&self.max_usdt, 6)? {
            return Err(invalid("invalid OTC trade limits"));
        }
        super::amount::units(&self.ever_gas_reserve, 9)?;
        super::amount::units(&self.eth_gas_reserve, 18)?;
        if !self.demo {
            for endpoints in [&self.ethereum_rpc, &self.everscale_graphql] {
                if endpoints.len() < 2 || endpoints[0] == endpoints[1] {
                    return Err(invalid("two independent chain endpoints required"));
                }
                for endpoint in endpoints {
                    if !endpoint.starts_with("https://") {
                        return Err(invalid("live endpoints must use HTTPS"));
                    }
                }
            }
            for endpoint in [
                &self.origin,
                &self.relay_origin,
                &self.matcher_actor,
                &self.desk_actor,
                &self.ever_resource,
                &self.usdt_resource,
            ] {
                if !endpoint.starts_with("https://") {
                    return Err(invalid("live identities must use HTTPS"));
                }
            }
            for endpoint in [&self.alert_webhook, &self.heartbeat_url]
                .into_iter()
                .flatten()
            {
                if !endpoint.starts_with("https://") {
                    return Err(invalid("live alert routes must use HTTPS"));
                }
            }
            if super::wallet::address("ethereum", &self.usdt_wallet)? != self.usdt_wallet
                || super::wallet::address("everscale", &self.ever_wallet)? != self.ever_wallet
            {
                return Err(invalid(
                    "configured wallet addresses must be canonical lowercase",
                ));
            }
            if self.everscale_global_id != 42 {
                return Err(invalid("Everscale mainnet global ID must be 42"));
            }
        }
        if !self.demo
            && (self.ethereum_rpc.len() < 2
                || self.everscale_graphql.len() < 2
                || self.everwallet_code_hashes.is_empty()
                || self.ethereum_lag_blocks == 0
                || self.everscale_lag_seconds == 0
                || self.alert_webhook.is_none()
                || self.heartbeat_url.is_none()
                || self
                    .live_gate_evidence
                    .as_ref()
                    .is_none_or(|s| s.is_empty())
                || self.operating_requirements_evidence.is_empty())
        {
            return Err(Error::Disabled);
        }
        Ok(())
    }
}

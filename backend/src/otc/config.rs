use super::error::{invalid, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub(crate) const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
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
    /// Peer settlement network. OTC settlement currently supports Ethereum mainnet only.
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
    #[serde(skip)]
    pub fee_recipients: BTreeMap<String, String>,
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
            fee_recipients: BTreeMap::new(),
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
        cfg.fee_recipients = match std::env::var("OTC_FEE_RECIPIENTS") {
            Ok(value) if value.trim().is_empty() => BTreeMap::new(),
            Ok(value) => parse_fee_recipients(&value)?,
            Err(std::env::VarError::NotPresent) => BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };
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
        self.check_evm_network(self.p2p_chain(), self.p2p_usdt())?;
        super::amount::units(&self.p2p_ever_fee_reserve, 9)?;
        Ok(())
    }
    fn check_evm_network(&self, chain_id: u64, contract: &str) -> Result<()> {
        let canonical = super::wallet::address("ethereum", contract)?;
        if canonical != contract || chain_id != 1 {
            return Err(invalid(
                "OTC settlement currently supports Ethereum mainnet only",
            ));
        }
        if canonical != USDT {
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
    pub fn p2p_offers_mainnet(&self) -> bool {
        self.p2p_chain() == 1 && self.p2p_usdt() == USDT
    }
    pub fn p2p_rpc(&self) -> &[String] {
        if self.p2p_ethereum_rpc.is_empty() {
            &self.ethereum_rpc
        } else {
            &self.p2p_ethereum_rpc
        }
    }
}

fn parse_fee_recipients(raw: &str) -> anyhow::Result<BTreeMap<String, String>> {
    let entries: Vec<String> = serde_json::from_str(raw).map_err(|error| {
        anyhow::anyhow!("OTC_FEE_RECIPIENTS must be a JSON string array: {error}")
    })?;
    let mut recipients = BTreeMap::new();
    for entry in entries {
        let (network, address) = entry
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("fee recipient must use network=address format"))?;
        let network = network.trim().to_ascii_lowercase();
        let address = address.trim();
        if !valid_fee_recipient(&network, address) {
            anyhow::bail!("invalid OTC fee recipient for network {network}");
        }
        if recipients
            .insert(network.clone(), address.to_string())
            .is_some()
        {
            anyhow::bail!("duplicate OTC fee recipient for network {network}");
        }
    }
    Ok(recipients)
}

fn valid_fee_recipient(network: &str, address: &str) -> bool {
    let evm = matches!(
        network,
        "ethereum"
            | "avalanche-c"
            | "polygon-pos"
            | "bnb-smart-chain"
            | "arbitrum-one"
            | "base"
            | "optimism"
    );
    if evm {
        return address.len() == 42
            && address.starts_with("0x")
            && address[2..].bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    match network {
        "tron" => {
            address.len() == 34
                && address.starts_with('T')
                && address.bytes().all(|byte| {
                    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&byte)
                })
        }
        "near" => {
            address.len() == 64 && address.bytes().all(|byte| byte.is_ascii_hexdigit())
                || address.ends_with(".near")
                    && (2..=64).contains(&address.len())
                    && address.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'-' | b'_')
                    })
        }
        "bitcoin" => address.strip_prefix("bc1").is_some_and(|payload| {
            (42..=62).contains(&address.len())
                && payload
                    .bytes()
                    .all(|byte| b"023456789acdefghjklmnpqrstuvwxyz".contains(&byte))
        }),
        "solana" => {
            (32..=44).contains(&address.len())
                && address.bytes().all(|byte| {
                    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&byte)
                })
        }
        "sei" => {
            address.starts_with("sei1")
                && (8..=90).contains(&address.len())
                && address
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        }
        "ton" => {
            address.len() == 48
                && (address.starts_with("EQ") || address.starts_with("UQ"))
                && address
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        }
        "aptos" => {
            address.len() == 66
                && address.starts_with("0x")
                && address[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
        }
        _ => false,
    }
}

impl Config {
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
        if self
            .fee_recipients
            .iter()
            .any(|(network, address)| !valid_fee_recipient(network, address))
        {
            return Err(invalid("invalid OTC fee recipient configuration"));
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

#[cfg(test)]
mod config_tests {
    use super::{parse_fee_recipients, Config, USDT};

    #[test]
    fn parses_all_operator_fee_recipients() {
        let entries = r#"[
            "tron=T111111111111111111111111111111111",
            "near=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bitcoin=bc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "ethereum=0x1111111111111111111111111111111111111111",
            "solana=11111111111111111111111111111111",
            "avalanche-c=0x1111111111111111111111111111111111111111",
            "sei=sei1aaaaaaaaaaaaaaaaaaaa",
            "polygon-pos=0x1111111111111111111111111111111111111111",
            "ton=UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "bnb-smart-chain=0x1111111111111111111111111111111111111111",
            "arbitrum-one=0x1111111111111111111111111111111111111111",
            "base=0x1111111111111111111111111111111111111111",
            "aptos=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ]"#;
        let recipients = parse_fee_recipients(entries).unwrap();
        assert_eq!(recipients.len(), 13);
        assert_eq!(
            recipients.get("tron").map(String::as_str),
            Some("T111111111111111111111111111111111")
        );
        assert_eq!(
            recipients.get("ethereum").map(String::as_str),
            Some("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            recipients.get("aptos").map(String::as_str),
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn rejects_invalid_and_duplicate_fee_recipients() {
        assert!(parse_fee_recipients(r#"["ethereum=not-an-address"]"#).is_err());
        assert!(parse_fee_recipients(
            r#"["ethereum=0x1111111111111111111111111111111111111111","ethereum=0x1111111111111111111111111111111111111111"]"#
        )
        .is_err());
    }

    #[test]
    fn rejects_testnet_and_unknown_networks_for_otc_settlement() {
        let mut cfg = Config {
            enabled: true,
            ..Config::default()
        };
        cfg.ethereum_chain_id = 11155111;
        assert!(cfg.validate_network().is_err());

        cfg.ethereum_chain_id = 1;
        cfg.p2p_ethereum_chain_id = Some(11155111);
        assert!(cfg.validate_network().is_err());

        cfg.p2p_usdt_contract = Some(USDT.into());
        cfg.p2p_ethereum_chain_id = Some(8453);
        assert!(cfg.validate_network().is_err());
    }
}

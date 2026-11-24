use super::error::{invalid, Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub(crate) const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
pub(crate) const PROFILE: &str = "pay3flow-otc-fep0837-v1";

/// Direction of the stablecoin leg relative to the desk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StablecoinLeg {
    /// The stablecoin arrives from the deal network and the swap starts there.
    Incoming,
    /// The stablecoin is delivered to the deal network and the swap ends there.
    Outgoing,
}

/// Smallest cross-network stablecoin amount in six-decimal units.
///
/// Every cross-network deal has an Ethereum leg, so the Ethereum fixed fee
/// (0.3 USDT) sets the floor for most networks. The per-network values are the
/// fixed fees measured against the live 1Click quote API on 2026-11-16; bridge
/// floors move, so this is a lower bound rather than a live quote.
pub(crate) fn minimum_stablecoin_units(network: &str, leg: StablecoinLeg) -> u128 {
    const ETH_ORIGIN_REFUND: u128 = 300_001;
    const ETH_DESTINATION_WITHDRAW: u128 = 300_065;
    let (origin, destination) = match network {
        "ethereum" => (ETH_ORIGIN_REFUND, ETH_DESTINATION_WITHDRAW),
        "ton" => (42_912, 42_912),
        "sui" => (20_001, 20_000),
        "avalanche-c" => (14_302, 14_307),
        "aptos" => (11_957, 11_956),
        "polygon-pos" | "bnb-smart-chain" => (10_040, 10_039),
        "solana" => (9_977, 10_039),
        "arbitrum-one" => (5_300, 5_300),
        "base" => (2_400, 2_400),
        "optimism" => (494, 494),
        "gnosis" => (37, 37),
        // NEAR charges no fixed withdrawal fee, so only the Ethereum leg binds.
        "near" => (1, 0),
        // Sei has no 1Click route yet and other listed chains settle directly.
        _ => (1, 1),
    };
    match leg {
        StablecoinLeg::Incoming => ETH_DESTINATION_WITHDRAW.max(origin),
        StablecoinLeg::Outgoing => ETH_ORIGIN_REFUND.max(destination),
    }
}

/// Minimum stablecoin value accepted by the OTC P2P product, in six-decimal
/// units. Per-network technical route minimums may raise this floor.
pub(crate) const P2P_PRODUCT_MIN_STABLECOIN_UNITS: u128 = 30_000_000;

/// Effective minimum for a P2P deal: the product floor or technical route
/// minimum, whichever is larger.
pub(crate) fn required_stablecoin_units(network: &str, leg: StablecoinLeg) -> u128 {
    P2P_PRODUCT_MIN_STABLECOIN_UNITS.max(minimum_stablecoin_units(network, leg))
}

/// Fixed fee charged when the stablecoin is delivered on `network`, in
/// six-decimal units. This is the part of a cross-network route the trader
/// always pays, so the offer preview can show it before signing.
pub(crate) fn stablecoin_withdraw_fee_units(network: &str) -> u128 {
    match network {
        "ethereum" => 300_000,
        "ton" => 42_912,
        "sui" => 20_000,
        "avalanche-c" => 14_307,
        "aptos" => 11_956,
        "polygon-pos" | "bnb-smart-chain" | "solana" => 10_039,
        "arbitrum-one" => 5_300,
        "base" => 2_400,
        "optimism" => 494,
        "gnosis" => 37,
        _ => 0,
    }
}
fn default_evm_chain() -> u64 {
    1
}
fn default_usdt_contract() -> String {
    USDT.into()
}
fn default_ever_fee_reserve() -> String {
    "0.05".into()
}
fn default_temp_gas_reserve() -> String {
    "0.0005".into()
}
/// Treasury native balance below which an auto top-up swap is triggered.
/// Expressed in the chain's native token (e.g. ETH).  Default is 0.001 which
/// at $3000/ETH equals roughly $3 — enough headroom for several deal-wallet
/// gas fills.  Tune per deployment via OTC config JSON.
fn default_temp_gas_low_threshold() -> String {
    "0.001".into()
}
/// How much native token to buy per top-up swap.  Default is 0.005 (≈$15 at
/// $3000/ETH) which covers ten deal-wallet fills at the default reserve level.
fn default_temp_gas_target() -> String {
    "0.005".into()
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub enabled: bool,
    pub public_offers_enabled: bool,
    #[serde(default)]
    pub p2p_enabled: bool,
    /// Temporary custodial intermediaries. Off by default.
    #[serde(default)]
    pub temp_wallets: bool,
    /// Native gas a temporary wallet must hold before a sweep is signed.
    #[serde(default = "default_temp_gas_reserve")]
    pub temp_gas_reserve: String,
    /// Treasury native balance that triggers an automatic USDT->native swap.
    #[serde(default = "default_temp_gas_low_threshold")]
    pub temp_gas_low_threshold: String,
    /// How much native token to buy per automatic top-up swap.
    #[serde(default = "default_temp_gas_target")]
    pub temp_gas_target: String,
    /// Deprecated compatibility setting. Custodial same-network payouts always
    /// use the permanent treasury when `temp_wallets` is enabled.
    #[serde(default)]
    pub temp_reserve_payout: bool,
    pub demo: bool,
    #[serde(default = "default_evm_chain")]
    pub ethereum_chain_id: u64,
    #[serde(default = "default_usdt_contract")]
    pub usdt_contract: String,
    #[serde(default = "default_ever_fee_reserve")]
    pub p2p_ever_fee_reserve: String,
    /// Base peer settlement network; additional EVM networks use `p2p_evm_rpcs`.
    #[serde(default)]
    pub p2p_ethereum_chain_id: Option<u64>,
    #[serde(default)]
    pub p2p_usdt_contract: Option<String>,
    #[serde(default)]
    pub p2p_ethereum_rpc: Vec<String>,
    #[serde(default)]
    pub p2p_evm_rpcs: BTreeMap<String, Vec<String>>,
    /// Legacy direct non-EVM contours, keyed by `ton` or `solana`.
    #[serde(default)]
    pub p2p_chain_rpcs: BTreeMap<String, Vec<String>>,
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
            temp_wallets: false,
            temp_gas_reserve: default_temp_gas_reserve(),
            temp_gas_low_threshold: default_temp_gas_low_threshold(),
            temp_gas_target: default_temp_gas_target(),
            temp_reserve_payout: false,
            demo: false,
            ethereum_chain_id: default_evm_chain(),
            usdt_contract: default_usdt_contract(),
            p2p_ever_fee_reserve: default_ever_fee_reserve(),
            p2p_ethereum_chain_id: None,
            p2p_usdt_contract: None,
            p2p_ethereum_rpc: Vec::new(),
            p2p_evm_rpcs: BTreeMap::new(),
            p2p_chain_rpcs: BTreeMap::new(),
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
            min_usdt: "30".into(),
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
        cfg.temp_wallets = match std::env::var("OTC_TEMP_WALLETS") {
            Ok(value) => parse_flag("OTC_TEMP_WALLETS", &value)?,
            Err(std::env::VarError::NotPresent) => cfg.temp_wallets,
            Err(error) => return Err(error.into()),
        };
        cfg.temp_reserve_payout = match std::env::var("OTC_TEMP_RESERVE_PAYOUT") {
            Ok(value) => parse_flag("OTC_TEMP_RESERVE_PAYOUT", &value)?,
            Err(std::env::VarError::NotPresent) => cfg.temp_reserve_payout,
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
        for (network, endpoints) in &self.p2p_evm_rpcs {
            if evm_chain_id(network).is_none()
                || network == "ethereum"
                || endpoints.len() < 2
                || endpoints[0] == endpoints[1]
                || (!self.demo && endpoints.iter().any(|url| !url.starts_with("https://")))
            {
                return Err(invalid("invalid direct OTC EVM network configuration"));
            }
        }
        for (network, endpoints) in &self.p2p_chain_rpcs {
            if !matches!(network.as_str(), "ton" | "solana")
                || endpoints.len() < 2
                || endpoints[0] == endpoints[1]
                || (!self.demo && endpoints.iter().any(|url| !url.starts_with("https://")))
            {
                return Err(invalid("invalid direct OTC chain configuration"));
            }
        }
        Ok(())
    }
    fn check_evm_network(&self, chain_id: u64, contract: &str) -> Result<()> {
        let canonical = super::wallet::address("ethereum", contract)?;
        if canonical != contract || chain_id != 1 {
            return Err(invalid("OTC base network must be Ethereum mainnet"));
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
    pub fn p2p_evm_rpcs(&self, network: &str) -> Option<&[String]> {
        if network == "ethereum" {
            Some(self.p2p_rpc())
        } else {
            self.p2p_evm_rpcs.get(network).map(Vec::as_slice)
        }
    }
    pub fn p2p_chain_endpoints(&self, network: &str) -> Option<&[String]> {
        self.p2p_chain_rpcs
            .get(network)
            .filter(|endpoints| endpoints.len() >= 2)
            .map(Vec::as_slice)
    }

    /// EVM networks for which OTC has a complete signed temp-wallet path.
    pub(crate) fn temp_wallet_networks(&self) -> Vec<String> {
        let mut networks = Vec::new();
        if self.p2p_rpc().len() >= 2 {
            networks.push("ethereum".to_string());
        }
        for network in ["polygon-pos", "avalanche-c"] {
            if self
                .p2p_evm_rpcs(network)
                .is_some_and(|endpoints| endpoints.len() >= 2)
            {
                networks.push(network.to_string());
            }
        }
        networks
    }
}

pub(crate) fn evm_chain_id(network: &str) -> Option<u64> {
    match network {
        "ethereum" => Some(1),
        "optimism" => Some(10),
        "bnb-smart-chain" => Some(56),
        "gnosis" => Some(100),
        "polygon-pos" => Some(137),
        "avalanche-c" => Some(43114),
        "arbitrum-one" => Some(42161),
        "base" => Some(8453),
        "sei" => Some(1329),
        _ => None,
    }
}

fn parse_flag(name: &str, raw: &str) -> anyhow::Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" | "" => Ok(false),
        _ => anyhow::bail!("{name} must be 0 or 1"),
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
        let network = crate::route_engine::canonical_network_id(network.trim());
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
            | "gnosis"
            | "avalanche-c"
            | "polygon-pos"
            | "bnb-smart-chain"
            | "arbitrum-one"
            | "base"
            | "optimism"
            | "sei"
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
        "solana" | "sui" | "ton" => {
            crate::route_execution::validate_otc_wallet_address(network, address).is_ok()
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
        super::amount::units(&self.temp_gas_reserve, 18)?;
        super::amount::units(&self.temp_gas_low_threshold, 18)?;
        super::amount::units(&self.temp_gas_target, 18)?;
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
    use super::{evm_chain_id, parse_fee_recipients, parse_flag, Config, USDT};

    #[test]
    fn temporary_wallets_are_off_until_an_explicit_flag() {
        assert!(!Config::default().temp_wallets);
        assert!(!Config::default().temp_reserve_payout);
        assert!(parse_flag("OTC_TEMP_WALLETS", "1").unwrap());
        assert!(!parse_flag("OTC_TEMP_WALLETS", "0").unwrap());
        assert!(parse_flag("OTC_TEMP_WALLETS", "maybe").is_err());
    }

    #[test]
    fn treasury_networks_only_include_configured_temp_signers() {
        let mut cfg = Config::default();
        assert!(cfg.temp_wallet_networks().is_empty());
        cfg.p2p_ethereum_rpc = vec!["https://eth-a".into(), "https://eth-b".into()];
        cfg.p2p_evm_rpcs.insert(
            "polygon-pos".into(),
            vec!["https://polygon-a".into(), "https://polygon-b".into()],
        );
        cfg.p2p_evm_rpcs.insert(
            "avalanche-c".into(),
            vec!["https://avalanche-a".into(), "https://avalanche-b".into()],
        );
        cfg.p2p_evm_rpcs.insert(
            "base".into(),
            vec!["https://base-a".into(), "https://base-b".into()],
        );
        cfg.p2p_chain_rpcs.insert(
            "tron".into(),
            vec!["https://tron-a".into(), "https://tron-b".into()],
        );
        assert_eq!(
            cfg.temp_wallet_networks(),
            vec!["ethereum", "polygon-pos", "avalanche-c"]
        );
    }

    #[test]
    fn parses_all_operator_fee_recipients() {
        let entries = r#"[
            "tron=T111111111111111111111111111111111",
            "near=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bitcoin=bc1qaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "ethereum=0x1111111111111111111111111111111111111111",
            "solana=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "avalanche-c=0x1111111111111111111111111111111111111111",
            "sei=0x1111111111111111111111111111111111111111",
            "sui=0x1111111111111111111111111111111111111111111111111111111111111111",
            "polygon-pos=0x1111111111111111111111111111111111111111",
            "ton=UQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAJKZ",
            "bnb-smart-chain=0x1111111111111111111111111111111111111111",
            "arbitrum-one=0x1111111111111111111111111111111111111111",
            "base=0x1111111111111111111111111111111111111111",
            "gnosis=0x1111111111111111111111111111111111111111",
            "aptos=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ]"#;
        let recipients = parse_fee_recipients(entries).unwrap();
        assert_eq!(recipients.len(), 15);
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
    fn canonicalizes_fee_recipient_network_aliases() {
        let recipients =
            parse_fee_recipients(r#"["eth=0x1111111111111111111111111111111111111111"]"#).unwrap();
        assert!(recipients.contains_key("ethereum"));
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

    #[test]
    fn validates_independent_rpc_pairs_for_direct_evm_networks() {
        let mut cfg = Config {
            p2p_evm_rpcs: [(
                "base".into(),
                vec![
                    "https://base-a.example".into(),
                    "https://base-b.example".into(),
                ],
            )]
            .into(),
            ..Config::default()
        };
        assert!(cfg.validate_network().is_ok());
        assert_eq!(cfg.p2p_evm_rpcs("base").unwrap().len(), 2);
        for (network, chain_id) in [
            ("ethereum", 1),
            ("optimism", 10),
            ("bnb-smart-chain", 56),
            ("gnosis", 100),
            ("polygon-pos", 137),
            ("avalanche-c", 43114),
            ("arbitrum-one", 42161),
            ("base", 8453),
            ("sei", 1329),
        ] {
            assert_eq!(evm_chain_id(network), Some(chain_id));
        }

        cfg.p2p_evm_rpcs.insert(
            "base".into(),
            vec!["https://same.example".into(), "https://same.example".into()],
        );
        assert!(cfg.validate_network().is_err());
        cfg.p2p_evm_rpcs.insert(
            "goerli".into(),
            vec![
                "https://rpc-a.example".into(),
                "https://rpc-b.example".into(),
            ],
        );
        assert!(cfg.validate_network().is_err());
    }

    #[test]
    fn validates_independent_endpoint_pairs_for_legacy_direct_non_evm_networks() {
        let mut cfg = Config {
            p2p_chain_rpcs: [
                (
                    "ton".into(),
                    vec![
                        "https://ton-a.example".into(),
                        "https://ton-b.example".into(),
                    ],
                ),
                (
                    "solana".into(),
                    vec![
                        "https://sol-a.example".into(),
                        "https://sol-b.example".into(),
                    ],
                ),
            ]
            .into(),
            ..Config::default()
        };
        assert!(cfg.validate_network().is_ok());
        assert_eq!(cfg.p2p_chain_endpoints("solana").unwrap().len(), 2);

        cfg.p2p_chain_rpcs.insert(
            "tron".into(),
            vec![
                "https://tron-a.example".into(),
                "https://tron-b.example".into(),
            ],
        );
        assert!(cfg.validate_network().is_err());
        cfg.p2p_chain_rpcs.remove("tron");

        cfg.p2p_chain_rpcs.insert(
            "solana".into(),
            vec!["https://same.example".into(), "https://same.example".into()],
        );
        assert!(cfg.validate_network().is_err());
        cfg.p2p_chain_rpcs.insert(
            "cosmos".into(),
            vec!["https://a.example".into(), "https://b.example".into()],
        );
        assert!(cfg.validate_network().is_err());
    }
}

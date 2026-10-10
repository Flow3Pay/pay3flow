use super::error::{invalid, Error, Result};
use serde::{Deserialize, Serialize};

pub(crate) const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub(crate) const PROFILE: &str = "pay3flow-otc-fep0837-v1";
#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub enabled: bool,
    pub demo: bool,
    pub origin: String,
    pub relay_origin: String,
    pub matcher_actor: String,
    pub desk_actor: String,
    pub desk_name: String,
    pub ethereum_rpc: Vec<String>,
    pub everscale_graphql: Vec<String>,
    pub everwallet_code_hashes: Vec<String>,
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
}
impl Config {
    pub fn load(origin: &str) -> anyhow::Result<Self> {
        let mut cfg: Self = match std::env::var("OTC_CONFIG_FILE") {
            Ok(path) => serde_json::from_slice(&std::fs::read(path)?)?,
            Err(_) => Self::default(),
        };
        cfg.origin = origin.trim_end_matches('/').to_string();
        if cfg.enabled {
            cfg.validate()?;
        }
        Ok(cfg)
    }
    pub fn validate(&self) -> Result<()> {
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

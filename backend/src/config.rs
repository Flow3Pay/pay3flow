use anyhow::Context;
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

/// Runtime configuration loaded from `config.toml` plus secret values supplied
/// through the process environment.
pub struct Config {
    pub log_filter: String,
    pub http_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub ap_origin: String,
    pub ap_handle: String,
    pub ap_key_path: String,
    pub ap_require_signatures: bool,
    pub fmatch_inbox: String,
    pub fmatch_actor_id: String,
    /// AES-GCM key material for encrypting provider credentials (PLAN #30).
    pub secrets_key: String,
    /// Our cut, percent of the gross payment amount (PLAN #41).
    pub service_fee_percent: f64,
    /// FX source: `mock` (offline table) or `http` (live endpoint, PLAN #42).
    pub fx_source: String,
    pub fx_url: String,
    pub fx_cache_ttl_secs: u64,
    /// Margin applied on top of the mid-market rate for cross-currency routes
    /// (PLAN #43).
    pub fx_margin_percent: f64,
    /// Bearer token guarding the admin endpoints (PLAN #46e). Mismatched
    /// tokens get 401; an unset value keeps the dev default.
    pub admin_token: String,
    /// TTL of the exchange-pairs in-memory cache (PLAN #46c), seconds.
    pub pairs_cache_ttl_secs: u64,
    /// Redis URL for caching fmatch candidates (PLAN #5a, #37d). It remains
    /// environment-only because it may contain a password.
    pub redis_url: String,
    /// Public P2P advertisement search. This is read-only and never places orders.
    pub p2p_search_enabled: bool,
    pub p2p_search_timeout_ms: u64,
    pub p2p_search_cache_ttl_ms: u64,
    pub p2p_search_assets: Vec<String>,
    pub playwright_chromium_executable: Option<String>,
    pub p2p_workflow_debug_screenshot: Option<String>,
    /// NEAR Intents 1-Click API configuration. The JWT is never logged or
    /// serialized into a capability.
    pub near_intents_url: String,
    pub near_intents_jwt: Option<String>,
    pub near_intents_quote_recipient: Option<String>,
    pub near_intents_quote_refund_to: Option<String>,
    pub near_intents_quote_recipients: Vec<String>,
    pub near_intents_quote_refunds: Vec<String>,
    pub near_intents_refresh_secs: u64,
    /// Deployment-owned inputs for the private route capability graph.
    pub route_max_depth: usize,
    pub route_source_fiats: Vec<String>,
    pub route_p2p_assets: Vec<String>,
    pub route_intent_assets: Vec<String>,
    pub route_withdrawals: Vec<String>,
    /// Optional CoW Protocol quote endpoints, formatted as `chain=url`.
    pub cow_api_urls: Vec<String>,
    /// Optional CoW token addresses, formatted as `chain:SYMBOL=address`.
    pub cow_tokens: Vec<String>,
    pub cow_quote_address: Option<String>,
    /// Symbiosis cross-chain quote API settings.
    pub symbiosis_url: String,
    pub symbiosis_partner_id: Option<String>,
    pub symbiosis_quote_address: String,
    pub symbiosis_slippage_bps: u32,
}

/// The checked-in TOML surface deliberately contains no credentials. Unknown
/// keys are rejected so a secret cannot be accidentally added to the file and
/// silently mistaken for supported configuration.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    log_filter: String,
    http_addr: String,
    ap_origin: String,
    ap_handle: String,
    ap_key_path: String,
    ap_require_signatures: bool,
    fmatch_inbox: String,
    fmatch_actor_id: String,
    service_fee_percent: f64,
    fx_source: String,
    fx_url: String,
    fx_cache_ttl_secs: u64,
    fx_margin_percent: f64,
    pairs_cache_ttl_secs: u64,
    p2p_search_enabled: bool,
    p2p_search_timeout_ms: u64,
    p2p_search_cache_ttl_ms: u64,
    p2p_search_assets: Vec<String>,
    playwright_chromium_executable: Option<String>,
    p2p_workflow_debug_screenshot: Option<String>,
    near_intents_url: String,
    near_intents_quote_recipient: Option<String>,
    near_intents_quote_refund_to: Option<String>,
    near_intents_quote_recipients: Vec<String>,
    near_intents_quote_refunds: Vec<String>,
    near_intents_refresh_secs: u64,
    route_max_depth: usize,
    route_source_fiats: Vec<String>,
    route_p2p_assets: Vec<String>,
    route_intent_assets: Vec<String>,
    route_withdrawals: Vec<String>,
    cow_api_urls: Vec<String>,
    cow_tokens: Vec<String>,
    cow_quote_address: Option<String>,
    symbiosis_url: String,
    symbiosis_quote_address: String,
    symbiosis_slippage_bps: u32,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            log_filter: "info".into(),
            http_addr: "0.0.0.0:8080".into(),
            ap_origin: "http://localhost:8080".into(),
            ap_handle: "pay3flow".into(),
            ap_key_path: "./data/ap-key.pem".into(),
            ap_require_signatures: false,
            fmatch_inbox: "http://localhost:7277/inbox/actra".into(),
            fmatch_actor_id: "http://localhost:7277/actor/actra".into(),
            service_fee_percent: 0.7,
            fx_source: "mock".into(),
            fx_url: "https://api.frankfurter.app/latest".into(),
            fx_cache_ttl_secs: 300,
            fx_margin_percent: 1.0,
            pairs_cache_ttl_secs: 300,
            p2p_search_enabled: true,
            p2p_search_timeout_ms: 4_000,
            p2p_search_cache_ttl_ms: 5_000,
            p2p_search_assets: Vec::new(),
            playwright_chromium_executable: None,
            p2p_workflow_debug_screenshot: None,
            near_intents_url: "https://1click.chaindefuser.com".into(),
            near_intents_quote_recipient: None,
            near_intents_quote_refund_to: None,
            near_intents_quote_recipients: Vec::new(),
            near_intents_quote_refunds: Vec::new(),
            near_intents_refresh_secs: 300,
            route_max_depth: 4,
            route_source_fiats: vec!["AMD".into()],
            route_p2p_assets: vec!["USDT".into(), "USDC".into(), "XRP".into()],
            route_intent_assets: vec!["USDT@tron".into(), "USDC@solana".into(), "XRP@xrpl".into()],
            route_withdrawals: Vec::new(),
            cow_api_urls: Vec::new(),
            cow_tokens: Vec::new(),
            cow_quote_address: None,
            symbiosis_url: "https://api.symbiosis.finance/crosschain".into(),
            symbiosis_quote_address: "0x0000000000000000000000000000000000000001".into(),
            symbiosis_slippage_bps: 300,
        }
    }
}

impl Config {
    /// Loads non-secret settings from TOML and credentials from environment.
    /// `PAY3FLOW_CONFIG_FILE` only selects the file; it is not a configuration
    /// override. Without it, `config.toml` is searched from the current
    /// directory and then its parent for local `cargo` workflows.
    pub fn load() -> anyhow::Result<Self> {
        let file = load_file_config()?;
        Ok(Self {
            log_filter: file.log_filter,
            http_addr: file.http_addr,
            database_url: secret_env_or(
                "DATABASE_URL",
                "postgres://pay3flow:pay3flow@localhost:5432/pay3flow",
            ),
            jwt_secret: secret_env_or("JWT_SECRET", "dev-secret-change-me"),
            ap_origin: file.ap_origin,
            ap_handle: file.ap_handle,
            ap_key_path: file.ap_key_path,
            ap_require_signatures: file.ap_require_signatures,
            fmatch_inbox: file.fmatch_inbox,
            fmatch_actor_id: file.fmatch_actor_id,
            secrets_key: secret_env_or("SECRETS_KEY", "dev-secrets-key-change-me"),
            service_fee_percent: file.service_fee_percent,
            fx_source: file.fx_source,
            fx_url: file.fx_url,
            fx_cache_ttl_secs: file.fx_cache_ttl_secs,
            fx_margin_percent: file.fx_margin_percent,
            admin_token: secret_env_or("ADMIN_TOKEN", "dev-admin-token-change-me"),
            pairs_cache_ttl_secs: file.pairs_cache_ttl_secs,
            redis_url: secret_env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            p2p_search_enabled: file.p2p_search_enabled,
            p2p_search_timeout_ms: file.p2p_search_timeout_ms,
            p2p_search_cache_ttl_ms: file.p2p_search_cache_ttl_ms,
            p2p_search_assets: normalize_assets(file.p2p_search_assets),
            playwright_chromium_executable: clean_optional(file.playwright_chromium_executable),
            p2p_workflow_debug_screenshot: clean_optional(file.p2p_workflow_debug_screenshot),
            near_intents_url: file.near_intents_url,
            near_intents_jwt: optional_secret_env("NEAR_INTENTS_JWT"),
            near_intents_quote_recipient: clean_optional(file.near_intents_quote_recipient),
            near_intents_quote_refund_to: clean_optional(file.near_intents_quote_refund_to),
            near_intents_quote_recipients: file.near_intents_quote_recipients,
            near_intents_quote_refunds: file.near_intents_quote_refunds,
            near_intents_refresh_secs: file.near_intents_refresh_secs,
            route_max_depth: file.route_max_depth,
            route_source_fiats: file.route_source_fiats,
            route_p2p_assets: file.route_p2p_assets,
            route_intent_assets: file.route_intent_assets,
            route_withdrawals: file.route_withdrawals,
            cow_api_urls: file.cow_api_urls,
            cow_tokens: file.cow_tokens,
            cow_quote_address: clean_optional(file.cow_quote_address),
            symbiosis_url: file.symbiosis_url,
            symbiosis_partner_id: optional_secret_env("SYMBIOSIS_PARTNER_ID"),
            symbiosis_quote_address: file.symbiosis_quote_address,
            symbiosis_slippage_bps: file.symbiosis_slippage_bps,
        })
    }

    /// Compatibility name for existing callers. Configuration values are no
    /// longer read from individual environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::load()
    }
}

fn load_file_config() -> anyhow::Result<FileConfig> {
    let explicit_path = env::var_os("PAY3FLOW_CONFIG_FILE").map(PathBuf::from);
    let path = explicit_path.or_else(|| {
        [
            PathBuf::from("config.toml"),
            PathBuf::from("../config.toml"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file())
    });

    let Some(path) = path else {
        return Ok(FileConfig::default());
    };

    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read configuration file {}", path.display()))?;
    toml::from_str(&source)
        .with_context(|| format!("failed to parse TOML configuration file {}", path.display()))
}

fn secret_env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn optional_secret_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .and_then(|value| clean_optional(Some(value)))
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_assets(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::FileConfig;

    #[test]
    fn parses_non_secret_toml_configuration() {
        let config: FileConfig = toml::from_str(
            r#"
                http_addr = "127.0.0.1:9090"
                ap_origin = "https://pay3flow.example"
                route_source_fiats = ["RUB"]
                route_intent_assets = ["USDT@tron", "USDT@solana"]
                cow_api_urls = ["ethereum=https://api.cow.fi/mainnet"]
            "#,
        )
        .unwrap();

        assert_eq!(config.http_addr, "127.0.0.1:9090");
        assert_eq!(config.route_source_fiats, ["RUB"]);
        assert_eq!(config.cow_api_urls.len(), 1);
    }

    #[test]
    fn rejects_secret_keys_in_toml() {
        let result = toml::from_str::<FileConfig>(r#"jwt_secret = "must-not-be-here""#);
        assert!(result.is_err());
    }
}

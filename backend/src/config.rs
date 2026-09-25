use std::env;

pub struct Config {
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
    /// Redis URL for caching fmatch candidates (PLAN #5a, #37d).
    pub redis_url: String,
    /// Public P2P advertisement search. This is read-only and never places orders.
    pub p2p_search_enabled: bool,
    pub p2p_search_timeout_ms: u64,
    pub p2p_search_cache_ttl_ms: u64,
    pub p2p_search_assets: Vec<String>,
    /// NEAR Intents 1-Click API configuration. The JWT is never logged or
    /// serialized into a capability.
    pub near_intents_url: String,
    pub near_intents_jwt: Option<String>,
    pub near_intents_quote_recipient: Option<String>,
    pub near_intents_quote_refund_to: Option<String>,
    pub near_intents_refresh_secs: u64,
    /// Deployment-owned inputs for the private route capability graph.
    pub route_max_depth: usize,
    pub route_source_fiats: Vec<String>,
    pub route_p2p_assets: Vec<String>,
    pub route_intent_assets: Vec<String>,
    pub route_withdrawals: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            http_addr: env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://pay3flow:pay3flow@localhost:5432/pay3flow".into()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into()),
            ap_origin: env::var("AP_ORIGIN").unwrap_or_else(|_| "http://localhost:8080".into()),
            ap_handle: env::var("AP_HANDLE").unwrap_or_else(|_| "pay3flow".into()),
            ap_key_path: env::var("AP_KEY_PATH").unwrap_or_else(|_| "./data/ap-key.pem".into()),
            ap_require_signatures: env::var("AP_REQUIRE_SIGNATURES")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            fmatch_inbox: env::var("FMATCH_INBOX")
                .unwrap_or_else(|_| "http://localhost:7277/inbox/actra".into()),
            fmatch_actor_id: env::var("FMATCH_ACTOR_ID")
                .unwrap_or_else(|_| "http://localhost:7277/actor/actra".into()),
            secrets_key: env::var("SECRETS_KEY")
                .unwrap_or_else(|_| "dev-secrets-key-change-me".into()),
            service_fee_percent: env::var("SERVICE_FEE_PERCENT")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.7),
            fx_source: env::var("FX_SOURCE").unwrap_or_else(|_| "mock".into()),
            fx_url: env::var("FX_URL")
                .unwrap_or_else(|_| "https://api.frankfurter.app/latest".into()),
            fx_cache_ttl_secs: env::var("FX_CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(300),
            fx_margin_percent: env::var("FX_MARGIN_PERCENT")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0),
            admin_token: env::var("ADMIN_TOKEN")
                .unwrap_or_else(|_| "dev-admin-token-change-me".into()),
            pairs_cache_ttl_secs: env::var("PAIRS_CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(300),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            p2p_search_enabled: env_flag("P2P_SEARCH_ENABLED", true),
            p2p_search_timeout_ms: env::var("P2P_SEARCH_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(4_000),
            p2p_search_cache_ttl_ms: env::var("P2P_SEARCH_CACHE_TTL_MS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(5_000),
            p2p_search_assets: env::var("P2P_SEARCH_ASSETS")
                .unwrap_or_default()
                .split(',')
                .map(|asset| asset.trim().to_ascii_uppercase())
                .filter(|asset| !asset.is_empty())
                .collect(),
            near_intents_url: env::var("NEAR_INTENTS_URL")
                .unwrap_or_else(|_| "https://1click.chaindefuser.com".into()),
            near_intents_jwt: env::var("NEAR_INTENTS_JWT")
                .ok()
                .filter(|value| !value.is_empty()),
            near_intents_quote_recipient: optional_env("NEAR_INTENTS_QUOTE_RECIPIENT"),
            near_intents_quote_refund_to: optional_env("NEAR_INTENTS_QUOTE_REFUND_TO"),
            near_intents_refresh_secs: env::var("NEAR_INTENTS_REFRESH_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(300),
            route_max_depth: env::var("ROUTE_MAX_DEPTH")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(4),
            route_source_fiats: csv_env("ROUTE_SOURCE_FIATS", &["AMD"]),
            route_p2p_assets: csv_env("ROUTE_P2P_ASSETS", &["USDT", "USDC", "XRP"]),
            route_intent_assets: csv_env(
                "ROUTE_INTENT_ASSETS",
                &["USDT@tron", "USDC@solana", "XRP@xrpl"],
            ),
            route_withdrawals: csv_env("ROUTE_WITHDRAWALS", &[]),
        })
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn csv_env(name: &str, default: &[&str]) -> Vec<String> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .filter(|values: &Vec<String>| !values.is_empty())
        .unwrap_or_else(|| default.iter().map(|value| (*value).to_string()).collect())
}

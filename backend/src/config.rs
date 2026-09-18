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
    /// crw gRPC search endpoint (PLAN #82).
    pub crw_grpc_url: String,
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
            crw_grpc_url: env::var("CRW_GRPC_URL")
                .unwrap_or_else(|_| "http://localhost:3031".into()),
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
        })
    }
}

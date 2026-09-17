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
        })
    }
}

use std::env;

pub struct Config {
    pub http_addr: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            http_addr: env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
        })
    }
}
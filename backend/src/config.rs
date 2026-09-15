use std::env;

pub struct Config {
    pub http_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            http_addr: env::var("HTTP_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://pay3flow:pay3flow@localhost:5432/pay3flow".into()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into()),
        })
    }
}
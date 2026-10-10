//! Customer-first, non-custodial EVER/USDT OTC settlement.
mod accounting;
mod amount;
mod booking;
mod chain;
mod config;
mod error;
mod federation;
pub(crate) mod http;
mod metrics;
mod model;
mod operations;
mod protocol;
mod repo;
mod reporting;
mod review;
#[cfg(test)]
mod tests;
mod transfers;
mod wallet;
mod workers;

use crate::{
    core::{crypto::SecretBox, jwt::Jwt},
    db::DbPool,
};
use config::Config;
use error::{Error, Result};
use std::sync::Arc;

/// Durable OTC application service. It never holds blockchain signing keys.
#[derive(Clone)]
pub struct Service {
    pub(crate) pool: DbPool,
    pub(crate) cfg: Arc<Config>,
    pub(crate) secrets: SecretBox,
    pub(crate) jwt: Jwt,
    pub(crate) http: reqwest::Client,
}
impl Service {
    /// Load OTC configuration, disabled by default, and build bounded HTTP transport.
    pub fn build(
        pool: DbPool,
        origin: &str,
        secrets_key: &str,
        jwt_secret: &str,
    ) -> anyhow::Result<Self> {
        let cfg = Config::load(origin)?;
        if cfg.enabled && !cfg.demo && (secrets_key.len() < 32 || jwt_secret.len() < 32) {
            anyhow::bail!("live OTC requires strong secrets");
        }
        Ok(Self {
            pool,
            cfg: Arc::new(cfg),
            secrets: SecretBox::new(secrets_key),
            jwt: Jwt::new(&format!("otc:{jwt_secret}")),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    pub(crate) async fn ready(&self) -> Result<()> {
        if !self.cfg.enabled {
            return Err(Error::Disabled);
        }
        let client = self.pool.get().await?;
        let mode: Option<bool> = client
            .query_one("SELECT demo FROM otc_controls WHERE singleton", &[])
            .await?
            .get(0);
        if mode.is_some_and(|mode| mode != self.cfg.demo) {
            return Err(Error::Disabled);
        }
        let paused: bool = client
            .query_one("SELECT paused FROM otc_controls WHERE singleton", &[])
            .await?
            .get(0);
        let unhealthy: i64 = client.query_one("SELECT count(*) FROM otc_observers WHERE paused OR last_scan IS NULL OR last_scan < now()-interval '60 seconds'",&[]).await?.get(0);
        if paused || unhealthy > 0 {
            return Err(Error::Disabled);
        }
        Ok(())
    }
    pub(crate) async fn quoting(&self) -> Result<()> {
        use chrono::Timelike;
        self.ready().await?;
        let hour = chrono::Utc::now().hour();
        if hour < self.cfg.staffed_utc_start || hour >= self.cfg.staffed_utc_end {
            return Err(Error::Disabled);
        }
        Ok(())
    }
    /// Start persistent observation, relay reconciliation, and incident notification workers.
    pub fn start(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(error) = service.tick().await {
                    tracing::warn!(error = %error, "otc.worker.tick_failed");
                }
            }
        });
    }
}

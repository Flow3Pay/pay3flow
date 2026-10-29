pub mod repo;

use anyhow::{Context, Result};
use deadpool_postgres::{
    Config as PoolCfg, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime, Timeouts,
};
use std::time::Duration;
use tokio_postgres::NoTls;

pub type DbPool = Pool;

/// Connection budget for one backend replica. PostgreSQL allows 100 by
/// default, leaving room for background workers and an operator session.
const POOL_MAX_SIZE: usize = 32;

/// Deadpool waits forever by default. Without these bounds an unreachable
/// database turns every request that touches it into a hang that no error and
/// no probe reports, so the process keeps accepting requests it never answers.
const POOL_TIMEOUTS: Timeouts = Timeouts {
    wait: Some(Duration::from_secs(5)),
    create: Some(Duration::from_secs(5)),
    recycle: Some(Duration::from_secs(5)),
};

/// Detect a database that stopped answering instead of blocking on a socket
/// that a vanished service never closes.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(30);

pub async fn build_pool(url: &str) -> Result<DbPool> {
    let mut cfg = PoolCfg::new();
    cfg.url = Some(url.to_string());
    cfg.connect_timeout = Some(Duration::from_secs(5));
    cfg.keepalives = Some(true);
    cfg.keepalives_idle = Some(KEEPALIVE_IDLE);
    cfg.manager = Some(manager());
    cfg.pool = Some(PoolConfig {
        max_size: POOL_MAX_SIZE,
        timeouts: POOL_TIMEOUTS,
        ..PoolConfig::default()
    });
    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    ping(&pool).await?;
    Ok(pool)
}

pub async fn apply_schema(pool: &DbPool) -> Result<()> {
    let client = pool.get().await?;
    client
        .batch_execute(include_str!("../../migrations/schema.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/otc.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/otc_escrow.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/otc_public_offers.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/otc_ever_multisig.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/catalogs.sql"))
        .await?;
    client
        .batch_execute(include_str!("../../migrations/providers.sql"))
        .await?;
    Ok(())
}

fn manager() -> ManagerConfig {
    ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    }
}

/// Round-trip the database so readiness can report a pool that cannot serve.
pub async fn ping(pool: &DbPool) -> Result<()> {
    let client = pool.get().await.context("cannot reach database")?;
    client.simple_query("SELECT 1").await?;
    Ok(())
}

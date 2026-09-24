pub mod repo;

use anyhow::{Context, Result};
use deadpool_postgres::{Config as PoolCfg, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

pub type DbPool = Pool;

pub async fn build_pool(url: &str) -> Result<DbPool> {
    let mut cfg = PoolCfg::new();
    cfg.url = Some(url.to_string());
    cfg.manager = Some(manager());
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

async fn ping(pool: &DbPool) -> Result<()> {
    let client = pool.get().await.context("cannot reach database")?;
    client.simple_query("SELECT 1").await?;
    Ok(())
}

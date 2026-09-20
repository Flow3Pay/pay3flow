use anyhow::{Context, Result};
use deadpool_redis::{redis::AsyncCommands, Config as RedisConfig, Pool, Runtime};

/// Redis connection pool used for caching fmatch candidate results.
pub type RedisPool = Pool;

/// Build a Redis pool lazily. Connection errors are returned to the caller;
/// the pool itself is still created so the app can start without Redis.
pub fn build_pool(url: &str) -> Result<RedisPool> {
    let config = RedisConfig::from_url(url);
    Ok(config
        .create_pool(Some(Runtime::Tokio1))
        .context("failed to build Redis pool")?)
}

pub async fn get_json<T>(pool: &RedisPool, key: &str) -> Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    let mut conn = pool.get().await.context("failed to get Redis connection")?;
    let value: Option<String> = conn.get(key).await.context("failed to read Redis key")?;
    match value {
        Some(v) => serde_json::from_str(&v)
            .context("failed to decode Redis value")
            .map(Some),
        None => Ok(None),
    }
}

pub async fn set_json<T>(pool: &RedisPool, key: &str, value: &T, ttl_secs: u64) -> Result<()>
where
    T: serde::Serialize,
{
    let mut conn = pool.get().await.context("failed to get Redis connection")?;
    let encoded = serde_json::to_string(value).context("failed to encode value for Redis")?;
    conn.set_ex::<_, _, ()>(key, encoded, ttl_secs as usize)
        .await
        .context("failed to write Redis value")?;
    Ok(())
}

/// Acquire a short-lived single-flight lock for a background cache refresh.
/// The expiration prevents a crashed task from blocking future refreshes.
pub async fn try_acquire_lock(pool: &RedisPool, key: &str, ttl_secs: u64) -> Result<bool> {
    let mut conn = pool.get().await.context("failed to get Redis connection")?;
    let result: Option<String> = deadpool_redis::redis::cmd("SET")
        .arg(key)
        .arg("1")
        .arg("NX")
        .arg("EX")
        .arg(ttl_secs)
        .query_async(&mut conn)
        .await
        .context("failed to acquire Redis lock")?;
    Ok(result.is_some())
}

/// Build the cache key for a fmatch candidate list.
pub fn cache_key(from: &str, to: &str, amount: Option<&str>) -> String {
    match amount {
        Some(amount) => format!("fmatch:{from}:{to}:{amount}"),
        None => format!("fmatch:{from}:{to}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_includes_amount_when_present() {
        assert_eq!(cache_key("USD", "EUR", None), "fmatch:USD:EUR");
        assert_eq!(cache_key("USD", "EUR", Some("1000")), "fmatch:USD:EUR:1000");
    }
}

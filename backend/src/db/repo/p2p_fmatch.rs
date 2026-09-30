use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::db::DbPool;

#[derive(Debug, Clone)]
pub struct CachedP2pAnswer {
    pub response: Value,
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn save_answer(
    pool: &DbPool,
    cache_key: &str,
    response: &Value,
    source: &str,
    observed_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<()> {
    let client = pool.get().await?;
    client
        .execute(
            r#"
            INSERT INTO p2p_fmatch_answers
                (cache_key, response, source, observed_at, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cache_key) DO UPDATE SET
                response = EXCLUDED.response,
                source = EXCLUDED.source,
                observed_at = EXCLUDED.observed_at,
                expires_at = EXCLUDED.expires_at
            "#,
            &[&cache_key, response, &source, &observed_at, &expires_at],
        )
        .await?;
    Ok(())
}

pub async fn latest_answer(
    pool: &DbPool,
    cache_key: &str,
    max_age_secs: i64,
) -> Result<Option<CachedP2pAnswer>> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            r#"
            SELECT response, observed_at, expires_at
            FROM p2p_fmatch_answers
            WHERE cache_key = $1
              AND observed_at >= now() - ($2::bigint * interval '1 second')
            "#,
            &[&cache_key, &max_age_secs.max(0)],
        )
        .await?;
    Ok(row.map(|row| CachedP2pAnswer {
        response: row.get(0),
        observed_at: row.get(1),
        expires_at: row.get(2),
    }))
}

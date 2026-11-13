//! Cached external review snapshots for providers and exchange profiles.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::db::repo::reputation::Conn;

const ENABLED_PROVIDER_GUIDANCE: &str =
    "SELECT guidance FROM providers WHERE slug = $1 AND status = 'enabled' LIMIT 1";
const PROVIDER_REVIEW_CACHE: &str = "SELECT source_url, reviews, fetched_at FROM external_review_cache WHERE subject_key = $1 AND fetched_at > now() - interval '7 days'";
const PROFILE_REVIEW_CACHE: &str = "SELECT reviews, fetched_at FROM external_review_cache WHERE subject_key = $1 AND source_url = $2 AND fetched_at > now() - interval '7 days'";
const UPSERT_REVIEW_CACHE: &str = "INSERT INTO external_review_cache (subject_key, source_url, reviews, fetched_at) VALUES ($1, $2, $3, now()) ON CONFLICT (subject_key) DO UPDATE SET source_url = EXCLUDED.source_url, reviews = EXCLUDED.reviews, fetched_at = now()";
const UPDATE_REVIEW_CACHE: &str = "UPDATE external_review_cache SET reviews = $2, fetched_at = now() WHERE subject_key = $1 AND source_url = $3";
const ENABLED_PROVIDERS: &str = "SELECT DISTINCT ON (slug) slug, guidance FROM providers WHERE status = 'enabled' ORDER BY slug, operation";
const PURGE_REVIEW_CACHE: &str =
    "DELETE FROM external_review_cache WHERE fetched_at < now() - interval '7 days'";

/// Review guidance JSON of an enabled provider.
pub(crate) async fn enabled_provider_guidance<C>(client: &C, slug: &str) -> Result<Option<Value>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(ENABLED_PROVIDER_GUIDANCE, &[&slug])
        .await?;
    Ok(row.map(|row| row.get("guidance")))
}

/// `(source_url, reviews, fetched_at)` of a fresh provider cache entry.
pub(crate) async fn provider_review_cache<C>(
    client: &C,
    subject_key: &str,
) -> Result<Option<(String, Value, DateTime<Utc>)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(PROVIDER_REVIEW_CACHE, &[&subject_key])
        .await?;
    Ok(row.map(|row| {
        (
            row.get("source_url"),
            row.get("reviews"),
            row.get("fetched_at"),
        )
    }))
}

/// `(reviews, fetched_at)` of a fresh profile cache entry.
pub(crate) async fn profile_review_cache<C>(
    client: &C,
    subject_key: &str,
    source_url: &str,
) -> Result<Option<(Value, DateTime<Utc>)>>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_opt(PROFILE_REVIEW_CACHE, &[&subject_key, &source_url])
        .await?;
    Ok(row.map(|row| (row.get("reviews"), row.get("fetched_at"))))
}

pub(crate) async fn upsert_review_cache<C>(
    client: &C,
    subject_key: &str,
    source_url: &str,
    reviews: &Value,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(UPSERT_REVIEW_CACHE, &[&subject_key, &source_url, &reviews])
        .await?;
    Ok(())
}

pub(crate) async fn update_review_cache<C>(
    client: &C,
    subject_key: &str,
    reviews: &Value,
    source_url: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(UPDATE_REVIEW_CACHE, &[&subject_key, &reviews, &source_url])
        .await?;
    Ok(())
}

/// `(slug, guidance)` per enabled provider.
pub(crate) async fn enabled_providers<C>(client: &C) -> Result<Vec<(String, Value)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(ENABLED_PROVIDERS, &[]).await?;
    Ok(rows
        .iter()
        .map(|row| (row.get("slug"), row.get("guidance")))
        .collect())
}

pub(crate) async fn purge_review_cache<C>(client: &C) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(PURGE_REVIEW_CACHE, &[]).await?;
    Ok(())
}

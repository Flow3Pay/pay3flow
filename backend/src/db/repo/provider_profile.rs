use anyhow::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::Transaction;
use uuid::Uuid;

use crate::db::DbPoolObject;

const INSERT_OBSERVATIONS: &str = r#"
INSERT INTO provider_search_observations (search_id, provider_slug, best_rank, response_ok, latency_ms)
SELECT searches.id, observation.slug, observation.rank, observation.ok, observation.latency
FROM route_searches searches
CROSS JOIN unnest($2::text[], $3::integer[], $4::boolean[], $5::double precision[])
    AS observation(slug, rank, ok, latency)
WHERE searches.id = $1 AND searches.counted_for_activity AND searches.status = 'finished'
ON CONFLICT (search_id, provider_slug) DO NOTHING
"#;
const ENABLED_PROVIDER: &str =
    "SELECT 1 FROM providers WHERE slug = $1 AND status = 'enabled' LIMIT 1";
const NOW: &str = "SELECT now()";
const SUMMARY: &str = r#"
WITH observations AS (
    SELECT observation.*, searches.created_at
    FROM provider_search_observations observation
    JOIN route_searches searches ON searches.id = observation.search_id
    WHERE observation.provider_slug = $1 AND searches.status = 'finished'
      AND searches.counted_for_activity
      AND searches.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' - ($2::integer - 1) * interval '1 day'
)
SELECT count(*)::bigint,
       count(*) FILTER (WHERE best_rank <= 10)::bigint,
       count(*) FILTER (WHERE best_rank = 1)::bigint,
       avg(best_rank)::double precision,
       count(response_ok)::bigint,
       count(*) FILTER (WHERE response_ok)::bigint,
       avg(latency_ms)::double precision,
       (SELECT count(*)::bigint FROM service_executions executions
        JOIN services ON services.id = executions.service_id
        WHERE services.slug = $1
          AND executions.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' - ($2::integer - 1) * interval '1 day'),
       (SELECT min(searches.created_at) FROM provider_search_observations observation
        JOIN route_searches searches ON searches.id = observation.search_id
        WHERE observation.provider_slug = $1 AND searches.status = 'finished' AND searches.counted_for_activity)
FROM observations
"#;
const BUCKETS: &str = r#"
WITH buckets AS (
    SELECT generate_series(
        date_trunc('day', now() AT TIME ZONE 'UTC') - ($2::integer - 1) * interval '1 day',
        date_trunc('day', now() AT TIME ZONE 'UTC'), interval '1 day'
    ) AT TIME ZONE 'UTC' AS started_at
), observations AS (
    SELECT observation.*, searches.created_at
    FROM provider_search_observations observation
    JOIN route_searches searches ON searches.id = observation.search_id
    WHERE observation.provider_slug = $1 AND searches.status = 'finished' AND searches.counted_for_activity
      AND searches.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' - ($2::integer - 1) * interval '1 day'
)
SELECT buckets.started_at, count(observations.search_id)::bigint,
       count(*) FILTER (WHERE observations.best_rank <= 10)::bigint,
       count(*) FILTER (WHERE observations.best_rank = 1)::bigint
FROM buckets LEFT JOIN observations
    ON observations.created_at >= buckets.started_at AND observations.created_at < buckets.started_at + interval '1 day'
GROUP BY buckets.started_at ORDER BY buckets.started_at
"#;
const DIRECTIONS: &str = r#"
SELECT searches.source_currency, searches.target_currency, count(*)::bigint,
       count(*) FILTER (WHERE observation.best_rank <= 10)::bigint, min(observation.best_rank)
FROM provider_search_observations observation
JOIN route_searches searches ON searches.id = observation.search_id
WHERE observation.provider_slug = $1 AND searches.status = 'finished' AND searches.counted_for_activity
  AND searches.source_currency IS NOT NULL AND searches.target_currency IS NOT NULL
  AND searches.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' - ($2::integer - 1) * interval '1 day'
GROUP BY searches.source_currency, searches.target_currency
ORDER BY count(*) DESC, searches.source_currency, searches.target_currency LIMIT 8
"#;

pub async fn record_search(
    client: &DbPoolObject,
    search_id: &Uuid,
    slugs: &[String],
    ranks: &[Option<i32>],
    successes: &[Option<bool>],
    latencies: &[Option<f64>],
) -> Result<()> {
    client
        .execute(
            INSERT_OBSERVATIONS,
            &[&search_id, &slugs, &ranks, &successes, &latencies],
        )
        .await?;
    Ok(())
}

pub async fn enabled_provider(client: &DbPoolObject, slug: &str) -> Result<bool> {
    Ok(client
        .query_opt(ENABLED_PROVIDER, &[&slug])
        .await?
        .is_some())
}

pub async fn now(tx: &Transaction<'_>) -> Result<DateTime<Utc>> {
    let row = tx.query_one(NOW, &[]).await?;
    Ok(row.get(0))
}

pub struct SummaryRow {
    pub searches: i64,
    pub top10: i64,
    pub top1: i64,
    pub average_rank: Option<f64>,
    pub response_samples: i64,
    pub successful_responses: i64,
    pub average_response_ms: Option<f64>,
    pub site_opens: i64,
    pub first_seen: Option<DateTime<Utc>>,
}

pub async fn summary(tx: &Transaction<'_>, slug: &str, days: i32) -> Result<SummaryRow> {
    let row = tx.query_one(SUMMARY, &[&slug, &days]).await?;
    Ok(SummaryRow {
        searches: row.get(0),
        top10: row.get(1),
        top1: row.get(2),
        average_rank: row.get(3),
        response_samples: row.get(4),
        successful_responses: row.get(5),
        average_response_ms: row.get(6),
        site_opens: row.get(7),
        first_seen: row.get(8),
    })
}

pub struct BucketRow {
    pub started_at: DateTime<Utc>,
    pub searches: i64,
    pub top10: i64,
    pub top1: i64,
}

pub async fn buckets(tx: &Transaction<'_>, slug: &str, days: i32) -> Result<Vec<BucketRow>> {
    Ok(tx
        .query(BUCKETS, &[&slug, &days])
        .await?
        .into_iter()
        .map(|row| BucketRow {
            started_at: row.get(0),
            searches: row.get(1),
            top10: row.get(2),
            top1: row.get(3),
        })
        .collect())
}

pub struct DirectionRow {
    pub source_currency: String,
    pub target_currency: String,
    pub searches: i64,
    pub top10: i64,
    pub best_rank: Option<i32>,
}

pub async fn directions(tx: &Transaction<'_>, slug: &str, days: i32) -> Result<Vec<DirectionRow>> {
    Ok(tx
        .query(DIRECTIONS, &[&slug, &days])
        .await?
        .into_iter()
        .map(|row| DirectionRow {
            source_currency: row.get(0),
            target_currency: row.get(1),
            searches: row.get(2),
            top10: row.get(3),
            best_rank: row.get(4),
        })
        .collect())
}

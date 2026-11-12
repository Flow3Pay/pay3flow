//! Service reputation, voting and search-activity statements.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient as PgClient;

/// Callers pass a pooled connection, a deadpool transaction or a bare
/// tokio_postgres transaction; all three implement this trait.
pub trait Conn: PgClient {}
impl Conn for crate::db::DbPoolObject {}
impl Conn for deadpool_postgres::Transaction<'_> {}
use uuid::Uuid;

pub type DbRow = tokio_postgres::Row;

const DECAY: &str = r#"
UPDATE services
SET reputation_score = GREATEST(0, reputation_score - 15),
    reputation_last_decay_at = now()
WHERE reputation_last_interaction_at <= now() - interval '1 day'
  AND reputation_last_decay_at <= now() - interval '1 day'
  AND reputation_score > 0
"#;
const PURGE_SPAM_SESSIONS: &str = r#"DELETE FROM reputation_spam_sessions
                                   WHERE session_started_at < now() - interval '1 day'"#;
const PURGE_VOTE_BASELINES: &str =
    "DELETE FROM {table} WHERE session_started_at < now() - interval '1 day'";
const LOCK_SESSION: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";
const SPAM_SESSION_EXISTS: &str = "SELECT EXISTS (SELECT 1 FROM reputation_spam_sessions WHERE anonymous_id = $1 AND session_started_at = $2)";
const MARK_SPAM_SESSION: &str = r#"
INSERT INTO reputation_spam_sessions (anonymous_id, session_started_at)
VALUES ($1, $2)
ON CONFLICT DO NOTHING
RETURNING anonymous_id
"#;
const AFFECTED_SERVICES: &str = r#"SELECT service_id FROM service_vote_session_baselines
                   WHERE anonymous_id = $1 AND session_started_at = $2
                   UNION
                   SELECT service_id FROM service_executions
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   UNION
                   SELECT service_id FROM service_instruction_opens
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   ORDER BY service_id"#;
const LOCK_SERVICES: &str = "SELECT id FROM services WHERE id = ANY($1) ORDER BY id FOR UPDATE";
const SERVICE_BASELINES: &str = r#"
SELECT service_id, previous_vote, reputation_score_delta
FROM service_vote_session_baselines
WHERE anonymous_id = $1 AND session_started_at = $2
ORDER BY service_id
FOR UPDATE
"#;
const SERVICE_VOTE: &str =
    "SELECT vote FROM service_votes WHERE anonymous_id = $1 AND service_id = $2 FOR UPDATE";
const RESTORE_SERVICE_VOTE: &str = r#"INSERT INTO service_votes (anonymous_id, service_id, vote)
                           VALUES ($1, $2, $3)
                           ON CONFLICT (anonymous_id, service_id) DO UPDATE SET
                               vote = EXCLUDED.vote, updated_at = now()"#;
const DELETE_SERVICE_VOTE: &str =
    "DELETE FROM service_votes WHERE anonymous_id = $1 AND service_id = $2";
const REVERT_SERVICE_TOTALS: &str = r#"UPDATE services SET
                           likes_total = GREATEST(0, likes_total + $2),
                           dislikes_total = GREATEST(0, dislikes_total + $3),
                           reputation_score = GREATEST(0, LEAST(100, reputation_score - $4)),
                           updated_at = now()
                       WHERE id = $1"#;
const ROUTE_BASELINES: &str = r#"
SELECT route_id, previous_vote
FROM route_vote_session_baselines
WHERE anonymous_id = $1 AND session_started_at = $2
ORDER BY route_id
FOR UPDATE
"#;
const RESTORE_ROUTE_VOTE: &str = r#"INSERT INTO route_votes (anonymous_id, route_id, vote)
                           VALUES ($1, $2, $3)
                           ON CONFLICT (anonymous_id, route_id) DO UPDATE SET
                               vote = EXCLUDED.vote, updated_at = now()"#;
const DELETE_ROUTE_VOTE: &str = "DELETE FROM route_votes WHERE anonymous_id = $1 AND route_id = $2";
const REMOVE_EXECUTIONS: &str = r#"DELETE FROM service_executions
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   RETURNING service_id, reputation_score_delta"#;
const REMOVE_INSTRUCTION_OPENS: &str = r#"DELETE FROM service_instruction_opens
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   RETURNING service_id, reputation_score_delta"#;
const REVERT_EXECUTION_TOTALS: &str = r#"UPDATE services SET
                           executions_total = GREATEST(0, executions_total - $2),
                           reputation_score = GREATEST(0, LEAST(100, reputation_score - $3)),
                           updated_at = now()
                       WHERE id = $1"#;
const DELETE_SERVICE_BASELINES: &str =
    "DELETE FROM service_vote_session_baselines WHERE anonymous_id = $1 AND session_started_at = $2";
const DELETE_ROUTE_BASELINES: &str =
    "DELETE FROM route_vote_session_baselines WHERE anonymous_id = $1 AND session_started_at = $2";
const INSERT_SEARCH: &str = r#"
INSERT INTO route_searches (id, source_currency, target_currency, anonymous_id, counted_for_activity, status, routes_found)
VALUES ($1, $2, $3, $4, $5, 'searching', 0)
ON CONFLICT (id) DO NOTHING
"#;
const OLDEST_SEARCH: &str = "SELECT min(created_at) FROM route_searches WHERE source_currency = $1 AND target_currency = $2 AND counted_for_activity AND status = 'finished'";
const SEARCH_ACTIVITY: &str = r#"
WITH bounds AS (
    SELECT date_trunc($3::text, now()) AS end_bucket,
           CASE $3::text
               WHEN 'minute' THEN interval '1 minute'
               WHEN 'hour' THEN interval '1 hour'
               WHEN 'day' THEN interval '1 day'
               WHEN 'week' THEN interval '1 week'
               ELSE interval '1 month'
           END AS bucket_size
), hours AS (
    SELECT generate_series(
        least(date_trunc($3::text, coalesce($5::timestamptz, now() - ($4::text)::interval)), end_bucket - bucket_size),
        end_bucket,
        bucket_size
    ) AS started_at, bucket_size
    FROM bounds
)
SELECT hours.started_at, count(searches.id)::bigint
FROM hours
LEFT JOIN route_searches searches
    ON searches.created_at >= hours.started_at
    AND searches.created_at < hours.started_at + hours.bucket_size
    AND searches.source_currency = $1
    AND searches.target_currency = $2
    AND searches.counted_for_activity
    AND searches.status = 'finished'
GROUP BY hours.started_at
ORDER BY hours.started_at
"#;
const SEARCHES_LAST_HOUR: &str = "SELECT COUNT(*)::BIGINT FROM route_searches \
                 WHERE created_at >= now() - INTERVAL '1 hour'";
const FINISH_SEARCH: &str = r#"
UPDATE route_searches
SET routes_found = $2, status = $3, updated_at = now()
WHERE id = $1
"#;
const SERVICE_STATS: &str = r#"SELECT s.id, s.slug, s.display_name, s.executions_total,
                              s.likes_total, s.dislikes_total, s.reputation_score, v.vote
                       FROM services s
                       LEFT JOIN service_votes v
                         ON v.service_id = s.id AND v.anonymous_id = $2
                       WHERE s.id = $1"#;
const LOCK_SERVICE_SCORE: &str = "SELECT reputation_score FROM services WHERE id = $1 FOR UPDATE";
const INSERT_EXECUTION: &str = r#"
INSERT INTO service_executions
    (service_id, search_id, route_id, anonymous_id, status)
VALUES ($1, $2, $3, $4, 'started')
ON CONFLICT (anonymous_id, service_id, search_id, route_id) DO NOTHING
RETURNING id
"#;
const BUMP_EXECUTION_TOTALS: &str = r#"
UPDATE services
SET executions_total = executions_total + 1,
    reputation_score = LEAST(100, reputation_score + 5),
    reputation_last_interaction_at = now(),
    reputation_last_decay_at = now(),
    updated_at = now()
WHERE id = $1
RETURNING reputation_score
"#;
const SET_EXECUTION_DELTA: &str =
    "UPDATE service_executions SET reputation_score_delta = $2 WHERE id = $1";
const EXECUTION_BY_KEY: &str = r#"
SELECT id FROM service_executions
WHERE anonymous_id = $1 AND service_id = $2 AND search_id = $3 AND route_id = $4
"#;
const PLEDGED_SERVICE_STATS: &str = r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, s.reputation_score, v.vote
FROM services s
LEFT JOIN service_votes v
  ON v.service_id = s.id AND v.anonymous_id = $2
WHERE s.id = $1
"#;
const SESSION_SERVICE_VOTE: &str = r#"
SELECT vote FROM service_votes
WHERE service_id = $1 AND anonymous_id = $2
FOR UPDATE
"#;
const INSERT_SERVICE_BASELINE: &str = r#"INSERT INTO service_vote_session_baselines
                       (anonymous_id, session_started_at, service_id, previous_vote)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT DO NOTHING"#;
const UPSERT_SERVICE_VOTE: &str = r#"
INSERT INTO service_votes (anonymous_id, service_id, vote)
VALUES ($1, $2, $3)
ON CONFLICT (anonymous_id, service_id) DO UPDATE SET
    vote = EXCLUDED.vote,
    updated_at = now()
"#;
const APPLY_SERVICE_VOTE: &str = r#"
UPDATE services
SET likes_total = likes_total + $2,
    dislikes_total = dislikes_total + $3,
    reputation_score = GREATEST(0, LEAST(100, reputation_score + $4)),
    reputation_last_interaction_at = CASE WHEN $5 THEN now() ELSE reputation_last_interaction_at END,
    reputation_last_decay_at = CASE WHEN $5 THEN now() ELSE reputation_last_decay_at END,
    updated_at = now()
WHERE id = $1
RETURNING reputation_score
"#;
const ADD_SERVICE_BASELINE_DELTA: &str = r#"UPDATE service_vote_session_baselines
                   SET reputation_score_delta = reputation_score_delta + $4
                   WHERE anonymous_id = $1 AND session_started_at = $2 AND service_id = $3"#;
const VOTED_SERVICE_STATS: &str = r#"
SELECT id, slug, display_name, executions_total, likes_total, dislikes_total,
       reputation_score, $2::TEXT
FROM services WHERE id = $1
"#;
const INSERT_INSTRUCTION_OPEN: &str = r#"
INSERT INTO service_instruction_opens (anonymous_id, service_id, search_id, route_id)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING
"#;
const BUMP_INSTRUCTION_SCORE: &str = "UPDATE services SET reputation_score = LEAST(100, reputation_score + 5), reputation_last_interaction_at = now(), reputation_last_decay_at = now(), updated_at = now() WHERE id = $1 RETURNING reputation_score";
const SET_INSTRUCTION_DELTA: &str = r#"UPDATE service_instruction_opens
                           SET reputation_score_delta = $5
                           WHERE anonymous_id = $1 AND service_id = $2 AND search_id = $3 AND route_id = $4"#;
const ROUTE_FEEDBACK: &str = r#"SELECT COUNT(*) FILTER (WHERE vote = 'like')::BIGINT,
                              COUNT(*) FILTER (WHERE vote = 'dislike')::BIGINT,
                              (SELECT vote::TEXT FROM route_votes
                               WHERE route_id = $1 AND anonymous_id = $2)
                       FROM route_votes WHERE route_id = $1"#;
const SESSION_ROUTE_VOTE: &str =
    "SELECT vote FROM route_votes WHERE route_id = $1 AND anonymous_id = $2 FOR UPDATE";
const INSERT_ROUTE_BASELINE: &str = r#"INSERT INTO route_vote_session_baselines
                       (anonymous_id, session_started_at, route_id, previous_vote)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT DO NOTHING"#;
const UPSERT_ROUTE_VOTE: &str = r#"
INSERT INTO route_votes (anonymous_id, route_id, vote)
VALUES ($1, $2, $3)
ON CONFLICT (anonymous_id, route_id) DO UPDATE SET
    vote = EXCLUDED.vote,
    updated_at = now()
"#;
const UPDATED_ROUTE_FEEDBACK: &str = r#"
SELECT COUNT(*) FILTER (WHERE vote = 'like')::BIGINT,
       COUNT(*) FILTER (WHERE vote = 'dislike')::BIGINT,
       (SELECT vote::TEXT FROM route_votes
        WHERE route_id = $1 AND anonymous_id = $2)
FROM route_votes
WHERE route_id = $1
"#;

const UPSERT_SERVICE: &str = r#"
INSERT INTO services (slug, display_name)
VALUES ($1, $2)
ON CONFLICT (slug) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at = now()
WHERE services.display_name IS DISTINCT FROM EXCLUDED.display_name
"#;
const ALL_SERVICE_STATS: &str = r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, s.reputation_score, NULL::TEXT
FROM services s
"#;
const VIEWER_VOTES: &str = r#"
SELECT s.slug, v.vote
FROM services s
JOIN service_votes v ON v.service_id = s.id
WHERE s.slug = ANY($1) AND v.anonymous_id = $2
"#;
const ROUTE_FEEDBACKS: &str = r#"
SELECT requested.route_id,
       COUNT(votes.*) FILTER (WHERE votes.vote = 'like')::BIGINT,
       COUNT(votes.*) FILTER (WHERE votes.vote = 'dislike')::BIGINT,
       viewer.vote
FROM unnest($1::TEXT[]) AS requested(route_id)
LEFT JOIN route_votes votes ON votes.route_id = requested.route_id
LEFT JOIN route_votes viewer
  ON viewer.route_id = requested.route_id AND viewer.anonymous_id = $2
GROUP BY requested.route_id, viewer.vote
"#;

pub async fn decay<C>(client: &C) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(DECAY, &[]).await?;
    Ok(())
}

pub async fn purge_spam_sessions<C>(client: &C) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.execute(PURGE_SPAM_SESSIONS, &[]).await?;
    Ok(())
}

/// Removes expired baselines from one of the vote-session baseline tables.
pub async fn purge_vote_baselines<C>(client: &C, table: &str) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(&PURGE_VOTE_BASELINES.replace("{table}", table), &[])
        .await?;
    Ok(())
}

pub async fn lock_session<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    let key = format!("{anonymous_id}:{}", session_started_at.timestamp());
    client.query_one(LOCK_SESSION, &[&key]).await?;
    Ok(())
}

pub async fn spam_session_exists<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    let row = client
        .query_one(SPAM_SESSION_EXISTS, &[&anonymous_id, &session_started_at])
        .await?;
    Ok(row.get(0))
}

pub async fn mark_spam_session<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<bool>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(MARK_SPAM_SESSION, &[&anonymous_id, &session_started_at])
        .await?
        .is_some())
}

pub async fn affected_services<C>(
    client: &C,
    anonymous_id: Uuid,
    started_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> Result<Vec<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(AFFECTED_SERVICES, &[&anonymous_id, &started_at, &ends_at])
        .await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

pub async fn lock_services<C>(client: &C, ids: &[Uuid]) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client.query(LOCK_SERVICES, &[&ids]).await?;
    Ok(())
}

/// `(service_id, previous_vote, reputation_score_delta)`.
pub async fn service_baselines<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<Vec<(Uuid, Option<String>, i16)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(SERVICE_BASELINES, &[&anonymous_id, &session_started_at])
        .await?;
    Ok(rows
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

pub async fn service_vote<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
) -> Result<Option<String>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(SERVICE_VOTE, &[&anonymous_id, &service_id])
        .await?
        .map(|row| row.get(0)))
}

pub async fn restore_service_vote<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
    vote: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(RESTORE_SERVICE_VOTE, &[&anonymous_id, &service_id, &vote])
        .await?;
    Ok(())
}

pub async fn delete_service_vote<C>(client: &C, anonymous_id: Uuid, service_id: Uuid) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(DELETE_SERVICE_VOTE, &[&anonymous_id, &service_id])
        .await?;
    Ok(())
}

pub async fn revert_service_totals<C>(
    client: &C,
    service_id: Uuid,
    likes_delta: i64,
    dislikes_delta: i64,
    score_delta: i16,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            REVERT_SERVICE_TOTALS,
            &[&service_id, &likes_delta, &dislikes_delta, &score_delta],
        )
        .await?;
    Ok(())
}

/// `(route_id, previous_vote)`.
pub async fn route_baselines<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<Vec<(String, Option<String>)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(ROUTE_BASELINES, &[&anonymous_id, &session_started_at])
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub async fn restore_route_vote<C>(
    client: &C,
    anonymous_id: Uuid,
    route_id: &str,
    vote: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(RESTORE_ROUTE_VOTE, &[&anonymous_id, &route_id, &vote])
        .await?;
    Ok(())
}

pub async fn delete_route_vote<C>(client: &C, anonymous_id: Uuid, route_id: &str) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(DELETE_ROUTE_VOTE, &[&anonymous_id, &route_id])
        .await?;
    Ok(())
}

/// `(service_id, reputation_score_delta)`.
pub async fn remove_executions<C>(
    client: &C,
    anonymous_id: Uuid,
    started_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> Result<Vec<(Uuid, i16)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(REMOVE_EXECUTIONS, &[&anonymous_id, &started_at, &ends_at])
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// `(service_id, reputation_score_delta)`.
pub async fn remove_instruction_opens<C>(
    client: &C,
    anonymous_id: Uuid,
    started_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
) -> Result<Vec<(Uuid, i16)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(
            REMOVE_INSTRUCTION_OPENS,
            &[&anonymous_id, &started_at, &ends_at],
        )
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub async fn revert_execution_totals<C>(
    client: &C,
    service_id: Uuid,
    executions: i64,
    score_delta: i64,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            REVERT_EXECUTION_TOTALS,
            &[&service_id, &executions, &score_delta],
        )
        .await?;
    Ok(())
}

pub async fn delete_service_baselines<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            DELETE_SERVICE_BASELINES,
            &[&anonymous_id, &session_started_at],
        )
        .await?;
    Ok(())
}

pub async fn delete_route_baselines<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            DELETE_ROUTE_BASELINES,
            &[&anonymous_id, &session_started_at],
        )
        .await?;
    Ok(())
}

pub async fn insert_search<C>(
    client: &C,
    search_id: Uuid,
    source_currency: &str,
    target_currency: &str,
    anonymous_id: Option<Uuid>,
    count_activity: bool,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_SEARCH,
            &[
                &search_id,
                &source_currency,
                &target_currency,
                &anonymous_id,
                &count_activity,
            ],
        )
        .await?;
    Ok(())
}

pub async fn oldest_search<C>(
    client: &C,
    source_currency: &str,
    target_currency: &str,
) -> Result<Option<DateTime<Utc>>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(OLDEST_SEARCH, &[&source_currency, &target_currency])
        .await?
        .get(0))
}

/// `(bucket_started_at, count)` per search-activity bucket.
pub async fn search_activity<C>(
    client: &C,
    source_currency: &str,
    target_currency: &str,
    grain: &str,
    span: &str,
    oldest: Option<DateTime<Utc>>,
) -> Result<Vec<(DateTime<Utc>, i64)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client
        .query(
            SEARCH_ACTIVITY,
            &[&source_currency, &target_currency, &grain, &span, &oldest],
        )
        .await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

pub async fn count_searches_last_hour<C>(client: &C) -> Result<i64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client.query_one(SEARCHES_LAST_HOUR, &[]).await?.get(0))
}

pub async fn finish_search<C>(
    client: &C,
    search_id: Uuid,
    routes_found: i64,
    status: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(FINISH_SEARCH, &[&search_id, &routes_found, &status])
        .await?;
    Ok(())
}

/// `(id, slug, display_name, executions, likes, dislikes, score, vote)`.
pub async fn service_stats<C>(
    client: &C,
    service_id: Uuid,
    anonymous_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(SERVICE_STATS, &[&service_id, &anonymous_id])
        .await?)
}

pub async fn lock_service_score<C>(client: &C, service_id: Uuid) -> Result<Option<i16>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(LOCK_SERVICE_SCORE, &[&service_id])
        .await?
        .map(|row| row.get(0)))
}

pub async fn insert_execution<C>(
    client: &C,
    service_id: Uuid,
    search_id: Uuid,
    route_id: &str,
    anonymous_id: Uuid,
) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(
            INSERT_EXECUTION,
            &[&service_id, &search_id, &route_id, &anonymous_id],
        )
        .await?
        .map(|row| row.get(0)))
}

pub async fn bump_execution_totals<C>(client: &C, service_id: Uuid) -> Result<i16>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(BUMP_EXECUTION_TOTALS, &[&service_id])
        .await?
        .get(0))
}

pub async fn set_execution_delta<C>(client: &C, execution_id: Uuid, score_delta: i16) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(SET_EXECUTION_DELTA, &[&execution_id, &score_delta])
        .await?;
    Ok(())
}

pub async fn execution_by_key<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
    search_id: Uuid,
    route_id: &str,
) -> Result<Option<Uuid>>
where
    C: Conn + Sync + ?Sized,
{
    let Some(row) = client
        .query_opt(
            EXECUTION_BY_KEY,
            &[&anonymous_id, &service_id, &search_id, &route_id],
        )
        .await?
    else {
        return Err(anyhow!("execution conflict winner was not found"));
    };
    Ok(Some(row.get(0)))
}

pub async fn pledged_service_stats<C>(
    client: &C,
    service_id: Uuid,
    anonymous_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(PLEDGED_SERVICE_STATS, &[&service_id, &anonymous_id])
        .await?)
}

pub async fn session_service_vote<C>(
    client: &C,
    service_id: Uuid,
    anonymous_id: Uuid,
) -> Result<Option<String>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(SESSION_SERVICE_VOTE, &[&service_id, &anonymous_id])
        .await?
        .map(|row| row.get(0)))
}

pub async fn insert_service_baseline<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
    service_id: Uuid,
    previous_vote: Option<&str>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_SERVICE_BASELINE,
            &[
                &anonymous_id,
                &session_started_at,
                &service_id,
                &previous_vote,
            ],
        )
        .await?;
    Ok(())
}

pub async fn upsert_service_vote<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
    vote: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(UPSERT_SERVICE_VOTE, &[&anonymous_id, &service_id, &vote])
        .await?;
    Ok(())
}

pub async fn apply_service_vote<C>(
    client: &C,
    service_id: Uuid,
    likes_delta: i64,
    dislikes_delta: i64,
    score_delta: i16,
    new_vote: bool,
) -> Result<i16>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(
            APPLY_SERVICE_VOTE,
            &[
                &service_id,
                &likes_delta,
                &dislikes_delta,
                &score_delta,
                &new_vote,
            ],
        )
        .await?
        .get(0))
}

pub async fn add_service_baseline_delta<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
    service_id: Uuid,
    score_delta: i16,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            ADD_SERVICE_BASELINE_DELTA,
            &[
                &anonymous_id,
                &session_started_at,
                &service_id,
                &score_delta,
            ],
        )
        .await?;
    Ok(())
}

pub async fn voted_service_stats<C>(
    client: &C,
    service_id: Uuid,
    vote: &str,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(VOTED_SERVICE_STATS, &[&service_id, &vote])
        .await?)
}

pub async fn insert_instruction_open<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
    search_id: Uuid,
    route_id: &str,
) -> Result<u64>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .execute(
            INSERT_INSTRUCTION_OPEN,
            &[&anonymous_id, &service_id, &search_id, &route_id],
        )
        .await?)
}

pub async fn bump_instruction_score<C>(client: &C, service_id: Uuid) -> Result<i16>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(BUMP_INSTRUCTION_SCORE, &[&service_id])
        .await?
        .get(0))
}

pub async fn set_instruction_delta<C>(
    client: &C,
    anonymous_id: Uuid,
    service_id: Uuid,
    search_id: Uuid,
    route_id: &str,
    score_delta: i16,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            SET_INSTRUCTION_DELTA,
            &[
                &anonymous_id,
                &service_id,
                &search_id,
                &route_id,
                &score_delta,
            ],
        )
        .await?;
    Ok(())
}

pub async fn route_feedback<C>(
    client: &C,
    route_id: &str,
    anonymous_id: Uuid,
) -> Result<Option<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(ROUTE_FEEDBACK, &[&route_id, &anonymous_id])
        .await?)
}

pub async fn session_route_vote<C>(
    client: &C,
    route_id: &str,
    anonymous_id: Uuid,
) -> Result<Option<String>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_opt(SESSION_ROUTE_VOTE, &[&route_id, &anonymous_id])
        .await?
        .map(|row| row.get(0)))
}

pub async fn insert_route_baseline<C>(
    client: &C,
    anonymous_id: Uuid,
    session_started_at: DateTime<Utc>,
    route_id: &str,
    previous_vote: Option<&str>,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(
            INSERT_ROUTE_BASELINE,
            &[
                &anonymous_id,
                &session_started_at,
                &route_id,
                &previous_vote,
            ],
        )
        .await?;
    Ok(())
}

pub async fn upsert_route_vote<C>(
    client: &C,
    anonymous_id: Uuid,
    route_id: &str,
    vote: &str,
) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(UPSERT_ROUTE_VOTE, &[&anonymous_id, &route_id, &vote])
        .await?;
    Ok(())
}

pub async fn updated_route_feedback<C>(
    client: &C,
    route_id: &str,
    anonymous_id: Uuid,
) -> Result<tokio_postgres::Row>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query_one(UPDATED_ROUTE_FEEDBACK, &[&route_id, &anonymous_id])
        .await?)
}

pub async fn upsert_service<C>(client: &C, slug: &str, display_name: &str) -> Result<()>
where
    C: Conn + Sync + ?Sized,
{
    client
        .execute(UPSERT_SERVICE, &[&slug, &display_name])
        .await?;
    Ok(())
}

pub async fn all_service_stats<C>(client: &C) -> Result<Vec<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client.query(ALL_SERVICE_STATS, &[]).await?)
}

/// `(slug, vote)` for the viewer's votes on the requested slugs.
pub async fn viewer_votes<C>(
    client: &C,
    slugs: &[String],
    anonymous_id: Uuid,
) -> Result<Vec<(String, String)>>
where
    C: Conn + Sync + ?Sized,
{
    let rows = client.query(VIEWER_VOTES, &[&slugs, &anonymous_id]).await?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// `(route_id, likes, dislikes, viewer_vote)` per requested route.
pub async fn route_feedbacks<C>(
    client: &C,
    route_ids: &[String],
    anonymous_id: Option<Uuid>,
) -> Result<Vec<tokio_postgres::Row>>
where
    C: Conn + Sync + ?Sized,
{
    Ok(client
        .query(ROUTE_FEEDBACKS, &[&route_ids, &anonymous_id])
        .await?)
}

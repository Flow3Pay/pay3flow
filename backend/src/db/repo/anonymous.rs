use anyhow::Result;
use deadpool_postgres::Transaction;
use uuid::Uuid;

use crate::db::DbPoolObject;

const INSERT_USER: &str = r#"
INSERT INTO anonymous_users (id, request_count)
VALUES ($1, 0)
ON CONFLICT (id) DO NOTHING
"#;
const LOCK_USER: &str = r#"
SELECT last_session_at IS NULL
    OR last_seen_at <= now() - INTERVAL '30 minutes'
FROM anonymous_users
WHERE id = $1
FOR UPDATE
"#;
const BUMP_REQUEST: &str = r#"
UPDATE anonymous_users
SET request_count = request_count + 1,
    last_seen_at = now(),
    last_session_at = CASE WHEN $2 THEN now() ELSE last_session_at END
WHERE id = $1
"#;
const INSERT_SESSION: &str = "INSERT INTO anonymous_user_sessions (anonymous_id) VALUES ($1)";
const COUNT_USERS: &str = "SELECT (SELECT COUNT(*)::BIGINT FROM anonymous_users), \
                    (SELECT COUNT(*)::BIGINT FROM users)";
const ACTIVITY_COUNTS: &str = r#"
SELECT
    COUNT(DISTINCT anonymous_id) FILTER (
        WHERE started_at >= now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(DISTINCT anonymous_id) FILTER (
        WHERE started_at >= now() - INTERVAL '14 days'
          AND started_at < now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(*) FILTER (
        WHERE started_at >= now() - INTERVAL '7 days'
    )::BIGINT,
    COUNT(*) FILTER (
        WHERE started_at >= now() - INTERVAL '14 days'
          AND started_at < now() - INTERVAL '7 days'
    )::BIGINT
FROM anonymous_user_sessions
"#;

pub async fn insert_user(tx: &Transaction<'_>, id: &Uuid) -> Result<bool> {
    Ok(tx.execute(INSERT_USER, &[&id]).await? == 1)
}

pub async fn lock_user(tx: &Transaction<'_>, id: &Uuid) -> Result<bool> {
    let row = tx.query_one(LOCK_USER, &[&id]).await?;
    Ok(row.get(0))
}

pub async fn bump_request(tx: &Transaction<'_>, id: &Uuid, starts_session: bool) -> Result<()> {
    tx.execute(BUMP_REQUEST, &[&id, &starts_session]).await?;
    Ok(())
}

pub async fn insert_session(tx: &Transaction<'_>, id: &Uuid) -> Result<()> {
    tx.execute(INSERT_SESSION, &[&id]).await?;
    Ok(())
}

pub async fn count_users(client: &DbPoolObject) -> Result<(i64, i64)> {
    let row = client.query_one(COUNT_USERS, &[]).await?;
    Ok((row.get(0), row.get(1)))
}

pub async fn activity_counts(client: &DbPoolObject) -> Result<(i64, i64, i64, i64)> {
    let row = client.query_one(ACTIVITY_COUNTS, &[]).await?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

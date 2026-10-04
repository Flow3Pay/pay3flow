use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::core::redis::{get_json, set_json, RedisPool};
use crate::db::DbPool;

const TRACKING_TOKEN_TTL_SECS: u64 = 30 * 60;
const REPUTATION_SESSION_SECS: u64 = 10 * 60;
const REPUTATION_EVENT_LIMIT_PER_SESSION: i64 = 100;

#[derive(Debug, Error)]
pub enum ReputationError {
    #[error("service not found")]
    ServiceNotFound,
    #[error("invalid or expired service link")]
    InvalidTrackingToken,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Debug, Serialize)]
pub struct RouteSearchActivityHour {
    pub started_at: DateTime<Utc>,
    pub count: i64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub enum SearchActivityPeriod {
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "1d")]
    Day,
    #[default]
    #[serde(rename = "1w")]
    Week,
    #[serde(rename = "1m")]
    Month,
    #[serde(rename = "3m")]
    ThreeMonths,
    #[serde(rename = "6m")]
    SixMonths,
    #[serde(rename = "1y")]
    Year,
    #[serde(rename = "all")]
    All,
}

impl SearchActivityPeriod {
    fn aggregation(self, oldest: Option<DateTime<Utc>>) -> (&'static str, &'static str) {
        match self {
            Self::Hour => ("minute", "59 minutes"),
            Self::Day => ("hour", "23 hours"),
            Self::Week => ("hour", "167 hours"),
            Self::Month => ("day", "29 days"),
            Self::ThreeMonths => ("day", "89 days"),
            Self::SixMonths => ("day", "179 days"),
            Self::Year => ("day", "364 days"),
            Self::All => {
                let age = oldest.map(|started| Utc::now().signed_duration_since(started));
                let grain = match age {
                    Some(age) if age <= Duration::days(7) => "hour",
                    Some(age) if age <= Duration::days(365) => "day",
                    Some(age) if age <= Duration::days(365 * 5) => "week",
                    Some(_) => "month",
                    None => "hour",
                };
                (grain, "0 seconds")
            }
        }
    }
}

impl From<tokio_postgres::Error> for ReputationError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<deadpool_postgres::PoolError> for ReputationError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Internal(error.into())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoteChoice {
    Like,
    Dislike,
}

impl VoteChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Like => "like",
            Self::Dislike => "dislike",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "like" => Some(Self::Like),
            "dislike" => Some(Self::Dislike),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceStats {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub executions_total: i64,
    pub likes_total: i64,
    pub dislikes_total: i64,
    #[serde(default = "default_reputation_score")]
    pub reputation_score: i16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer_vote: Option<VoteChoice>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteServiceStats {
    #[serde(flatten)]
    pub stats: ServiceStats,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceLinkKind {
    Entry,
    Exit,
    MarketSource,
    MarketTarget,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceLink {
    pub service_id: Uuid,
    pub service_slug: String,
    pub kind: ServiceLinkKind,
    pub tracking_token: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CombinedReputation {
    pub executions_average: i64,
    pub likes_average: i64,
    pub dislikes_average: i64,
}

/// Anonymous feedback for one concrete route shown in a search result.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteFeedback {
    pub likes_total: i64,
    pub dislikes_total: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer_vote: Option<VoteChoice>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionOpen {
    pub execution_id: Uuid,
    pub newly_recorded: bool,
    pub redirect_url: String,
    pub service: ServiceStats,
}

#[derive(Debug, Serialize)]
pub struct InstructionOpen {
    pub newly_recorded: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackingClaims {
    service_id: Uuid,
    search_id: Uuid,
    route_id: String,
    destination_url: String,
    exp: u64,
}

#[derive(Serialize)]
struct ReputationEventSnapshot<'a> {
    event: &'a str,
    anonymous_id: Uuid,
    service_id: Option<Uuid>,
    search_id: Uuid,
    route_id: &'a str,
    vote: Option<VoteChoice>,
    session_started_at: DateTime<Utc>,
}

#[derive(Clone, Copy)]
struct ReputationEventStatus {
    session_started_at: DateTime<Utc>,
    spam: bool,
}

#[derive(Clone)]
pub struct ServiceReputation {
    pool: DbPool,
    redis: Option<RedisPool>,
    encode_key: EncodingKey,
    decode_key: DecodingKey,
}

impl ServiceReputation {
    pub fn new(pool: DbPool, redis: Option<RedisPool>, secret: &str) -> Self {
        Self {
            pool,
            redis,
            encode_key: EncodingKey::from_secret(secret.as_bytes()),
            decode_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn start_daily_decay(&self) {
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            loop {
                interval.tick().await;
                match pool.get().await {
                    Ok(client) => {
                        if let Err(error) = client
                            .execute(
                                r#"
UPDATE services
SET reputation_score = GREATEST(0, reputation_score - 15),
    reputation_last_decay_at = now()
WHERE reputation_last_interaction_at <= now() - interval '1 day'
  AND reputation_last_decay_at <= now() - interval '1 day'
  AND reputation_score > 0
"#,
                                &[],
                            )
                            .await
                        {
                            tracing::warn!(%error, "service.reputation.daily_decay_failed");
                        }
                        if let Err(error) = client
                            .execute(
                                r#"DELETE FROM reputation_spam_sessions
                                   WHERE session_started_at < now() - interval '1 day'"#,
                                &[],
                            )
                            .await
                        {
                            tracing::warn!(%error, "service.reputation.spam_session_cleanup_failed");
                        }
                        for table in [
                            "service_vote_session_baselines",
                            "route_vote_session_baselines",
                        ] {
                            let query = format!(
                                "DELETE FROM {table} WHERE session_started_at < now() - interval '1 day'"
                            );
                            if let Err(error) = client.execute(&query, &[]).await {
                                tracing::warn!(%error, table, "service.reputation.vote_baseline_cleanup_failed");
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "service.reputation.daily_decay_db_unavailable")
                    }
                }
            }
        });
    }

    async fn lock_reputation_session(
        transaction: &tokio_postgres::Transaction<'_>,
        anonymous_id: Uuid,
        session_started_at: DateTime<Utc>,
    ) -> Result<bool, ReputationError> {
        let lock_key = format!("{anonymous_id}:{}", session_started_at.timestamp());
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                &[&lock_key],
            )
            .await?;
        let row = transaction
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM reputation_spam_sessions WHERE anonymous_id = $1 AND session_started_at = $2)",
                &[&anonymous_id, &session_started_at],
            )
            .await?;
        Ok(row.get(0))
    }

    async fn rollback_spam_session(
        &self,
        anonymous_id: Uuid,
        session_started_at: DateTime<Utc>,
    ) -> Result<(), ReputationError> {
        let session_ends_at =
            session_started_at + chrono::Duration::seconds(REPUTATION_SESSION_SECS as i64);
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        let lock_key = format!("{anonymous_id}:{}", session_started_at.timestamp());
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                &[&lock_key],
            )
            .await?;
        let newly_marked = transaction
            .query_opt(
                r#"
INSERT INTO reputation_spam_sessions (anonymous_id, session_started_at)
VALUES ($1, $2)
ON CONFLICT DO NOTHING
RETURNING anonymous_id
"#,
                &[&anonymous_id, &session_started_at],
            )
            .await?
            .is_some();
        if !newly_marked {
            transaction.commit().await?;
            return Ok(());
        }

        let affected_services = transaction
            .query(
                r#"SELECT service_id FROM service_vote_session_baselines
                   WHERE anonymous_id = $1 AND session_started_at = $2
                   UNION
                   SELECT service_id FROM service_executions
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   UNION
                   SELECT service_id FROM service_instruction_opens
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   ORDER BY service_id"#,
                &[&anonymous_id, &session_started_at, &session_ends_at],
            )
            .await?
            .into_iter()
            .map(|row| row.get::<_, Uuid>(0))
            .collect::<Vec<_>>();
        transaction
            .query(
                "SELECT id FROM services WHERE id = ANY($1) ORDER BY id FOR UPDATE",
                &[&affected_services],
            )
            .await?;

        let service_baselines = transaction
            .query(
                r#"
SELECT service_id, previous_vote, reputation_score_delta
FROM service_vote_session_baselines
WHERE anonymous_id = $1 AND session_started_at = $2
ORDER BY service_id
FOR UPDATE
"#,
                &[&anonymous_id, &session_started_at],
            )
            .await?;
        for row in service_baselines {
            let service_id: Uuid = row.get(0);
            let previous = row
                .get::<_, Option<String>>(1)
                .as_deref()
                .and_then(VoteChoice::parse);
            let score_delta: i16 = row.get(2);
            let current = transaction
                .query_opt(
                    "SELECT vote FROM service_votes WHERE anonymous_id = $1 AND service_id = $2 FOR UPDATE",
                    &[&anonymous_id, &service_id],
                )
                .await?
                .and_then(|row| VoteChoice::parse(row.get::<_, String>(0).as_str()));
            let (likes_delta, dislikes_delta, _) = vote_state_deltas(current, previous);
            match previous {
                Some(previous) => {
                    transaction
                        .execute(
                            r#"INSERT INTO service_votes (anonymous_id, service_id, vote)
                           VALUES ($1, $2, $3)
                           ON CONFLICT (anonymous_id, service_id) DO UPDATE SET
                               vote = EXCLUDED.vote, updated_at = now()"#,
                            &[&anonymous_id, &service_id, &previous.as_str()],
                        )
                        .await?;
                }
                None => {
                    transaction
                        .execute(
                            "DELETE FROM service_votes WHERE anonymous_id = $1 AND service_id = $2",
                            &[&anonymous_id, &service_id],
                        )
                        .await?;
                }
            }
            transaction
                .execute(
                    r#"UPDATE services SET
                           likes_total = GREATEST(0, likes_total + $2),
                           dislikes_total = GREATEST(0, dislikes_total + $3),
                           reputation_score = GREATEST(0, LEAST(100, reputation_score - $4)),
                           updated_at = now()
                       WHERE id = $1"#,
                    &[&service_id, &likes_delta, &dislikes_delta, &score_delta],
                )
                .await?;
        }

        let route_baselines = transaction
            .query(
                r#"
SELECT route_id, previous_vote
FROM route_vote_session_baselines
WHERE anonymous_id = $1 AND session_started_at = $2
ORDER BY route_id
FOR UPDATE
"#,
                &[&anonymous_id, &session_started_at],
            )
            .await?;
        for row in route_baselines {
            let route_id: String = row.get(0);
            let previous = row
                .get::<_, Option<String>>(1)
                .as_deref()
                .and_then(VoteChoice::parse);
            match previous {
                Some(previous) => {
                    transaction
                        .execute(
                            r#"INSERT INTO route_votes (anonymous_id, route_id, vote)
                           VALUES ($1, $2, $3)
                           ON CONFLICT (anonymous_id, route_id) DO UPDATE SET
                               vote = EXCLUDED.vote, updated_at = now()"#,
                            &[&anonymous_id, &route_id, &previous.as_str()],
                        )
                        .await?;
                }
                None => {
                    transaction
                        .execute(
                            "DELETE FROM route_votes WHERE anonymous_id = $1 AND route_id = $2",
                            &[&anonymous_id, &route_id],
                        )
                        .await?;
                }
            }
        }

        let removed_executions = transaction
            .query(
                r#"DELETE FROM service_executions
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   RETURNING service_id, reputation_score_delta"#,
                &[&anonymous_id, &session_started_at, &session_ends_at],
            )
            .await?;
        let removed_instructions = transaction
            .query(
                r#"DELETE FROM service_instruction_opens
                   WHERE anonymous_id = $1 AND created_at >= $2 AND created_at < $3
                   RETURNING service_id, reputation_score_delta"#,
                &[&anonymous_id, &session_started_at, &session_ends_at],
            )
            .await?;
        let mut removed_by_service = HashMap::<Uuid, (i64, i64, i64)>::new();
        for row in &removed_executions {
            let entry = removed_by_service.entry(row.get(0)).or_default();
            entry.0 += 1;
            entry.2 += i64::from(row.get::<_, i16>(1));
        }
        for row in &removed_instructions {
            let entry = removed_by_service.entry(row.get(0)).or_default();
            entry.1 += 1;
            entry.2 += i64::from(row.get::<_, i16>(1));
        }
        for (service_id, (executions, _instructions, score_delta)) in removed_by_service {
            transaction
                .execute(
                    r#"UPDATE services SET
                           executions_total = GREATEST(0, executions_total - $2),
                           reputation_score = GREATEST(0, LEAST(100, reputation_score - $3)),
                           updated_at = now()
                       WHERE id = $1"#,
                    &[&service_id, &executions, &score_delta],
                )
                .await?;
        }
        transaction
            .execute(
                "DELETE FROM service_vote_session_baselines WHERE anonymous_id = $1 AND session_started_at = $2",
                &[&anonymous_id, &session_started_at],
            )
            .await?;
        transaction
            .execute(
                "DELETE FROM route_vote_session_baselines WHERE anonymous_id = $1 AND session_started_at = $2",
                &[&anonymous_id, &session_started_at],
            )
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn cache_event_first(
        &self,
        event: &str,
        anonymous_id: Uuid,
        service_id: Option<Uuid>,
        search_id: Uuid,
        route_id: &str,
        vote: Option<VoteChoice>,
    ) -> ReputationEventStatus {
        let session_started_at = reputation_session_started_at();
        let Some(redis) = self.redis.as_ref() else {
            return ReputationEventStatus {
                session_started_at,
                spam: false,
            };
        };
        let identity =
            format!("{event}:{anonymous_id}:{service_id:?}:{search_id}:{route_id}:{vote:?}");
        let digest = Sha256::digest(identity.as_bytes());
        let key = format!("pay3flow:reputation:event:{digest:x}");
        let snapshot = ReputationEventSnapshot {
            event,
            anonymous_id,
            service_id,
            search_id,
            route_id,
            vote,
            session_started_at,
        };
        let operation = async {
            let mut connection = redis.get().await?;
            let marker = format!("{key}:seen");
            let spam_key = format!(
                "pay3flow:reputation:spam:{}:{}",
                anonymous_id,
                session_started_at.timestamp()
            );
            let counter_key = format!(
                "pay3flow:reputation:session-events:{}:{}",
                anonymous_id,
                session_started_at.timestamp()
            );
            let baseline_key = format!(
                "pay3flow:reputation:session-baseline:{}:{}",
                anonymous_id,
                session_started_at.timestamp()
            );
            let encoded_snapshot = serde_json::to_string(&snapshot)?;
            let script = r#"
local ttl = tonumber(ARGV[4])
redis.call('SET', KEYS[1], ARGV[1], 'EX', ttl)
if redis.call('EXISTS', KEYS[4]) == 1 then return {-1, 1} end
local inserted = redis.call('SET', KEYS[2], '1', 'NX', 'EX', ttl)
if not inserted then return {0, 0} end
local count = redis.call('INCR', KEYS[3])
if count == 1 then
  redis.call('EXPIRE', KEYS[3], ttl)
  local cached = redis.call('GET', KEYS[5]) or '{}'
  local ok, services = pcall(cjson.decode, cached)
  if not ok then services = {} end
  redis.call('SET', KEYS[6], cjson.encode({session_started_at=tonumber(ARGV[3]), services=services}), 'NX', 'EX', ttl)
end
local spam = 0
if count > tonumber(ARGV[2]) then
  redis.call('SET', KEYS[4], '1', 'EX', ttl)
  spam = 1
end
return {count, spam}
"#;
            let (count, spam): (i64, i64) = deadpool_redis::redis::cmd("EVAL")
                .arg(script)
                .arg(6)
                .arg(&key)
                .arg(&marker)
                .arg(&counter_key)
                .arg(&spam_key)
                .arg("pay3flow:reputation:services:v2")
                .arg(&baseline_key)
                .arg(encoded_snapshot)
                .arg(REPUTATION_EVENT_LIMIT_PER_SESSION)
                .arg(session_started_at.timestamp())
                .arg(24 * 60 * 60)
                .query_async(&mut connection)
                .await?;
            if count == REPUTATION_EVENT_LIMIT_PER_SESSION + 1 {
                tracing::warn!(
                    session_start = session_started_at.timestamp(),
                    event_count = count,
                    "service.reputation.session_marked_as_spam"
                );
            }
            Ok::<bool, anyhow::Error>(spam > 0)
        };

        let spam = match tokio::time::timeout(std::time::Duration::from_millis(50), operation).await
        {
            Ok(Ok(spam)) => spam,
            Ok(Err(error)) => {
                tracing::debug!(%error, "service.reputation.event_cache_failed");
                false
            }
            Err(_) => {
                tracing::debug!("service.reputation.event_cache_timed_out");
                false
            }
        };
        ReputationEventStatus {
            session_started_at,
            spam,
        }
    }

    pub async fn start_search(
        &self,
        search_id: Uuid,
        source_currency: &str,
        target_currency: &str,
        anonymous_id: Option<Uuid>,
        count_activity: bool,
    ) -> Result<(), ReputationError> {
        let client = self.pool.get().await?;
        let source_currency = source_currency.trim().to_ascii_uppercase();
        let target_currency = target_currency.trim().to_ascii_uppercase();
        client
            .execute(
                r#"
INSERT INTO route_searches (id, source_currency, target_currency, anonymous_id, counted_for_activity, status, routes_found)
VALUES ($1, $2, $3, $4, $5, 'searching', 0)
ON CONFLICT (id) DO NOTHING
"#,
                &[&search_id, &source_currency, &target_currency, &anonymous_id, &count_activity],
            )
            .await?;
        Ok(())
    }

    pub async fn search_activity(
        &self,
        source_currency: &str,
        target_currency: &str,
        anonymous_id: Option<Uuid>,
        period: SearchActivityPeriod,
    ) -> Result<Vec<RouteSearchActivityHour>, ReputationError> {
        let client = self.pool.get().await?;
        let oldest: Option<DateTime<Utc>> = if matches!(period, SearchActivityPeriod::All) {
            client
                .query_one(
                    "SELECT min(created_at) FROM route_searches WHERE source_currency = $1 AND target_currency = $2 AND counted_for_activity AND status = 'finished'",
                    &[&source_currency, &target_currency],
                )
                .await?
                .get(0)
        } else {
            None
        };
        let (grain, span) = period.aggregation(oldest);
        let rows = client
            .query(
                r#"
WITH bounds AS (
    SELECT date_trunc($4::text, now()) AS end_bucket,
           CASE $4::text
               WHEN 'minute' THEN interval '1 minute'
               WHEN 'hour' THEN interval '1 hour'
               WHEN 'day' THEN interval '1 day'
               WHEN 'week' THEN interval '1 week'
               ELSE interval '1 month'
           END AS bucket_size
), hours AS (
    SELECT generate_series(
        least(date_trunc($4::text, coalesce($6::timestamptz, now() - $5::interval)), end_bucket - bucket_size),
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
    AND ($3::uuid IS NULL OR searches.anonymous_id IS DISTINCT FROM $3)
    AND searches.counted_for_activity
    AND searches.status = 'finished'
GROUP BY hours.started_at
ORDER BY hours.started_at
"#,
                &[&source_currency, &target_currency, &anonymous_id, &grain, &span, &oldest],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| RouteSearchActivityHour {
                started_at: row.get(0),
                count: row.get(1),
            })
            .collect())
    }

    pub async fn count_searches_last_hour(&self) -> Result<i64, ReputationError> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "SELECT COUNT(*)::BIGINT FROM route_searches \
                 WHERE created_at >= now() - INTERVAL '1 hour'",
                &[],
            )
            .await?;
        Ok(row.get(0))
    }

    pub async fn update_search(
        &self,
        search_id: Uuid,
        routes_found: usize,
        status: &str,
    ) -> Result<(), ReputationError> {
        let routes_found = i64::try_from(routes_found).unwrap_or(i64::MAX);
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
UPDATE route_searches
SET routes_found = $2, status = $3, updated_at = now()
WHERE id = $1
"#,
                &[&search_id, &routes_found, &status],
            )
            .await?;
        Ok(())
    }

    pub async fn ensure_services(
        &self,
        services: &[(String, String)],
    ) -> Result<(), ReputationError> {
        if services.is_empty() {
            return Ok(());
        }
        let client = self.pool.get().await?;
        for (slug, display_name) in services {
            client
                .execute(
                    r#"
INSERT INTO services (slug, display_name)
VALUES ($1, $2)
ON CONFLICT (slug) DO UPDATE SET
    display_name = EXCLUDED.display_name,
    updated_at = now()
WHERE services.display_name IS DISTINCT FROM EXCLUDED.display_name
"#,
                    &[slug, display_name],
                )
                .await?;
        }
        Ok(())
    }

    pub async fn stats_for_slugs(
        &self,
        slugs: &[String],
        anonymous_id: Option<Uuid>,
    ) -> Result<HashMap<String, ServiceStats>, ReputationError> {
        if slugs.is_empty() {
            return Ok(HashMap::new());
        }
        let mut canonical_slugs = slugs.to_vec();
        canonical_slugs.sort();
        canonical_slugs.dedup();
        let cache_key = "pay3flow:reputation:services:v2";
        let cached = match self.redis.as_ref() {
            Some(redis) => tokio::time::timeout(
                std::time::Duration::from_millis(50),
                get_json::<HashMap<String, ServiceStats>>(redis, cache_key),
            )
            .await
            .unwrap_or(Ok(None))
            .unwrap_or(None),
            None => None,
        };
        let mut stats = if let Some(cached) = cached {
            cached
        } else {
            let client = self.pool.get().await?;
            let rows = client
                .query(
                    r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, s.reputation_score, NULL::TEXT
FROM services s
"#,
                    &[],
                )
                .await?;
            let stats = rows
                .into_iter()
                .map(row_to_stats)
                .map(|stats| (stats.slug.clone(), stats))
                .collect::<HashMap<_, _>>();
            if let Some(redis) = self.redis.as_ref() {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    set_json(redis, cache_key, &stats, 30),
                )
                .await
                {
                    Ok(Err(error)) => {
                        tracing::debug!(%error, "service.reputation.cache_write_failed")
                    }
                    Err(_) => tracing::debug!("service.reputation.cache_write_timed_out"),
                    Ok(Ok(())) => {}
                }
            }
            stats
        };
        stats.retain(|slug, _| canonical_slugs.binary_search(slug).is_ok());
        if let Some(anonymous_id) = anonymous_id {
            let client = self.pool.get().await?;
            let votes = client
                .query(
                    r#"
SELECT s.slug, v.vote
FROM services s
JOIN service_votes v ON v.service_id = s.id
WHERE s.slug = ANY($1) AND v.anonymous_id = $2
"#,
                    &[&canonical_slugs, &anonymous_id],
                )
                .await?;
            for row in votes {
                if let Some(service) = stats.get_mut(row.get::<_, String>(0).as_str()) {
                    let vote = row.get::<_, String>(1);
                    service.viewer_vote = VoteChoice::parse(&vote);
                }
            }
        }
        Ok(stats)
    }

    pub async fn feedback_for_routes(
        &self,
        route_ids: &[String],
        anonymous_id: Option<Uuid>,
    ) -> Result<HashMap<String, RouteFeedback>, ReputationError> {
        if route_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let client = self.pool.get().await?;
        let rows = client
            .query(
                r#"
SELECT requested.route_id,
       COUNT(votes.*) FILTER (WHERE votes.vote = 'like')::BIGINT,
       COUNT(votes.*) FILTER (WHERE votes.vote = 'dislike')::BIGINT,
       viewer.vote
FROM unnest($1::TEXT[]) AS requested(route_id)
LEFT JOIN route_votes votes ON votes.route_id = requested.route_id
LEFT JOIN route_votes viewer
  ON viewer.route_id = requested.route_id AND viewer.anonymous_id = $2
GROUP BY requested.route_id, viewer.vote
"#,
                &[&route_ids, &anonymous_id],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let route_id = row.get::<_, String>(0);
                (route_id, route_feedback_from_row(row, 1))
            })
            .collect())
    }

    pub fn tracking_token(
        &self,
        service_id: Uuid,
        search_id: Uuid,
        route_id: &str,
        destination_url: &str,
    ) -> Result<String, ReputationError> {
        if !valid_destination(destination_url) {
            return Err(ReputationError::InvalidTrackingToken);
        }
        let claims = TrackingClaims {
            service_id,
            search_id,
            route_id: route_id.to_string(),
            destination_url: destination_url.to_string(),
            exp: now_secs().saturating_add(TRACKING_TOKEN_TTL_SECS),
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encode_key)
            .map_err(|_| ReputationError::InvalidTrackingToken)
    }

    pub async fn record_execution(
        &self,
        token: &str,
        anonymous_id: Uuid,
    ) -> Result<ExecutionOpen, ReputationError> {
        let claims =
            decode::<TrackingClaims>(token, &self.decode_key, &Validation::new(Algorithm::HS256))
                .map_err(|_| ReputationError::InvalidTrackingToken)?
                .claims;

        let event_status = self
            .cache_event_first(
                "provider_link_open",
                anonymous_id,
                Some(claims.service_id),
                claims.search_id,
                &claims.route_id,
                None,
            )
            .await;
        if event_status.spam {
            self.rollback_spam_session(anonymous_id, event_status.session_started_at)
                .await?;
            let client = self.pool.get().await?;
            let row = client
                .query_opt(
                    r#"SELECT s.id, s.slug, s.display_name, s.executions_total,
                              s.likes_total, s.dislikes_total, s.reputation_score, v.vote
                       FROM services s
                       LEFT JOIN service_votes v
                         ON v.service_id = s.id AND v.anonymous_id = $2
                       WHERE s.id = $1"#,
                    &[&claims.service_id, &anonymous_id],
                )
                .await?
                .ok_or(ReputationError::ServiceNotFound)?;
            return Ok(ExecutionOpen {
                execution_id: Uuid::new_v4(),
                newly_recorded: false,
                redirect_url: claims.destination_url,
                service: row_to_stats(row),
            });
        }

        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        if Self::lock_reputation_session(
            &transaction,
            anonymous_id,
            event_status.session_started_at,
        )
        .await?
        {
            let row = transaction
                .query_opt(
                    r#"SELECT s.id, s.slug, s.display_name, s.executions_total,
                              s.likes_total, s.dislikes_total, s.reputation_score, v.vote
                       FROM services s
                       LEFT JOIN service_votes v
                         ON v.service_id = s.id AND v.anonymous_id = $2
                       WHERE s.id = $1"#,
                    &[&claims.service_id, &anonymous_id],
                )
                .await?
                .ok_or(ReputationError::ServiceNotFound)?;
            let service = row_to_stats(row);
            transaction.commit().await?;
            return Ok(ExecutionOpen {
                execution_id: Uuid::new_v4(),
                newly_recorded: false,
                redirect_url: claims.destination_url,
                service,
            });
        }
        let reputation_before = transaction
            .query_opt(
                "SELECT reputation_score FROM services WHERE id = $1 FOR UPDATE",
                &[&claims.service_id],
            )
            .await?
            .map(|row| row.get::<_, i16>(0))
            .ok_or(ReputationError::ServiceNotFound)?;
        let inserted = transaction
            .query_opt(
                r#"
INSERT INTO service_executions
    (service_id, search_id, route_id, anonymous_id, status)
VALUES ($1, $2, $3, $4, 'started')
ON CONFLICT (anonymous_id, service_id, search_id, route_id) DO NOTHING
RETURNING id
"#,
                &[
                    &claims.service_id,
                    &claims.search_id,
                    &claims.route_id,
                    &anonymous_id,
                ],
            )
            .await?;
        let (execution_id, newly_recorded) = match inserted {
            Some(row) => {
                let score_row = transaction
                    .query_one(
                        r#"
UPDATE services
SET executions_total = executions_total + 1,
    reputation_score = LEAST(100, reputation_score + 5),
    reputation_last_interaction_at = now(),
    reputation_last_decay_at = now(),
    updated_at = now()
WHERE id = $1
RETURNING reputation_score
"#,
                        &[&claims.service_id],
                    )
                    .await?;
                let score_delta = score_row.get::<_, i16>(0) - reputation_before;
                transaction
                    .execute(
                        "UPDATE service_executions SET reputation_score_delta = $2 WHERE id = $1",
                        &[&row.get::<_, Uuid>(0), &score_delta],
                    )
                    .await?;
                (row.get(0), true)
            }
            None => {
                let row = transaction
                    .query_opt(
                        r#"
SELECT id FROM service_executions
WHERE anonymous_id = $1 AND service_id = $2 AND search_id = $3 AND route_id = $4
"#,
                        &[
                            &anonymous_id,
                            &claims.service_id,
                            &claims.search_id,
                            &claims.route_id,
                        ],
                    )
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("execution conflict winner was not found"))?;
                (row.get(0), false)
            }
        };
        let row = transaction
            .query_opt(
                r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, s.reputation_score, v.vote
FROM services s
LEFT JOIN service_votes v
  ON v.service_id = s.id AND v.anonymous_id = $2
WHERE s.id = $1
"#,
                &[&claims.service_id, &anonymous_id],
            )
            .await?
            .ok_or(ReputationError::ServiceNotFound)?;
        let service = row_to_stats(row);
        transaction.commit().await?;

        Ok(ExecutionOpen {
            execution_id,
            newly_recorded,
            redirect_url: claims.destination_url,
            service,
        })
    }

    pub async fn set_vote(
        &self,
        service_id: Uuid,
        anonymous_id: Uuid,
        vote: VoteChoice,
    ) -> Result<ServiceStats, ReputationError> {
        let event_status = self
            .cache_event_first(
                "service_vote",
                anonymous_id,
                Some(service_id),
                Uuid::nil(),
                "",
                Some(vote),
            )
            .await;
        if event_status.spam {
            self.rollback_spam_session(anonymous_id, event_status.session_started_at)
                .await?;
            let client = self.pool.get().await?;
            let row = client
                .query_opt(
                    r#"SELECT s.id, s.slug, s.display_name, s.executions_total,
                              s.likes_total, s.dislikes_total, s.reputation_score, v.vote
                       FROM services s
                       LEFT JOIN service_votes v
                         ON v.service_id = s.id AND v.anonymous_id = $2
                       WHERE s.id = $1"#,
                    &[&service_id, &anonymous_id],
                )
                .await?
                .ok_or(ReputationError::ServiceNotFound)?;
            return Ok(row_to_stats(row));
        }
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        if Self::lock_reputation_session(
            &transaction,
            anonymous_id,
            event_status.session_started_at,
        )
        .await?
        {
            let row = transaction
                .query_opt(
                    r#"SELECT s.id, s.slug, s.display_name, s.executions_total,
                              s.likes_total, s.dislikes_total, s.reputation_score, v.vote
                       FROM services s
                       LEFT JOIN service_votes v
                         ON v.service_id = s.id AND v.anonymous_id = $2
                       WHERE s.id = $1"#,
                    &[&service_id, &anonymous_id],
                )
                .await?
                .ok_or(ReputationError::ServiceNotFound)?;
            let stats = row_to_stats(row);
            transaction.commit().await?;
            return Ok(stats);
        }
        let service_score_before = transaction
            .query_opt(
                "SELECT reputation_score FROM services WHERE id = $1 FOR UPDATE",
                &[&service_id],
            )
            .await?
            .map(|row| row.get::<_, i16>(0))
            .ok_or(ReputationError::ServiceNotFound)?;
        let previous = transaction
            .query_opt(
                r#"
SELECT vote FROM service_votes
WHERE service_id = $1 AND anonymous_id = $2
FOR UPDATE
"#,
                &[&service_id, &anonymous_id],
            )
            .await?
            .and_then(|row| VoteChoice::parse(row.get::<_, String>(0).as_str()));

        transaction
            .execute(
                r#"INSERT INTO service_vote_session_baselines
                       (anonymous_id, session_started_at, service_id, previous_vote)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT DO NOTHING"#,
                &[
                    &anonymous_id,
                    &event_status.session_started_at,
                    &service_id,
                    &previous.map(VoteChoice::as_str),
                ],
            )
            .await?;

        transaction
            .execute(
                r#"
INSERT INTO service_votes (anonymous_id, service_id, vote)
VALUES ($1, $2, $3)
ON CONFLICT (anonymous_id, service_id) DO UPDATE SET
    vote = EXCLUDED.vote,
    updated_at = now()
"#,
                &[&anonymous_id, &service_id, &vote.as_str()],
            )
            .await?;

        let (likes_delta, dislikes_delta) = vote_deltas(previous, vote);
        let score_row = transaction
            .query_one(
                r#"
UPDATE services
SET likes_total = likes_total + $2,
    dislikes_total = dislikes_total + $3,
    reputation_score = GREATEST(0, LEAST(100, reputation_score + $4)),
    reputation_last_interaction_at = CASE WHEN $5 THEN now() ELSE reputation_last_interaction_at END,
    reputation_last_decay_at = CASE WHEN $5 THEN now() ELSE reputation_last_decay_at END,
    updated_at = now()
WHERE id = $1
RETURNING reputation_score
"#,
                &[
                    &service_id,
                    &likes_delta,
                    &dislikes_delta,
                    &vote_score_delta(previous, vote),
                    &(previous != Some(vote)),
                ],
            )
            .await?;
        let score_delta = score_row.get::<_, i16>(0) - service_score_before;
        transaction
            .execute(
                r#"UPDATE service_vote_session_baselines
                   SET reputation_score_delta = reputation_score_delta + $4
                   WHERE anonymous_id = $1 AND session_started_at = $2 AND service_id = $3"#,
                &[
                    &anonymous_id,
                    &event_status.session_started_at,
                    &service_id,
                    &score_delta,
                ],
            )
            .await?;
        let row = transaction
            .query_one(
                r#"
SELECT id, slug, display_name, executions_total, likes_total, dislikes_total,
       reputation_score, $2::TEXT
FROM services WHERE id = $1
"#,
                &[&service_id, &vote.as_str()],
            )
            .await?;
        let stats = row_to_stats(row);
        transaction.commit().await?;
        Ok(stats)
    }

    pub async fn record_instruction_open(
        &self,
        tokens: &[String],
        anonymous_id: Uuid,
    ) -> Result<InstructionOpen, ReputationError> {
        let mut claims = Vec::with_capacity(tokens.len());
        for token in tokens {
            claims.push(
                decode::<TrackingClaims>(
                    token,
                    &self.decode_key,
                    &Validation::new(Algorithm::HS256),
                )
                .map_err(|_| ReputationError::InvalidTrackingToken)?
                .claims,
            );
        }
        let Some(first) = claims.first() else {
            return Ok(InstructionOpen { newly_recorded: 0 });
        };
        if claims
            .iter()
            .any(|claim| claim.search_id != first.search_id || claim.route_id != first.route_id)
        {
            return Err(ReputationError::InvalidTrackingToken);
        }

        let mut event_status = None;
        for claim in &claims {
            let status = self
                .cache_event_first(
                    "instruction_open",
                    anonymous_id,
                    Some(claim.service_id),
                    claim.search_id,
                    &claim.route_id,
                    None,
                )
                .await;
            event_status = Some(status);
            if status.spam {
                self.rollback_spam_session(anonymous_id, status.session_started_at)
                    .await?;
                return Ok(InstructionOpen { newly_recorded: 0 });
            }
        }

        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        if let Some(status) = event_status {
            if Self::lock_reputation_session(&transaction, anonymous_id, status.session_started_at)
                .await?
            {
                transaction.commit().await?;
                return Ok(InstructionOpen { newly_recorded: 0 });
            }
        }
        let mut newly_recorded = 0;
        let mut service_ids = claims
            .iter()
            .map(|claim| claim.service_id)
            .collect::<Vec<_>>();
        service_ids.sort_unstable();
        service_ids.dedup();
        for service_id in service_ids {
            let reputation_before = transaction
                .query_opt(
                    "SELECT reputation_score FROM services WHERE id = $1 FOR UPDATE",
                    &[&service_id],
                )
                .await?
                .map(|row| row.get::<_, i16>(0))
                .ok_or(ReputationError::ServiceNotFound)?;
            let inserted = transaction
                .execute(
                    r#"
INSERT INTO service_instruction_opens (anonymous_id, service_id, search_id, route_id)
VALUES ($1, $2, $3, $4)
ON CONFLICT DO NOTHING
"#,
                    &[
                        &anonymous_id,
                        &service_id,
                        &first.search_id,
                        &first.route_id,
                    ],
                )
                .await?;
            if inserted > 0 {
                let score_row = transaction
                    .query_one(
                        "UPDATE services SET reputation_score = LEAST(100, reputation_score + 5), reputation_last_interaction_at = now(), reputation_last_decay_at = now(), updated_at = now() WHERE id = $1 RETURNING reputation_score",
                        &[&service_id],
                    )
                    .await?;
                let score_delta = score_row.get::<_, i16>(0) - reputation_before;
                transaction
                    .execute(
                        r#"UPDATE service_instruction_opens
                           SET reputation_score_delta = $5
                           WHERE anonymous_id = $1 AND service_id = $2 AND search_id = $3 AND route_id = $4"#,
                        &[
                            &anonymous_id,
                            &service_id,
                            &first.search_id,
                            &first.route_id,
                            &score_delta,
                        ],
                    )
                    .await?;
                newly_recorded += 1;
            }
        }
        transaction.commit().await?;
        Ok(InstructionOpen { newly_recorded })
    }

    pub async fn set_route_vote(
        &self,
        route_id: &str,
        anonymous_id: Uuid,
        vote: VoteChoice,
    ) -> Result<RouteFeedback, ReputationError> {
        let event_status = self
            .cache_event_first(
                "route_vote",
                anonymous_id,
                None,
                Uuid::nil(),
                route_id,
                Some(vote),
            )
            .await;
        if event_status.spam {
            self.rollback_spam_session(anonymous_id, event_status.session_started_at)
                .await?;
            let client = self.pool.get().await?;
            let row = client
                .query_opt(
                    r#"SELECT COUNT(*) FILTER (WHERE vote = 'like')::BIGINT,
                              COUNT(*) FILTER (WHERE vote = 'dislike')::BIGINT,
                              (SELECT vote::TEXT FROM route_votes
                               WHERE route_id = $1 AND anonymous_id = $2)
                       FROM route_votes WHERE route_id = $1"#,
                    &[&route_id, &anonymous_id],
                )
                .await?;
            return Ok(route_feedback_from_row(
                row.ok_or_else(|| anyhow::anyhow!("route vote state was not found"))?,
                0,
            ));
        }
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        if Self::lock_reputation_session(
            &transaction,
            anonymous_id,
            event_status.session_started_at,
        )
        .await?
        {
            let row = transaction
                .query_one(
                    r#"SELECT COUNT(*) FILTER (WHERE vote = 'like')::BIGINT,
                              COUNT(*) FILTER (WHERE vote = 'dislike')::BIGINT,
                              (SELECT vote::TEXT FROM route_votes
                               WHERE route_id = $1 AND anonymous_id = $2)
                       FROM route_votes WHERE route_id = $1"#,
                    &[&route_id, &anonymous_id],
                )
                .await?;
            let feedback = route_feedback_from_row(row, 0);
            transaction.commit().await?;
            return Ok(feedback);
        }
        let previous = transaction
            .query_opt(
                "SELECT vote FROM route_votes WHERE route_id = $1 AND anonymous_id = $2 FOR UPDATE",
                &[&route_id, &anonymous_id],
            )
            .await?
            .and_then(|row| VoteChoice::parse(row.get::<_, String>(0).as_str()));
        transaction
            .execute(
                r#"INSERT INTO route_vote_session_baselines
                       (anonymous_id, session_started_at, route_id, previous_vote)
                   VALUES ($1, $2, $3, $4)
                   ON CONFLICT DO NOTHING"#,
                &[
                    &anonymous_id,
                    &event_status.session_started_at,
                    &route_id,
                    &previous.map(VoteChoice::as_str),
                ],
            )
            .await?;
        transaction
            .execute(
                r#"
INSERT INTO route_votes (anonymous_id, route_id, vote)
VALUES ($1, $2, $3)
ON CONFLICT (anonymous_id, route_id) DO UPDATE SET
    vote = EXCLUDED.vote,
    updated_at = now()
"#,
                &[&anonymous_id, &route_id, &vote.as_str()],
            )
            .await?;
        let row = transaction
            .query_one(
                r#"
SELECT COUNT(*) FILTER (WHERE vote = 'like')::BIGINT,
       COUNT(*) FILTER (WHERE vote = 'dislike')::BIGINT,
       (SELECT vote::TEXT FROM route_votes
        WHERE route_id = $1 AND anonymous_id = $2)
FROM route_votes
WHERE route_id = $1
"#,
                &[&route_id, &anonymous_id],
            )
            .await?;
        let feedback = route_feedback_from_row(row, 0);
        transaction.commit().await?;
        Ok(feedback)
    }
}

pub fn average_reputation(services: &[RouteServiceStats]) -> CombinedReputation {
    if services.is_empty() {
        return CombinedReputation::default();
    }
    let count = i64::try_from(services.len()).unwrap_or(i64::MAX);
    let rounded_average = |sum: i64| (sum.saturating_add(count / 2)) / count;
    let total = |select: fn(&RouteServiceStats) -> i64| {
        services.iter().map(select).fold(0_i64, i64::saturating_add)
    };
    CombinedReputation {
        executions_average: rounded_average(total(|service| service.stats.executions_total)),
        likes_average: rounded_average(total(|service| service.stats.likes_total)),
        dislikes_average: rounded_average(total(|service| service.stats.dislikes_total)),
    }
}

/// Conservative vote quality used only as a tie-breaker for economically
/// similar routes. `None` keeps low-volume results in their economic order.
pub fn vote_quality_score(likes: i64, dislikes: i64) -> Option<f64> {
    let likes = likes.max(0) as f64;
    let dislikes = dislikes.max(0) as f64;
    let votes = likes + dislikes;
    if votes < 10.0 {
        return None;
    }
    let positive = likes / votes;
    let z = 2.0;
    let denominator = 1.0 + z * z / votes;
    let center = positive + z * z / (2.0 * votes);
    let margin = z * ((positive * (1.0 - positive) / votes + z * z / (4.0 * votes * votes)).sqrt());
    Some((center - margin) / denominator)
}

fn valid_destination(destination_url: &str) -> bool {
    reqwest::Url::parse(destination_url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

fn vote_deltas(previous: Option<VoteChoice>, next: VoteChoice) -> (i64, i64) {
    let contribution = |vote| match vote {
        Some(VoteChoice::Like) => (1, 0),
        Some(VoteChoice::Dislike) => (0, 1),
        None => (0, 0),
    };
    let (old_like, old_dislike) = contribution(previous);
    let (new_like, new_dislike) = contribution(Some(next));
    (new_like - old_like, new_dislike - old_dislike)
}

fn vote_score_delta(previous: Option<VoteChoice>, next: VoteChoice) -> i16 {
    let score = |vote| match vote {
        VoteChoice::Like => 10_i16,
        VoteChoice::Dislike => -15_i16,
    };
    score(next) - previous.map_or(0, score)
}

fn vote_state_deltas(current: Option<VoteChoice>, desired: Option<VoteChoice>) -> (i64, i64, i16) {
    let vote_value = |vote: Option<VoteChoice>| match vote {
        Some(VoteChoice::Like) => (1_i64, 0_i64, 10_i16),
        Some(VoteChoice::Dislike) => (0, 1, -15),
        None => (0, 0, 0),
    };
    let (current_likes, current_dislikes, current_score) = vote_value(current);
    let (desired_likes, desired_dislikes, desired_score) = vote_value(desired);
    (
        desired_likes - current_likes,
        desired_dislikes - current_dislikes,
        desired_score - current_score,
    )
}

fn row_to_stats(row: tokio_postgres::Row) -> ServiceStats {
    ServiceStats {
        id: row.get(0),
        slug: row.get(1),
        display_name: row.get(2),
        executions_total: row.get(3),
        likes_total: row.get(4),
        dislikes_total: row.get(5),
        reputation_score: row.get(6),
        viewer_vote: row
            .get::<_, Option<String>>(7)
            .as_deref()
            .and_then(VoteChoice::parse),
    }
}

fn default_reputation_score() -> i16 {
    50
}

fn route_feedback_from_row(row: tokio_postgres::Row, offset: usize) -> RouteFeedback {
    RouteFeedback {
        likes_total: row.get(offset),
        dislikes_total: row.get(offset + 1),
        viewer_vote: row
            .get::<_, Option<String>>(offset + 2)
            .as_deref()
            .and_then(VoteChoice::parse),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn reputation_session_started_at() -> DateTime<Utc> {
    let now = Utc::now();
    let bucket =
        now.timestamp().div_euclid(REPUTATION_SESSION_SECS as i64) * REPUTATION_SESSION_SECS as i64;
    DateTime::from_timestamp(bucket, 0).unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vote_transition_deltas_cover_every_state_change() {
        let cases = [
            (None, VoteChoice::Like, (1, 0)),
            (None, VoteChoice::Dislike, (0, 1)),
            (Some(VoteChoice::Like), VoteChoice::Dislike, (-1, 1)),
            (Some(VoteChoice::Dislike), VoteChoice::Like, (1, -1)),
            (Some(VoteChoice::Like), VoteChoice::Like, (0, 0)),
            (Some(VoteChoice::Dislike), VoteChoice::Dislike, (0, 0)),
        ];
        for (previous, next, expected) in cases {
            assert_eq!(
                vote_deltas(previous, next),
                expected,
                "{previous:?} -> {next:?}"
            );
        }
    }

    #[test]
    fn vote_score_transitions_apply_only_the_change_in_choice() {
        assert_eq!(vote_score_delta(None, VoteChoice::Like), 10);
        assert_eq!(vote_score_delta(None, VoteChoice::Dislike), -15);
        assert_eq!(
            vote_score_delta(Some(VoteChoice::Like), VoteChoice::Dislike),
            -25
        );
        assert_eq!(
            vote_score_delta(Some(VoteChoice::Dislike), VoteChoice::Like),
            25
        );
        assert_eq!(
            vote_score_delta(Some(VoteChoice::Like), VoteChoice::Like),
            0
        );
    }

    #[test]
    fn spam_rollback_restores_only_the_session_vote_state() {
        assert_eq!(
            vote_state_deltas(Some(VoteChoice::Like), None),
            (-1, 0, -10)
        );
        assert_eq!(
            vote_state_deltas(Some(VoteChoice::Like), Some(VoteChoice::Dislike)),
            (-1, 1, -25)
        );
        assert_eq!(
            vote_state_deltas(Some(VoteChoice::Dislike), Some(VoteChoice::Dislike)),
            (0, 0, 0)
        );
    }

    #[test]
    fn combined_reputation_averages_distinct_services() {
        let service = |executions, likes, dislikes| RouteServiceStats {
            stats: ServiceStats {
                id: Uuid::new_v4(),
                slug: "service".into(),
                display_name: "Service".into(),
                executions_total: executions,
                likes_total: likes,
                dislikes_total: dislikes,
                reputation_score: 50,
                viewer_vote: None,
            },
        };
        let reputation = average_reputation(&[service(10, 9, 1), service(5, 4, 0)]);
        assert_eq!(reputation.executions_average, 8);
        assert_eq!(reputation.likes_average, 7);
        assert_eq!(reputation.dislikes_average, 1);
    }

    #[test]
    fn tracked_destinations_must_be_absolute_http_urls() {
        for valid in ["https://example.com/offer/1", "http://localhost:8080/trade"] {
            assert!(
                valid_destination(valid),
                "expected valid destination: {valid}"
            );
        }
        for invalid in ["javascript:alert(1)", "/relative", "not a url"] {
            assert!(
                !valid_destination(invalid),
                "expected invalid destination: {invalid}"
            );
        }
    }

    #[test]
    fn vote_quality_uses_both_volume_and_balance() {
        let strong = vote_quality_score(100, 4).unwrap();
        let smaller_clean = vote_quality_score(16, 0).unwrap();
        let contested = vote_quality_score(100, 68).unwrap();
        assert!(strong > smaller_clean);
        assert!(smaller_clean > contested);
        assert!(vote_quality_score(5, 0).is_none());
    }
}

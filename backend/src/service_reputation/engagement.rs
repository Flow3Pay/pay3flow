//! Anonymous, write-behind provider interaction counts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use deadpool_redis::redis::AsyncCommands;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time;
use uuid::Uuid;

use crate::core::redis::{self, RedisPool};
use crate::db::DbPool;

const SESSION_SECS: u64 = 30 * 60;
const PENDING_SECS: u64 = 7 * 24 * 60 * 60;
const SESSION_SPAM_THRESHOLD: u64 = 12;
const PENDING_SET: &str = "p2p:engagement:pending";
const LIVE_KEY: &str = "p2p:engagement:live";
const LAST_KEY: &str = "p2p:engagement:last";
const SCORE_CACHE_KEY: &str = "p2p:reputation:snapshot:v1";
const REDIS_TIMEOUT: Duration = Duration::from_millis(250);

const RECORD_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1] .. ':spam') == 1 then
  return 0
end
if not redis.call('SET', KEYS[4], '1', 'NX', 'EX', ARGV[1]) then
  return 0
end
local count = redis.call('LLEN', KEYS[1])
if count >= tonumber(ARGV[3]) then
  local prior = redis.call('LRANGE', KEYS[1], 1, -1)
  for _, raw in ipairs(prior) do
    local parts = {}
    for part in string.gmatch(raw, '[^|]+') do table.insert(parts, part) end
    for index = 2, #parts do
      local field = parts[index]
      redis.call('HINCRBY', parts[1], field, -1)
      redis.call('HINCRBY', KEYS[5], field, -1)
    end
  end
  redis.call('LTRIM', KEYS[1], 0, 0)
  redis.call('SET', KEYS[1] .. ':spam', '1', 'EX', math.max(redis.call('TTL', KEYS[1]), 1))
  return -1
end
local fields = {}
for index = 5, #ARGV do
  local field = ARGV[index]
  redis.call('HINCRBY', KEYS[2], field, 1)
  redis.call('HINCRBY', KEYS[5], field, 1)
  local separator = string.find(field, ':')
  if separator then redis.call('HSET', KEYS[6], string.sub(field, separator + 1), ARGV[4]) end
  table.insert(fields, field)
end
redis.call('RPUSH', KEYS[1], KEYS[2] .. '|' .. table.concat(fields, '|'))
if count == 0 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
redis.call('EXPIRE', KEYS[2], ARGV[2])
redis.call('SADD', KEYS[3], KEYS[2])
return 1
"#;

const CLEAR_SCRIPT: &str = r#"
local values = redis.call('HGETALL', KEYS[1])
for index = 1, #values, 2 do
  local amount = tonumber(values[index + 1])
  if amount and amount > 0 then
    redis.call('HINCRBY', KEYS[3], values[index], -amount)
  end
end
redis.call('DEL', KEYS[1])
redis.call('SREM', KEYS[2], KEYS[1])
return 1
"#;

#[derive(Clone, Copy)]
pub(crate) enum EngagementKind {
    Instruction,
    Link,
}

impl EngagementKind {
    fn field(self) -> &'static str {
        match self {
            Self::Instruction => "instruction",
            Self::Link => "link",
        }
    }
}

pub(crate) struct EngagementMetrics {
    pool: DbPool,
    redis: RedisPool,
    secret: Vec<u8>,
    scores: Arc<RwLock<HashMap<String, f64>>>,
    reputations: Arc<RwLock<HashMap<String, u8>>>,
}

#[derive(Clone, Deserialize, Serialize)]
struct ScoreSnapshot {
    calculated_at: u64,
    ranking: HashMap<String, f64>,
    reputations: HashMap<String, u8>,
}

impl EngagementMetrics {
    pub(crate) fn new(pool: DbPool, redis: RedisPool, secret: &[u8]) -> Arc<Self> {
        let metrics = Arc::new(Self {
            pool,
            redis,
            secret: secret.to_vec(),
            scores: Arc::new(RwLock::new(HashMap::new())),
            reputations: Arc::new(RwLock::new(HashMap::new())),
        });
        let worker = metrics.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(error) = worker.flush_pending().await {
                    tracing::warn!(%error, "provider engagement flush failed");
                }
                if let Err(error) = worker.refresh_scores().await {
                    tracing::warn!(%error, "provider engagement score refresh failed");
                }
            }
        });
        metrics
    }

    pub(crate) fn scores(&self) -> Arc<RwLock<HashMap<String, f64>>> {
        self.scores.clone()
    }

    pub(crate) fn reputations(&self) -> Arc<RwLock<HashMap<String, u8>>> {
        self.reputations.clone()
    }

    pub(crate) fn record(
        self: &Arc<Self>,
        anonymous_id: Uuid,
        kind: EngagementKind,
        event_id: String,
        slugs: Vec<String>,
    ) {
        let metrics = self.clone();
        tokio::spawn(async move {
            if let Err(error) = metrics
                .record_inner(anonymous_id, kind, event_id, slugs)
                .await
            {
                tracing::warn!(%error, "provider engagement record failed");
            }
        });
    }

    async fn record_inner(
        &self,
        anonymous_id: Uuid,
        kind: EngagementKind,
        event_id: String,
        mut slugs: Vec<String>,
    ) -> Result<()> {
        slugs.sort();
        slugs.dedup();
        slugs.retain(|slug| {
            !slug.is_empty()
                && slug.len() <= 64
                && slug
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        });
        if slugs.is_empty() {
            return Ok(());
        }
        let mut hash = Sha256::new();
        hash.update(&self.secret);
        hash.update(anonymous_id.as_bytes());
        hash.update(kind.field().as_bytes());
        let session_hash = hash.finalize();
        let session_key = format!("p2p:engagement:session:{session_hash:x}");
        let mut seen_hash = Sha256::new();
        seen_hash.update(&self.secret);
        seen_hash.update(anonymous_id.as_bytes());
        seen_hash.update(kind.field().as_bytes());
        seen_hash.update(event_id.as_bytes());
        let seen_key = format!("p2p:engagement:seen:{:x}", seen_hash.finalize());
        let minute = super::now_secs() / 60;
        let bucket_key = format!("p2p:engagement:bucket:{minute}");
        let mut connection = self.redis.get().await?;
        let mut command = deadpool_redis::redis::cmd("EVAL");
        command
            .arg(RECORD_SCRIPT)
            .arg(6)
            .arg(session_key)
            .arg(bucket_key)
            .arg(PENDING_SET)
            .arg(seen_key)
            .arg(LIVE_KEY)
            .arg(LAST_KEY)
            .arg(SESSION_SECS)
            .arg(PENDING_SECS)
            .arg(SESSION_SPAM_THRESHOLD)
            .arg(super::now_secs());
        for slug in slugs {
            command.arg(format!("{}:{}", kind.field(), slug.to_ascii_lowercase()));
        }
        let _: i64 = time::timeout(REDIS_TIMEOUT, command.query_async(&mut connection))
            .await
            .context("engagement Redis timeout")??;
        Ok(())
    }

    async fn flush_pending(&self) -> Result<()> {
        let mut connection = self.redis.get().await?;
        let keys: Vec<String> = connection.smembers(PENDING_SET).await?;
        drop(connection);
        let current_minute = super::now_secs() / 60;
        for key in keys {
            let minute = key
                .rsplit(':')
                .next()
                .and_then(|part| part.parse::<u64>().ok());
            // A session may still be rolled back during its 30-minute window.
            if !minute.is_some_and(|minute| minute + 32 < current_minute) {
                continue;
            }
            if !redis::try_acquire_lock(&self.redis, &format!("{key}:flush"), 120).await? {
                continue;
            }
            if let Err(error) = self.flush_bucket(&key).await {
                tracing::warn!(%error, bucket = %key, "provider engagement bucket flush failed");
            }
        }
        Ok(())
    }

    async fn flush_bucket(&self, key: &str) -> Result<()> {
        let mut connection = self.redis.get().await?;
        let counts: HashMap<String, i64> = connection.hgetall(key).await?;
        for (field, amount) in counts {
            let Some((kind, slug)) = field.split_once(':') else {
                continue;
            };
            if amount <= 0 || !matches!(kind, "instruction" | "link") {
                continue;
            }
            let batch_key = format!("{key}:{field}");
            let mut client = self.pool.get().await?;
            let transaction = client.transaction().await?;
            let inserted = transaction.query_opt(
                "INSERT INTO provider_engagement_flushes (batch_key) VALUES ($1) ON CONFLICT DO NOTHING RETURNING batch_key",
                &[&batch_key],
            ).await?;
            if inserted.is_some() {
                let instructions = if kind == "instruction" { amount } else { 0 };
                let links = if kind == "link" { amount } else { 0 };
                transaction.execute(
                    "INSERT INTO provider_engagement (slug, instruction_opens, link_opens) VALUES ($1, $2, $3) ON CONFLICT (slug) DO UPDATE SET instruction_opens = provider_engagement.instruction_opens + EXCLUDED.instruction_opens, link_opens = provider_engagement.link_opens + EXCLUDED.link_opens, updated_at = now()",
                    &[&slug, &instructions, &links],
                ).await?;
                if links > 0 {
                    transaction.execute(
                        "UPDATE services SET executions_total = executions_total + $2, updated_at = now() WHERE slug = $1",
                        &[&slug, &links],
                    ).await?;
                }
            }
            transaction.commit().await?;
        }
        let _: i64 = deadpool_redis::redis::cmd("EVAL")
            .arg(CLEAR_SCRIPT)
            .arg(3)
            .arg(key)
            .arg(PENDING_SET)
            .arg(LIVE_KEY)
            .query_async(&mut connection)
            .await?;
        Ok(())
    }

    async fn refresh_scores(&self) -> Result<()> {
        if let Ok(Ok(Some(snapshot))) = time::timeout(
            REDIS_TIMEOUT,
            redis::get_json::<ScoreSnapshot>(&self.redis, SCORE_CACHE_KEY),
        )
        .await
        {
            *self.scores.write() = snapshot.ranking.clone();
            *self.reputations.write() = snapshot.reputations.clone();
            if super::now_secs().saturating_sub(snapshot.calculated_at) < 55 {
                return Ok(());
            }
        }
        if !redis::try_acquire_lock(&self.redis, "p2p:reputation:refresh", 55).await? {
            return Ok(());
        }
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT s.slug, s.likes_total, s.dislikes_total, s.executions_total, COALESCE(e.instruction_opens, 0), COALESCE(e.link_opens, 0), GREATEST(s.updated_at, COALESCE(e.updated_at, s.updated_at)) FROM services s LEFT JOIN provider_engagement e ON e.slug = s.slug",
            &[],
        ).await?;
        let mut counts = rows
            .into_iter()
            .map(|row| {
                let slug: String = row.get(0);
                (
                    slug,
                    (
                        [row.get(1), row.get(2), row.get(4), row.get(5)],
                        row.get::<_, chrono::DateTime<chrono::Utc>>(6).timestamp(),
                    ),
                )
            })
            .collect::<HashMap<String, ([i64; 4], i64)>>();
        let mut connection = self.redis.get().await?;
        let live: HashMap<String, i64> = connection.hgetall(LIVE_KEY).await?;
        let last: HashMap<String, i64> = connection.hgetall(LAST_KEY).await?;
        for (field, amount) in live {
            if let Some((kind, slug)) = field.split_once(':') {
                if let Some((values, _)) = counts.get_mut(slug) {
                    match kind {
                        "instruction" => values[2] += amount,
                        "link" => values[3] += amount,
                        _ => {}
                    }
                }
            }
        }
        let now = super::now_secs() as i64;
        let mut ranking = HashMap::new();
        let mut reputations = HashMap::new();
        for (slug, ([likes, dislikes, instructions, links], db_last)) in counts {
            ranking.insert(slug.clone(), vote_priority(likes, dislikes));
            let last_action = last.get(&slug).copied().unwrap_or_default().max(db_last);
            reputations.insert(
                slug,
                reputation_score(likes, dislikes, instructions, links, now, last_action),
            );
        }
        let snapshot = ScoreSnapshot {
            calculated_at: super::now_secs(),
            ranking,
            reputations,
        };
        *self.scores.write() = snapshot.ranking.clone();
        *self.reputations.write() = snapshot.reputations.clone();
        time::timeout(
            REDIS_TIMEOUT,
            redis::set_json(&self.redis, SCORE_CACHE_KEY, &snapshot, 180),
        )
        .await
        .context("reputation Redis snapshot timeout")??;
        Ok(())
    }
}

fn reputation_score(
    likes: i64,
    dislikes: i64,
    instructions: i64,
    links: i64,
    now: i64,
    last_action: i64,
) -> u8 {
    let idle_days = now.saturating_sub(last_action).max(0) / 86_400;
    (50_i64
        .saturating_add(likes.saturating_mul(10))
        .saturating_sub(dislikes.saturating_mul(15))
        .saturating_add(instructions.saturating_mul(5))
        .saturating_add(links.saturating_mul(5))
        .saturating_sub(idle_days.saturating_mul(15)))
    .clamp(0, 100) as u8
}

pub(crate) fn vote_priority(likes: i64, dislikes: i64) -> f64 {
    let total = likes.max(0).saturating_add(dislikes.max(0)) as f64;
    if total == 0.0 {
        return 0.0;
    }
    let positive = likes.max(0) as f64 / total;
    let z = 1.96;
    let denominator = 1.0 + z * z / total;
    let centre = positive + z * z / (2.0 * total);
    let margin = z * ((positive * (1.0 - positive) + z * z / (4.0 * total)) / total).sqrt();
    ((centre - margin) / denominator).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{reputation_score, vote_priority};

    #[test]
    fn reputation_uses_fixed_points_and_daily_inactivity() {
        let now = 3 * 86_400;
        assert_eq!(reputation_score(0, 0, 0, 0, now, now), 50);
        assert_eq!(reputation_score(1, 1, 1, 1, now, now), 55);
        assert_eq!(reputation_score(0, 0, 0, 0, now, now - 86_400), 35);
        assert_eq!(reputation_score(10, 0, 0, 0, now, now), 100);
        assert_eq!(reputation_score(0, 10, 0, 0, now, now), 0);
    }

    #[test]
    fn votes_rank_popular_clean_routes_above_unpopular_or_disliked_ones() {
        assert_eq!(vote_priority(0, 0), 0.0);
        assert!(vote_priority(100, 4) > vote_priority(16, 0));
        assert!(vote_priority(100, 68) < vote_priority(16, 0));
    }
}

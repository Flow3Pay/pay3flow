use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::core::redis::RedisPool;
use crate::db::DbPool;

mod engagement;
pub(crate) use engagement::vote_priority;
use engagement::{EngagementKind, EngagementMetrics};

const TRACKING_TOKEN_TTL_SECS: u64 = 30 * 60;

#[derive(Debug, Error)]
pub enum ReputationError {
    #[error("service not found")]
    ServiceNotFound,
    #[error("invalid or expired service link")]
    InvalidTrackingToken,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
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

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStats {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub executions_total: i64,
    pub likes_total: i64,
    pub dislikes_total: i64,
    pub reputation_score: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_vote: Option<VoteChoice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteServiceStats {
    #[serde(flatten)]
    pub stats: ServiceStats,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceLinkKind {
    Entry,
    Exit,
    MarketSource,
    MarketTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceLink {
    pub service_id: Uuid,
    pub service_slug: String,
    pub kind: ServiceLinkKind,
    pub tracking_token: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CombinedReputation {
    pub executions_average: i64,
    pub likes_average: i64,
    pub dislikes_average: i64,
    pub score_average: u8,
}

/// Anonymous feedback for one concrete route shown in a search result.
#[derive(Debug, Clone, Serialize)]
pub struct RouteFeedback {
    pub likes_total: i64,
    pub dislikes_total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewer_vote: Option<VoteChoice>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionOpen {
    pub execution_id: Uuid,
    pub newly_recorded: bool,
    pub redirect_url: String,
    pub service: ServiceStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackingClaims {
    service_id: Uuid,
    #[serde(default)]
    service_slug: String,
    search_id: Uuid,
    route_id: String,
    destination_url: String,
    exp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstructionClaims {
    search_id: Uuid,
    route_id: String,
    services: Vec<String>,
    exp: u64,
}

#[derive(Clone)]
pub struct ServiceReputation {
    pool: DbPool,
    encode_key: EncodingKey,
    decode_key: DecodingKey,
    engagement: Option<Arc<EngagementMetrics>>,
}

impl ServiceReputation {
    fn current_reputation_score(&self, slug: &str) -> u8 {
        self.engagement
            .as_ref()
            .and_then(|metrics| metrics.reputations().read().get(slug).copied())
            .unwrap_or(50)
    }
    pub fn new(pool: DbPool, secret: &str) -> Self {
        Self {
            pool,
            encode_key: EncodingKey::from_secret(secret.as_bytes()),
            decode_key: DecodingKey::from_secret(secret.as_bytes()),
            engagement: None,
        }
    }

    pub fn with_engagement(mut self, redis: Option<RedisPool>, secret: &str) -> Self {
        self.engagement =
            redis.map(|redis| EngagementMetrics::new(self.pool.clone(), redis, secret.as_bytes()));
        self
    }

    pub fn vote_ranking_scores(&self) -> Option<Arc<parking_lot::RwLock<HashMap<String, f64>>>> {
        self.engagement.as_ref().map(|metrics| metrics.scores())
    }

    pub fn reputation_scores(&self) -> Option<Arc<parking_lot::RwLock<HashMap<String, u8>>>> {
        self.engagement
            .as_ref()
            .map(|metrics| metrics.reputations())
    }

    pub async fn start_search(&self, search_id: Uuid) -> Result<(), ReputationError> {
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
INSERT INTO route_searches (id, status, routes_found)
VALUES ($1, 'searching', 0)
ON CONFLICT (id) DO NOTHING
"#,
                &[&search_id],
            )
            .await?;
        Ok(())
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
        let client = self.pool.get().await?;
        let rows = client
            .query(
                r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, v.vote
FROM services s
LEFT JOIN service_votes v
  ON v.service_id = s.id AND v.anonymous_id = $2
WHERE s.slug = ANY($1)
"#,
                &[&slugs, &anonymous_id],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let mut stats = row_to_stats(row);
                stats.reputation_score = self.current_reputation_score(&stats.slug);
                stats
            })
            .map(|stats| (stats.slug.clone(), stats))
            .collect())
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
        service_slug: &str,
        search_id: Uuid,
        route_id: &str,
        destination_url: &str,
    ) -> Result<String, ReputationError> {
        if !valid_destination(destination_url) {
            return Err(ReputationError::InvalidTrackingToken);
        }
        let claims = TrackingClaims {
            service_id,
            service_slug: service_slug.to_string(),
            search_id,
            route_id: route_id.to_string(),
            destination_url: destination_url.to_string(),
            exp: now_secs().saturating_add(TRACKING_TOKEN_TTL_SECS),
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encode_key)
            .map_err(|_| ReputationError::InvalidTrackingToken)
    }

    pub fn instruction_token(
        &self,
        search_id: Uuid,
        route_id: &str,
        services: Vec<String>,
    ) -> Result<String, ReputationError> {
        encode(
            &Header::new(Algorithm::HS256),
            &InstructionClaims {
                search_id,
                route_id: route_id.to_string(),
                services,
                exp: now_secs().saturating_add(TRACKING_TOKEN_TTL_SECS),
            },
            &self.encode_key,
        )
        .map_err(|_| ReputationError::InvalidTrackingToken)
    }

    pub fn record_instruction_open(
        &self,
        token: &str,
        anonymous_id: Uuid,
    ) -> Result<(), ReputationError> {
        let claims = decode::<InstructionClaims>(
            token,
            &self.decode_key,
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| ReputationError::InvalidTrackingToken)?
        .claims;
        if let Some(metrics) = &self.engagement {
            metrics.record(
                anonymous_id,
                EngagementKind::Instruction,
                format!("{}:{}", claims.search_id, claims.route_id),
                claims.services,
            );
        }
        Ok(())
    }

    pub fn route_services_for_vote(&self, route_id: &str, token: Option<&str>) -> Vec<String> {
        token
            .and_then(|token| {
                decode::<InstructionClaims>(
                    token,
                    &self.decode_key,
                    &Validation::new(Algorithm::HS256),
                )
                .ok()
            })
            .map(|decoded| decoded.claims)
            .filter(|claims| claims.route_id == route_id)
            .map(|claims| claims.services)
            .unwrap_or_default()
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

        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                r#"
SELECT s.id, s.slug, s.display_name, s.executions_total, s.likes_total,
       s.dislikes_total, v.vote
FROM services s
LEFT JOIN service_votes v
  ON v.service_id = s.id AND v.anonymous_id = $2
WHERE s.id = $1
"#,
                &[&claims.service_id, &anonymous_id],
            )
            .await?
            .ok_or(ReputationError::ServiceNotFound)?;
        let mut service = row_to_stats(row);
        service.reputation_score = self.current_reputation_score(&service.slug);

        if let Some(metrics) = &self.engagement {
            if !claims.service_slug.is_empty() {
                metrics.record(
                    anonymous_id,
                    EngagementKind::Link,
                    format!(
                        "{}:{}:{}",
                        claims.search_id, claims.route_id, claims.service_slug
                    ),
                    vec![claims.service_slug],
                );
            }
        }

        Ok(ExecutionOpen {
            execution_id: Uuid::new_v4(),
            newly_recorded: true,
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
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        let service_exists = transaction
            .query_opt(
                "SELECT id FROM services WHERE id = $1 FOR UPDATE",
                &[&service_id],
            )
            .await?
            .is_some();
        if !service_exists {
            return Err(ReputationError::ServiceNotFound);
        }
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
        transaction
            .execute(
                r#"
UPDATE services
SET likes_total = likes_total + $2,
    dislikes_total = dislikes_total + $3,
    updated_at = now()
WHERE id = $1
"#,
                &[&service_id, &likes_delta, &dislikes_delta],
            )
            .await?;
        let row = transaction
            .query_one(
                r#"
SELECT id, slug, display_name, executions_total, likes_total, dislikes_total, $2::TEXT
FROM services WHERE id = $1
"#,
                &[&service_id, &vote.as_str()],
            )
            .await?;
        let mut stats = row_to_stats(row);
        stats.reputation_score = self.current_reputation_score(&stats.slug);
        transaction.commit().await?;
        Ok(stats)
    }

    pub async fn set_route_vote(
        &self,
        route_id: &str,
        anonymous_id: Uuid,
        vote: VoteChoice,
        services: &[String],
    ) -> Result<RouteFeedback, ReputationError> {
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        let lock_key = format!("{route_id}:{anonymous_id}");
        transaction
            .query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                &[&lock_key],
            )
            .await?;
        let previous = transaction
            .query_opt(
                "SELECT vote, attributed_services FROM route_votes WHERE anonymous_id = $1 AND route_id = $2 FOR UPDATE",
                &[&anonymous_id, &route_id],
            )
            .await?;
        let previous_vote = previous
            .as_ref()
            .and_then(|row| VoteChoice::parse(row.get::<_, String>(0).as_str()));
        let previous_services = previous
            .as_ref()
            .map(|row| row.get::<_, Vec<String>>(1))
            .unwrap_or_default();
        let mut attributed_services = if services.is_empty() {
            previous_services.clone()
        } else {
            services.to_vec()
        };
        attributed_services.sort();
        attributed_services.dedup();
        transaction
            .execute(
                r#"
INSERT INTO route_votes (anonymous_id, route_id, vote, attributed_services)
VALUES ($1, $2, $3, $4)
ON CONFLICT (anonymous_id, route_id) DO UPDATE SET
    vote = EXCLUDED.vote,
    attributed_services = EXCLUDED.attributed_services,
    updated_at = now()
"#,
                &[
                    &anonymous_id,
                    &route_id,
                    &vote.as_str(),
                    &attributed_services,
                ],
            )
            .await?;
        if let Some(previous_vote) = previous_vote {
            if !previous_services.is_empty() {
                if previous_services == attributed_services {
                    let (likes_delta, dislikes_delta) = vote_deltas(Some(previous_vote), vote);
                    if likes_delta != 0 || dislikes_delta != 0 {
                        transaction.execute(
                            "UPDATE services SET likes_total = GREATEST(0, likes_total + $2), dislikes_total = GREATEST(0, dislikes_total + $3), updated_at = now() WHERE slug = ANY($1)",
                            &[&previous_services, &likes_delta, &dislikes_delta],
                        ).await?;
                    }
                } else {
                    let (old_like, old_dislike) = match previous_vote {
                        VoteChoice::Like => (-1_i64, 0_i64),
                        VoteChoice::Dislike => (0_i64, -1_i64),
                    };
                    transaction.execute(
                        "UPDATE services SET likes_total = GREATEST(0, likes_total + $2), dislikes_total = GREATEST(0, dislikes_total + $3), updated_at = now() WHERE slug = ANY($1)",
                        &[&previous_services, &old_like, &old_dislike],
                    ).await?;
                }
            }
        }
        if previous_vote.is_none() || previous_services != attributed_services {
            if !attributed_services.is_empty() {
                let (new_like, new_dislike) = match vote {
                    VoteChoice::Like => (1_i64, 0_i64),
                    VoteChoice::Dislike => (0_i64, 1_i64),
                };
                transaction.execute(
                    "UPDATE services SET likes_total = likes_total + $2, dislikes_total = dislikes_total + $3, updated_at = now() WHERE slug = ANY($1)",
                    &[&attributed_services, &new_like, &new_dislike],
                ).await?;
            }
        }
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
        return CombinedReputation {
            score_average: 50,
            ..CombinedReputation::default()
        };
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
        score_average: rounded_average(total(|service| service.stats.reputation_score as i64))
            as u8,
    }
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

fn row_to_stats(row: tokio_postgres::Row) -> ServiceStats {
    ServiceStats {
        id: row.get(0),
        slug: row.get(1),
        display_name: row.get(2),
        executions_total: row.get(3),
        likes_total: row.get(4),
        dislikes_total: row.get(5),
        reputation_score: 50,
        viewer_vote: row
            .get::<_, Option<String>>(6)
            .as_deref()
            .and_then(VoteChoice::parse),
    }
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
}

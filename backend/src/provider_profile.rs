//! Provider statistics from completed, explicitly counted user searches.

use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::{error::AppError, state::AppState};
use crate::db::DbPool;
use crate::p2p::P2pRouteSearchResponse;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub enum ProfilePeriod {
    #[serde(rename = "7d")]
    Week,
    #[default]
    #[serde(rename = "30d")]
    Month,
    #[serde(rename = "90d")]
    Quarter,
}

impl ProfilePeriod {
    fn days(self) -> i32 {
        match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct StatisticsQuery {
    #[serde(default)]
    period: ProfilePeriod,
}

#[derive(Debug, Default)]
struct Observation {
    best_rank: Option<i32>,
    response_ok: Option<bool>,
    latency_sum: f64,
    latency_samples: u32,
}

fn observations(response: &P2pRouteSearchResponse) -> BTreeMap<String, Observation> {
    let mut result = BTreeMap::<String, Observation>::new();
    let statuses = response.provider_statuses.iter().chain(
        response
            .asset_statuses
            .iter()
            .flat_map(|asset| asset.entry_sources.iter().chain(&asset.exit_sources)),
    );
    for status in statuses {
        let observation = result
            .entry(status.source.to_ascii_lowercase())
            .or_default();
        observation.response_ok = Some(observation.response_ok.unwrap_or(false) || status.ok);
        // A cached quote is an observation, but not a network latency sample.
        if !status.cached {
            observation.latency_sum += status.latency_ms as f64;
            observation.latency_samples += 1;
        }
    }
    for route in &response.routes {
        let slugs = route
            .entry_offer
            .iter()
            .map(|offer| offer.source.as_str())
            .chain(route.exit_offer.iter().map(|offer| offer.source.as_str()))
            .chain(route.market_path.iter().map(|market| market.venue.as_str()))
            .chain(route.route_provider.as_deref())
            .chain(route.cycle_legs.iter().map(|leg| leg.provider.as_str()));
        for slug in slugs {
            let observation = result.entry(slug.to_ascii_lowercase()).or_default();
            if let Ok(rank) = i32::try_from(route.rank) {
                if rank > 0 {
                    observation.best_rank =
                        Some(observation.best_rank.map_or(rank, |old| old.min(rank)));
                }
            }
        }
    }
    result
}

/// Persist the final displayed ranking once. Progressive updates, automatic
/// refreshes and failed/cancelled searches never enter the profile statistics.
pub async fn record_search(pool: &DbPool, response: &P2pRouteSearchResponse) -> anyhow::Result<()> {
    let observations = observations(response);
    let slugs: Vec<_> = observations.keys().collect();
    let ranks: Vec<_> = observations.values().map(|value| value.best_rank).collect();
    let successes: Vec<_> = observations
        .values()
        .map(|value| value.response_ok)
        .collect();
    let latencies: Vec<_> = observations
        .values()
        .map(|value| {
            (value.latency_samples > 0)
                .then(|| value.latency_sum / f64::from(value.latency_samples))
        })
        .collect();
    let client = pool.get().await?;
    client.execute(
        r#"
INSERT INTO provider_search_observations (search_id, provider_slug, best_rank, response_ok, latency_ms)
SELECT searches.id, observation.slug, observation.rank, observation.ok, observation.latency
FROM route_searches searches
CROSS JOIN unnest($2::text[], $3::integer[], $4::boolean[], $5::double precision[])
    AS observation(slug, rank, ok, latency)
WHERE searches.id = $1 AND searches.counted_for_activity AND searches.status = 'finished'
ON CONFLICT (search_id, provider_slug) DO NOTHING
"#,
        &[&response.search_id, &slugs, &ranks, &successes, &latencies],
    ).await?;
    Ok(())
}

/// Record analytics without turning a successful route search into a failure.
pub async fn observe_search(state: &AppState, response: &P2pRouteSearchResponse) {
    if let Err(error) = record_search(&state.pool, response).await {
        tracing::warn!(search_id = %response.search_id, error = %error, "provider.statistics.record_failed");
    }
}

#[derive(Debug, Serialize)]
pub struct ProfileDay {
    started_at: DateTime<Utc>,
    searches: i64,
    top10: i64,
    top1: i64,
}

#[derive(Debug, Serialize)]
pub struct ProfileDirection {
    source_currency: String,
    target_currency: String,
    searches: i64,
    top10: i64,
    best_rank: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatistics {
    slug: String,
    period_days: i32,
    searches: i64,
    top10: i64,
    top1: i64,
    average_rank: Option<f64>,
    response_samples: i64,
    successful_responses: i64,
    average_response_ms: Option<f64>,
    site_opens: i64,
    first_seen: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    days: Vec<ProfileDay>,
    directions: Vec<ProfileDirection>,
}

/// `GET /api/providers/:slug/statistics?period=30d` returns UTC calendar-day
/// statistics. Top-10 is counted once per search, relative to searches in
/// which that venue was observed. A site's opens are not completed trades.
pub async fn statistics(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<StatisticsQuery>,
) -> Result<Json<ProviderStatistics>, AppError> {
    load_statistics(&state.pool, &slug, query.period)
        .await
        .map(Json)
}

async fn load_statistics(
    pool: &DbPool,
    slug: &str,
    period: ProfilePeriod,
) -> Result<ProviderStatistics, AppError> {
    let client = pool
        .get()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let exists = client
        .query_opt(
            "SELECT 1 FROM providers WHERE slug = $1 AND status = 'enabled' LIMIT 1",
            &[&slug],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    if exists.is_none() {
        return Err(AppError::NotFound("Provider not found".into()));
    }
    let days = period.days();
    // A read-only repeatable snapshot keeps totals, buckets and directions
    // consistent if another search finishes between the queries.
    let mut client = client;
    let transaction = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let updated_at: DateTime<Utc> = transaction
        .query_one("SELECT now()", &[])
        .await
        .map_err(|error| AppError::Internal(error.into()))?
        .get(0);
    let summary = transaction.query_one(r#"
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
"#, &[&slug, &days]).await.map_err(|error| AppError::Internal(error.into()))?;
    let buckets = transaction.query(r#"
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
"#, &[&slug, &days]).await.map_err(|error| AppError::Internal(error.into()))?;
    let directions = transaction.query(r#"
SELECT searches.source_currency, searches.target_currency, count(*)::bigint,
       count(*) FILTER (WHERE observation.best_rank <= 10)::bigint, min(observation.best_rank)
FROM provider_search_observations observation
JOIN route_searches searches ON searches.id = observation.search_id
WHERE observation.provider_slug = $1 AND searches.status = 'finished' AND searches.counted_for_activity
  AND searches.source_currency IS NOT NULL AND searches.target_currency IS NOT NULL
  AND searches.created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' - ($2::integer - 1) * interval '1 day'
GROUP BY searches.source_currency, searches.target_currency
ORDER BY count(*) DESC, searches.source_currency, searches.target_currency LIMIT 8
"#, &[&slug, &days]).await.map_err(|error| AppError::Internal(error.into()))?;
    transaction
        .commit()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    Ok(ProviderStatistics {
        slug: slug.to_string(),
        period_days: days,
        searches: summary.get(0),
        top10: summary.get(1),
        top1: summary.get(2),
        average_rank: summary.get(3),
        response_samples: summary.get(4),
        successful_responses: summary.get(5),
        average_response_ms: summary.get(6),
        site_opens: summary.get(7),
        first_seen: summary.get(8),
        updated_at,
        days: buckets
            .into_iter()
            .map(|row| ProfileDay {
                started_at: row.get(0),
                searches: row.get(1),
                top10: row.get(2),
                top1: row.get(3),
            })
            .collect(),
        directions: directions
            .into_iter()
            .map(|row| ProfileDirection {
                source_currency: row.get(0),
                target_currency: row.get(1),
                searches: row.get(2),
                top10: row.get(3),
                best_rank: row.get(4),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(routes: serde_json::Value) -> P2pRouteSearchResponse {
        serde_json::from_value(json!({
            "search_id": "00000000-0000-0000-0000-000000000001", "routes_found": 3,
            "routes_exhaustive": false, "searched_at": "2026-10-10T12:00:00Z",
            "source_fiat": "RUB", "target_fiat": "AMD", "source_amount": "100",
            "assets_searched": [], "can_exchange_to_target": true,
            "routes": routes, "asset_statuses": [], "source": "live", "stale": false,
            "provider_statuses": [
                {"source": "bybit", "ok": true, "cached": false, "latency_ms": 400, "offers_found": 5, "error": null},
                {"source": "bybit", "ok": true, "cached": true, "latency_ms": 9000, "offers_found": 5, "error": null},
                {"source": "whitebird", "ok": false, "cached": false, "latency_ms": 1200, "offers_found": 0, "error": "timeout"}
            ]
        })).unwrap()
    }

    fn route(rank: i32, provider: &str) -> serde_json::Value {
        json!({
            "route_id": format!("route-{rank}"), "rank": rank, "asset": "USDT",
            "source_fiat": "RUB", "source_amount": "100", "acquired_asset_amount": "1",
            "target_fiat": "AMD", "target_amount": "400", "effective_rate": "4",
            "same_venue": true, "requires_asset_transfer": false, "transfer_fee_included": true,
            "route_kind": "fiat_to_fiat", "route_provider": provider, "payment_methods_verified": true, "warnings": []
        })
    }

    #[test]
    fn counts_each_venue_once_and_keeps_best_displayed_rank() {
        let response = response(json!([
            route(11, "BYBIT"),
            route(2, "bybit"),
            route(1, "id-pay")
        ]));
        let observations = observations(&response);
        assert_eq!(observations.len(), 3);
        assert_eq!(observations["bybit"].best_rank, Some(2));
        assert_eq!(observations["id-pay"].best_rank, Some(1));
        assert_eq!(observations["whitebird"].best_rank, None);
        assert_eq!(observations["whitebird"].response_ok, Some(false));
    }

    #[test]
    fn excludes_cached_responses_from_latency_samples() {
        let observations = observations(&response(json!([])));
        assert_eq!(observations["bybit"].latency_samples, 1);
        assert_eq!(observations["bybit"].latency_sum, 400.0);
    }

    #[tokio::test]
    #[ignore = "requires PROFILE_TEST_DATABASE_URL; uses only temporary tables"]
    async fn statistics_count_final_user_searches_once_and_use_utc_buckets() {
        let url = std::env::var("PROFILE_TEST_DATABASE_URL")
            .expect("PROFILE_TEST_DATABASE_URL is required");
        let mut config = deadpool_postgres::Config::new();
        config.url = Some(url);
        config.pool = Some(deadpool_postgres::PoolConfig::new(1));
        let pool = config
            .create_pool(
                Some(deadpool_postgres::Runtime::Tokio1),
                tokio_postgres::NoTls,
            )
            .unwrap();
        {
            let client = pool.get().await.unwrap();
            client.batch_execute(r#"
SET TIME ZONE 'Pacific/Kiritimati';
CREATE TEMP TABLE providers (slug text, status text);
CREATE TEMP TABLE route_searches (id uuid PRIMARY KEY, source_currency text, target_currency text, counted_for_activity boolean, status text, created_at timestamptz);
CREATE TEMP TABLE services (id uuid, slug text);
CREATE TEMP TABLE service_executions (service_id uuid, reputation_score_delta smallint, created_at timestamptz);
CREATE TEMP TABLE provider_search_observations (search_id uuid REFERENCES route_searches(id), provider_slug text, best_rank integer CHECK(best_rank > 0), response_ok boolean, latency_ms double precision, PRIMARY KEY(search_id, provider_slug));
INSERT INTO providers VALUES ('bybit', 'enabled'), ('whitebird', 'enabled');
INSERT INTO route_searches
SELECT ('00000000-0000-0000-0000-' || lpad(n::text, 12, '0'))::uuid,
       'RUB', 'AMD', n <> 4,
       CASE n WHEN 5 THEN 'failed' WHEN 6 THEN 'searching' ELSE 'finished' END,
       now() - CASE n WHEN 3 THEN interval '9 days' WHEN 7 THEN interval '40 days' ELSE interval '0 days' END
FROM generate_series(1, 7) n;
INSERT INTO services VALUES ('00000000-0000-0000-0000-000000000099', 'bybit');
INSERT INTO service_executions VALUES
('00000000-0000-0000-0000-000000000099', 5, now()),
('00000000-0000-0000-0000-000000000099', 0, now()),
('00000000-0000-0000-0000-000000000099', 5, now() - interval '40 days');
"#).await.unwrap();
        }
        for n in 1..=7 {
            let routes = if n == 3 {
                json!([])
            } else {
                json!([route(11, "BYBIT"), route(2, "bybit"), route(1, "bybit")])
            };
            let mut snapshot = response(routes);
            snapshot.search_id = uuid::Uuid::from_u128(n);
            record_search(&pool, &snapshot).await.unwrap();
            record_search(&pool, &snapshot).await.unwrap(); // Replay must not inflate counts.
        }
        let month = load_statistics(&pool, "bybit", ProfilePeriod::Month)
            .await
            .unwrap();
        assert_eq!(month.searches, 3); // Excludes auto, failed, progressive and old searches.
        assert_eq!(month.top10, 2);
        assert_eq!(month.top1, 2);
        assert_eq!(month.average_rank, Some(1.0));
        assert_eq!(month.average_response_ms, Some(400.0));
        assert_eq!(month.response_samples, 3);
        assert_eq!(month.successful_responses, 3);
        // Valid opens still count when the reputation score is already at its cap.
        assert_eq!(month.site_opens, 2);
        assert_eq!(month.days.len(), 30);
        assert_eq!(month.days.iter().map(|day| day.searches).sum::<i64>(), 3);
        assert!(month
            .days
            .iter()
            .all(|day| day.started_at.timestamp() % 86400 == 0));
        assert_eq!(month.directions.len(), 1);
        assert_eq!(month.directions[0].searches, 3);
        assert_eq!(month.directions[0].top10, 2);
        assert_eq!(month.directions[0].best_rank, Some(1));
        let week = load_statistics(&pool, "bybit", ProfilePeriod::Week)
            .await
            .unwrap();
        assert_eq!(week.searches, 2);
        assert_eq!(week.days.len(), 7);
        let whitebird = load_statistics(&pool, "whitebird", ProfilePeriod::Month)
            .await
            .unwrap();
        assert_eq!(whitebird.searches, 3);
        assert_eq!(whitebird.top10, 0);
        assert_eq!(whitebird.average_rank, None);
        assert_eq!(whitebird.successful_responses, 0);
        assert!(matches!(
            load_statistics(&pool, "missing", ProfilePeriod::Month).await,
            Err(AppError::NotFound(_))
        ));
    }
}

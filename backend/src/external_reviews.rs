//! Cached reviews of non-P2P providers from explicitly configured sources.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

use crate::{
    compiled_review_code,
    core::{error::AppError, state::AppState},
    db::DbPool,
    providers::ProviderGuidance,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalReview {
    pub id: String,
    pub author: String,
    pub text: String,
    pub rating: Option<u8>,
    pub created_at: Option<DateTime<Utc>>,
    pub url: String,
    /// A source-hosted image URL. Image bytes are never fetched or persisted here.
    #[serde(default)]
    pub avatar_url: Option<String>,
}

pub fn source_avatar(raw: &str, base: &str, allowed_host: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let url = Url::parse(base).ok()?.join(raw).ok()?;
    let host = url.host_str()?;
    (url.scheme() == "https"
        && (host == allowed_host || host.ends_with(&format!(".{allowed_host}"))))
    .then(|| url.to_string())
}

#[derive(Debug, Serialize)]
pub struct ReviewSnapshot {
    pub source_url: String,
    pub fetched_at: Option<DateTime<Utc>>,
    pub reviews: Vec<ExternalReview>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileReviewsQuery {
    url: String,
}

static REVIEW_CLIENT: OnceLock<Client> = OnceLock::new();
static REFRESHING_PROFILES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub async fn provider_reviews(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<ReviewSnapshot>, AppError> {
    let client = state
        .pool
        .get()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    let row = client
        .query_opt(
            "SELECT guidance FROM providers WHERE slug = $1 AND status = 'enabled' LIMIT 1",
            &[&slug],
        )
        .await
        .map_err(|error| AppError::Internal(error.into()))?
        .ok_or_else(|| AppError::NotFound("provider not found".into()))?;
    let guidance: ProviderGuidance = serde_json::from_value(row.get("guidance"))
        .map_err(|error| AppError::Internal(error.into()))?;
    let source = guidance
        .review_sources
        .first()
        .ok_or_else(|| AppError::NotFound("provider has no review source".into()))?;
    let row = client.query_opt(
        "SELECT source_url, reviews, fetched_at FROM external_review_cache WHERE subject_key = $1 AND fetched_at > now() - interval '7 days'",
        &[&format!("provider:{slug}")],
    ).await.map_err(|error| AppError::Internal(error.into()))?;
    let (source_url, reviews, fetched_at) = match row {
        Some(row)
            if guidance
                .review_sources
                .iter()
                .any(|source| source.url == row.get::<_, String>("source_url")) =>
        {
            let reviews = serde_json::from_value(row.get("reviews"))
                .map_err(|error| AppError::Internal(error.into()))?;
            (row.get("source_url"), reviews, Some(row.get("fetched_at")))
        }
        _ => (source.url.clone(), Vec::new(), None),
    };
    Ok(Json(ReviewSnapshot {
        source_url,
        fetched_at,
        reviews,
    }))
}

/// Fetch one selected exchanger or P2P account's feedback after its route is shown.
/// The host and path are checked before any network request to avoid arbitrary URLs.
pub async fn profile_reviews(
    State(state): State<AppState>,
    Query(query): Query<ProfileReviewsQuery>,
) -> Result<Json<ReviewSnapshot>, AppError> {
    let url =
        Url::parse(&query.url).map_err(|_| AppError::BadRequest("invalid review URL".into()))?;
    if url.scheme() != "https" {
        return Err(AppError::BadRequest("unsupported review URL".into()));
    }
    let (slug, profile) = compiled_review_code::profile_identity(&url)
        .ok_or_else(|| AppError::BadRequest("unsupported review URL".into()))?
        .map_err(|error| AppError::BadRequest(error.to_string()))?;
    let (source_url, identity) = profile;
    let subject_key = format!("profile:{slug}:v4:{identity}");
    let db = state
        .pool
        .get()
        .await
        .map_err(|error| AppError::Internal(error.into()))?;
    if let Some(row) = db.query_opt(
        "SELECT reviews, fetched_at FROM external_review_cache WHERE subject_key = $1 AND source_url = $2 AND fetched_at > now() - interval '7 days'",
        &[&subject_key, &source_url],
    ).await.map_err(|error| AppError::Internal(error.into()))? {
        let fetched_at: DateTime<Utc> = row.get("fetched_at");
        if Utc::now().signed_duration_since(fetched_at) > chrono::Duration::hours(12) {
            refresh_stale_profile(state.pool.clone(), slug, source_url.clone(), identity, subject_key);
        }
        return Ok(Json(ReviewSnapshot {
            source_url,
            fetched_at: Some(fetched_at),
            reviews: serde_json::from_value(row.get("reviews")).map_err(|error| AppError::Internal(error.into()))?,
        }));
    }
    drop(db);
    let http = review_client().map_err(AppError::Internal)?;
    let (reviews, bitget_has_more) = if slug == "bitget" {
        let initial =
            compiled_review_code::bitget::fetch_profile_initial(&http, &source_url, &identity)
                .await
                .map_err(AppError::Internal)?;
        (initial.reviews, initial.has_more)
    } else {
        (
            compiled_review_code::fetch_profile(slug, &http, &source_url, &identity)
                .await
                .map_err(AppError::Internal)?,
            false,
        )
    };
    let cached_reviews =
        serde_json::to_value(&reviews).map_err(|error| AppError::Internal(error.into()))?;
    let bitget_seed = bitget_has_more.then(|| reviews.clone());
    let pool = state.pool.clone();
    let cached_source_url = source_url.clone();
    tokio::spawn(async move {
        let result: Result<()> = async {
            {
                let db = pool.get().await?;
                db.execute(
                    "INSERT INTO external_review_cache (subject_key, source_url, reviews, fetched_at) VALUES ($1, $2, $3, now()) ON CONFLICT (subject_key) DO UPDATE SET source_url = EXCLUDED.source_url, reviews = EXCLUDED.reviews, fetched_at = now()",
                    &[&subject_key, &cached_source_url, &cached_reviews],
                ).await?;
            }
            if let Some(seed) = bitget_seed {
                let http = review_client()?;
                let initial_count = seed.len();
                let full = compiled_review_code::bitget::fetch_profile_remaining(
                    &http,
                    &cached_source_url,
                    &identity,
                    seed,
                )
                .await?;
                if full.len() > initial_count {
                    let db = pool.get().await?;
                    db.execute(
                        "UPDATE external_review_cache SET reviews = $2, fetched_at = now() WHERE subject_key = $1 AND source_url = $3",
                        &[&subject_key, &serde_json::to_value(full)?, &cached_source_url],
                    ).await?;
                }
            }
            Ok(())
        }.await;
        if let Err(error) = result {
            tracing::warn!(%error, "profile review cache write failed");
        }
    });
    Ok(Json(ReviewSnapshot {
        source_url,
        fetched_at: Some(Utc::now()),
        reviews,
    }))
}

fn review_client() -> Result<Client> {
    if let Some(client) = REVIEW_CLIENT.get() {
        return Ok(client.clone());
    }
    let client = Client::builder().timeout(Duration::from_secs(20)).build()?;
    Ok(REVIEW_CLIENT.get_or_init(|| client).clone())
}

fn refresh_stale_profile(
    pool: DbPool,
    slug: &'static str,
    source_url: String,
    identity: String,
    subject_key: String,
) {
    let refreshing = REFRESHING_PROFILES.get_or_init(|| Mutex::new(HashSet::new()));
    let should_refresh = refreshing
        .lock()
        .is_ok_and(|mut active| active.insert(subject_key.clone()));
    if !should_refresh {
        return;
    }
    tokio::spawn(async move {
        let result: Result<()> = async {
            let http = review_client()?;
            let reviews = compiled_review_code::fetch_profile(slug, &http, &source_url, &identity).await?;
            let db = pool.get().await?;
            db.execute(
                "UPDATE external_review_cache SET reviews = $2, fetched_at = now() WHERE subject_key = $1 AND source_url = $3",
                &[&subject_key, &serde_json::to_value(reviews)?, &source_url],
            ).await?;
            Ok(())
        }.await;
        if let Err(error) = result {
            tracing::warn!(%error, "stale profile review refresh failed");
        }
        if let Some(refreshing) = REFRESHING_PROFILES.get() {
            if let Ok(mut active) = refreshing.lock() {
                active.remove(&subject_key);
            }
        }
    });
}

pub fn start_background_sync(pool: DbPool) {
    let key = std::env::var("TRUSTPILOT_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let Ok(client) = review_client() else {
        tracing::warn!("review sync disabled: HTTP client could not be created");
        return;
    };
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(12 * 60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = sync_all(&pool, &client, key.as_deref()).await {
                tracing::warn!(%error, "external review sync failed");
            }
        }
    });
}

async fn sync_all(pool: &DbPool, http: &Client, key: Option<&str>) -> Result<()> {
    let client = pool.get().await?;
    let rows = client.query(
        "SELECT DISTINCT ON (slug) slug, guidance FROM providers WHERE status = 'enabled' ORDER BY slug, operation",
        &[],
    ).await?;
    for row in rows {
        let slug: String = row.get("slug");
        let guidance_value: serde_json::Value = row.get("guidance");
        if guidance_value
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
        {
            continue;
        }
        let guidance: ProviderGuidance = match serde_json::from_value(guidance_value) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%slug, %error, "invalid review source configuration");
                continue;
            }
        };
        for source in guidance.review_sources {
            let Some(fetched) =
                compiled_review_code::fetch_source(&slug, http, &source.url, key).await
            else {
                continue;
            };
            match fetched {
                Ok(reviews) => {
                    if reviews.is_empty() {
                        continue;
                    }
                    client.execute(
                        "INSERT INTO external_review_cache (subject_key, source_url, reviews, fetched_at) VALUES ($1, $2, $3, now()) ON CONFLICT (subject_key) DO UPDATE SET source_url = EXCLUDED.source_url, reviews = EXCLUDED.reviews, fetched_at = now()",
                        &[&format!("provider:{slug}"), &source.url, &serde_json::to_value(reviews)?],
                    ).await?;
                    break;
                }
                Err(error) => tracing::warn!(%slug, %error, "review source refresh failed"),
            }
        }
    }
    // Reviews removed upstream must not remain available indefinitely after a failed sync.
    client
        .execute(
            "DELETE FROM external_review_cache WHERE fetched_at < now() - interval '7 days'",
            &[],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::source_avatar;

    #[test]
    fn accepts_only_source_hosted_https_avatars() {
        assert_eq!(
            source_avatar(
                "//user-images.trustpilot.com/avatar.png",
                "https://www.trustpilot.com/",
                "trustpilot.com"
            ),
            Some("https://user-images.trustpilot.com/avatar.png".into())
        );
        assert_eq!(
            source_avatar("", "https://www.trustpilot.com/", "trustpilot.com"),
            None
        );
        assert_eq!(
            source_avatar(
                "http://user-images.trustpilot.com/avatar.png",
                "https://www.trustpilot.com/",
                "trustpilot.com"
            ),
            None
        );
        assert_eq!(
            source_avatar(
                "https://trustpilot.com.evil.example/avatar.png",
                "https://www.trustpilot.com/",
                "trustpilot.com"
            ),
            None
        );
    }
}

use crate::external_reviews::ExternalReview;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let url = Url::parse(source_url)?;
    match url.host_str() {
        Some("bestchange.biz") => super::bestchange::fetch_profile(http, source_url, "").await,
        Some("papa-change.biz") => fetch_provider_reviews(http, source_url).await,
        _ => bail!("unsupported Papa Change review source"),
    }
}

async fn fetch_provider_reviews(http: &Client, source_url: &str) -> Result<Vec<ExternalReview>> {
    let data: serde_json::Value = http
        .get("https://api.papa-change.biz/reviews/get-review")
        .header("Origin", "https://papa-change.biz")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(items) = data.as_array() else {
        bail!("Papa Change reviews missing")
    };
    Ok(items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_u64()?;
            let text = item.get("description")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }
            Some(ExternalReview {
                id: id.to_string(),
                author: item
                    .get("nameClient")
                    .and_then(serde_json::Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or("Anonymous")
                    .into(),
                text: text.into(),
                rating: None,
                created_at: item
                    .get("createdAt")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                url: source_url.into(),
                avatar_url: None,
            })
        })
        .collect())
}

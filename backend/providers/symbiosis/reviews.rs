use crate::external_reviews::{source_avatar, ExternalReview};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde::Deserialize;
#[derive(Debug, Deserialize)]
struct TrustpilotBusiness {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustpilotPage {
    #[serde(default)]
    reviews: Vec<TrustpilotReview>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustpilotReview {
    id: String,
    stars: u8,
    title: Option<String>,
    text: Option<String>,
    created_at: Option<DateTime<Utc>>,
    consumer: Option<TrustpilotConsumer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustpilotConsumer {
    display_name: Option<String>,
    image_url: Option<String>,
}

async fn fetch_trustpilot_public(http: &Client, source_url: &str) -> Result<Vec<ExternalReview>> {
    let mut reviews = Vec::new();
    for page in 1..=10 {
        let mut url = Url::parse(source_url)?;
        url.query_pairs_mut()
            .append_pair("languages", "all")
            .append_pair("page", &page.to_string());
        let html = http
            .get(url)
            .header("User-Agent", "Googlebot")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let document = Html::parse_document(&html);
        let selector = Selector::parse("script#__NEXT_DATA__")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let json = document
            .select(&selector)
            .next()
            .context("Trustpilot review data missing")?
            .inner_html();
        let data: serde_json::Value = serde_json::from_str(&json)?;
        let page_reviews = data
            .pointer("/props/pageProps/reviews")
            .and_then(serde_json::Value::as_array)
            .context("Trustpilot reviews missing")?;
        if page_reviews.is_empty() {
            break;
        }
        for review in page_reviews {
            let Some(id) = review.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let text = [
                review.get("title").and_then(serde_json::Value::as_str),
                review.get("text").and_then(serde_json::Value::as_str),
            ]
            .into_iter()
            .flatten()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" — ");
            if text.is_empty() {
                continue;
            }
            reviews.push(ExternalReview {
                id: id.into(),
                author: review
                    .pointer("/consumer/displayName")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Anonymous")
                    .trim()
                    .into(),
                text,
                rating: review
                    .get("rating")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|rating| u8::try_from(rating).ok()),
                created_at: review
                    .pointer("/dates/publishedDate")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                url: format!("https://www.trustpilot.com/reviews/{id}"),
                avatar_url: review
                    .pointer("/consumer/imageUrl")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|url| {
                        source_avatar(url, "https://www.trustpilot.com/", "trustpilot.com")
                    }),
            });
        }
        if page_reviews.len() < 20 {
            break;
        }
    }
    Ok(reviews)
}

async fn fetch_trustpilot(
    http: &Client,
    key: &str,
    source_url: &str,
) -> Result<Vec<ExternalReview>> {
    let profile = Url::parse(source_url)?;
    let domain = profile
        .path()
        .strip_prefix("/review/")
        .context("invalid review URL")?;
    if domain.is_empty() || domain.contains('/') {
        bail!("invalid Trustpilot domain");
    }
    let business: TrustpilotBusiness = http
        .get("https://api.trustpilot.com/v1/business-units/find")
        .header("apikey", key)
        .query(&[("name", domain)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Trustpilot business lookup failed")?;
    let endpoint = format!(
        "https://api.trustpilot.com/v1/business-units/{}/all-reviews",
        business.id
    );
    let mut token: Option<String> = None;
    let mut reviews = Vec::new();
    // Bound upstream work while covering the full history of typical providers.
    for _ in 0..100 {
        let mut request = http.get(&endpoint).header("apikey", key);
        if let Some(page_token) = &token {
            request = request.query(&[("pageToken", page_token)]);
        }
        let page: TrustpilotPage = request.send().await?.error_for_status()?.json().await?;
        reviews.extend(page.reviews.into_iter().filter_map(|review| {
            let text = [review.title.as_deref(), review.text.as_deref()]
                .into_iter()
                .flatten()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            (!text.is_empty()).then(|| ExternalReview {
                url: format!("https://www.trustpilot.com/reviews/{}", review.id),
                id: review.id,
                author: review
                    .consumer
                    .as_ref()
                    .and_then(|consumer| consumer.display_name.clone())
                    .unwrap_or_else(|| "Anonymous".into()),
                text,
                rating: Some(review.stars),
                created_at: review.created_at,
                avatar_url: review
                    .consumer
                    .as_ref()
                    .and_then(|consumer| consumer.image_url.as_deref())
                    .and_then(|url| {
                        source_avatar(url, "https://www.trustpilot.com/", "trustpilot.com")
                    }),
            })
        }));
        token = page.next_page_token.filter(|value| !value.is_empty());
        if token.is_none() {
            return Ok(reviews);
        }
    }
    bail!("Trustpilot review history exceeds the configured page limit")
}

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    match key {
        Some(key) => fetch_trustpilot(http, key, source_url).await,
        None => fetch_trustpilot_public(http, source_url).await,
    }
}

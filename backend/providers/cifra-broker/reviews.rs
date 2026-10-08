use crate::external_reviews::ExternalReview;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde_json::Value;

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let url = Url::parse(source_url)?;
    if url.scheme() != "https"
        || url.host_str() != Some("www.rustore.ru")
        || url.path() != "/catalog/app/by.ciframarkets.app/reviews"
    {
        bail!("invalid Cifra Markets review source");
    }
    let html = http
        .get(source_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_reviews(&html, source_url)
}

fn parse_reviews(html: &str, source_url: &str) -> Result<Vec<ExternalReview>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("script[type='application/ld+json']")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let data = document
        .select(&selector)
        .find_map(|script| {
            serde_json::from_str::<Value>(&script.text().collect::<String>())
                .ok()
                .filter(|data| {
                    data.pointer("/mainEntity/name").and_then(Value::as_str)
                        == Some("Cifra Markets")
                })
        })
        .context("Cifra Markets reviews missing")?;
    let items = data
        .pointer("/mainEntity/review")
        .and_then(Value::as_array)
        .context("Cifra Markets review list missing")?;
    Ok(items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let body = item.get("reviewBody")?.as_str()?.trim();
            if body.is_empty() {
                return None;
            }
            let date = item.get("datePublished").and_then(Value::as_str);
            Some(ExternalReview {
                id: format!("{}-{index}", date.unwrap_or("undated")),
                author: item
                    .pointer("/author/name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or("Anonymous")
                    .to_string(),
                text: body.to_string(),
                rating: item
                    .pointer("/reviewRating/ratingValue")
                    .and_then(Value::as_u64)
                    .filter(|rating| (1..=5).contains(rating))
                    .map(|rating| rating as u8),
                created_at: date
                    .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                url: source_url.to_string(),
                avatar_url: None,
            })
        })
        .take(100)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{fetch_source, parse_reviews};

    #[test]
    fn reads_rustore_review_metadata() {
        let html = r#"<script type="application/ld+json">{"mainEntity":{"name":"Cifra Markets","review":[{"author":{"name":"Аня"},"reviewBody":"Обмен прошёл","datePublished":"2026-08-28T06:55:43.376Z","reviewRating":{"ratingValue":5}}]}}</script>"#;
        let reviews = parse_reviews(
            html,
            "https://www.rustore.ru/catalog/app/by.ciframarkets.app/reviews",
        )
        .unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].rating, Some(5));
        assert_eq!(reviews[0].text, "Обмен прошёл");
    }

    #[tokio::test]
    #[ignore = "requires live RuStore"]
    async fn live_cifra_reviews_are_available() {
        let http = reqwest::Client::new();
        let reviews = fetch_source(
            &http,
            "https://www.rustore.ru/catalog/app/by.ciframarkets.app/reviews",
            None,
        )
        .await
        .unwrap();
        assert!(!reviews.is_empty());
    }
}

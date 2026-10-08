use crate::external_reviews::ExternalReview;
use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use scraper::{Html, Selector};

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let html = http
        .get(source_url)
        .version(reqwest::Version::HTTP_11)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let document = Html::parse_document(&html);
    let blocks =
        Selector::parse("[itemprop=review][itemscope]").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let authors = Selector::parse("[itemprop=author] [itemprop=name]")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let bodies = Selector::parse("[itemprop=reviewBody] .spoiler-view__text-container")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let ratings = Selector::parse("[itemprop=reviewRating] [itemprop=ratingValue]")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let dates =
        Selector::parse("[itemprop=datePublished]").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let icons = Selector::parse(".business-review-view__user-icon img[src]")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut reviews = Vec::new();
    for (index, block) in document.select(&blocks).enumerate() {
        let Some(body) = block.select(&bodies).next() else {
            continue;
        };
        let body = body.text().collect::<Vec<_>>().join(" ");
        if body.trim().is_empty() {
            continue;
        }
        let author = block
            .select(&authors)
            .next()
            .map(|node| node.text().collect::<String>())
            .unwrap_or_else(|| "Anonymous".into());
        let date_raw = block
            .select(&dates)
            .next()
            .and_then(|node| node.value().attr("content"));
        reviews.push(ExternalReview {
            id: format!("{index}-{}", date_raw.unwrap_or("")),
            author: author.trim().into(),
            text: body.trim().into(),
            rating: block
                .select(&ratings)
                .next()
                .and_then(|node| node.value().attr("content"))
                .and_then(|value| value.parse::<f64>().ok())
                .map(|rating| rating.round() as u8)
                .filter(|rating| (1..=5).contains(rating)),
            created_at: date_raw
                .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                .map(|date| date.with_timezone(&Utc)),
            url: source_url.into(),
            avatar_url: block
                .select(&icons)
                .next()
                .and_then(|node| node.value().attr("src"))
                .and_then(|raw| {
                    crate::external_reviews::source_avatar(raw, source_url, "yandex.com").or_else(
                        || crate::external_reviews::source_avatar(raw, source_url, "yandex.net"),
                    )
                }),
        });
    }
    if reviews.is_empty() {
        anyhow::bail!("review records missing from Yandex Maps")
    }
    Ok(reviews)
}

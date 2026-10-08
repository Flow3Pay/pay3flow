use crate::external_reviews::ExternalReview;
use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let html = http
        .get("https://id-pay.ru/")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let document = Html::parse_document(&html);
    let feedback = Selector::parse("#feedback").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let avatars = Selector::parse("[data-scope=avatar][data-part=root]")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let names = Selector::parse("p").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let text = Selector::parse("span[style*='overflow:hidden'] > span")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let section = document
        .select(&feedback)
        .next()
        .context("ID Pay feedback section missing")?;
    let mut reviews = Vec::new();
    for (index, avatar) in section.select(&avatars).enumerate() {
        let Some(card) = avatar
            .parent()
            .and_then(|row| row.parent())
            .and_then(ElementRef::wrap)
        else {
            continue;
        };
        let Some(body) = card.select(&text).next() else {
            continue;
        };
        let body = body.text().collect::<Vec<_>>().join(" ");
        if body.trim().is_empty() {
            continue;
        }
        let mut paragraphs = card.select(&names);
        let author = paragraphs
            .next()
            .map(|p| p.text().collect::<String>())
            .unwrap_or_else(|| "Anonymous".into());
        let rating = paragraphs
            .next()
            .and_then(|p| p.text().collect::<String>().parse::<u8>().ok())
            .filter(|rating| (1..=5).contains(rating));
        reviews.push(ExternalReview {
            id: format!("{index}-{}", author.trim()),
            author: author.trim().into(),
            text: body.trim().into(),
            rating,
            created_at: None,
            url: source_url.into(),
            avatar_url: None,
        });
    }
    Ok(reviews)
}

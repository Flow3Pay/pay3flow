use crate::external_reviews::ExternalReview;
use anyhow::{bail, Result};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let url = Url::parse(source_url)?;
    match url.host_str() {
        Some("trustscores.org") => fetch_trustscores(http, source_url).await,
        _ => bail!("unsupported CoW Swap review source"),
    }
}

async fn fetch_trustscores(http: &Client, source_url: &str) -> Result<Vec<ExternalReview>> {
    let url = Url::parse(source_url)?;
    if url.path() != "/companies/defi/swap.cow.fi" {
        bail!("invalid CoW Swap review source");
    }
    let html = http
        .get(source_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_trustscores(&html, source_url)
}

fn parse_trustscores(html: &str, source_url: &str) -> Result<Vec<ExternalReview>> {
    let document = Html::parse_document(html);
    let cards = Selector::parse(".review__card").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let names = Selector::parse(".review__name").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let titles = Selector::parse(".review__title").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let bodies = Selector::parse(".review__text").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let stars = Selector::parse(".rating__item-icon.filled")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    Ok(document
        .select(&cards)
        .filter_map(|card| {
            let author = card.select(&names).next()?.text().collect::<String>();
            let body = card.select(&bodies).next()?.text().collect::<String>();
            let title = card
                .select(&titles)
                .next()
                .map(|node| node.text().collect::<String>())
                .unwrap_or_default();
            let text = [title.trim(), body.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            if text.is_empty() {
                return None;
            }
            let id = format!(
                "{:x}",
                Sha256::digest(format!("{author}|{text}").as_bytes())
            );
            let rating = card.select(&stars).count();
            Some(ExternalReview {
                id,
                author,
                text,
                rating: (1..=5).contains(&rating).then_some(rating as u8),
                created_at: None,
                url: source_url.to_string(),
                avatar_url: None,
            })
        })
        .take(100)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{fetch_source, parse_trustscores};

    const SOURCE: &str = "https://trustscores.org/companies/defi/swap.cow.fi";

    #[test]
    fn reads_written_cow_review() {
        let html = r#"<div class="review__card"><div class="review__name">Elsi</div><div class="review__title">Smooth swap</div><div class="review__text">Worked well</div><svg class="rating__item-icon filled"></svg><svg class="rating__item-icon filled"></svg></div>"#;
        let reviews = parse_trustscores(html, SOURCE).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].author, "Elsi");
        assert_eq!(reviews[0].text, "Smooth swap — Worked well");
        assert_eq!(reviews[0].rating, Some(2));
    }

    #[tokio::test]
    #[ignore = "requires live TrustScores"]
    async fn live_cow_review_is_available() {
        let http = reqwest::Client::new();
        let reviews = fetch_source(&http, SOURCE, None).await.unwrap();
        assert!(!reviews.is_empty());
    }
}

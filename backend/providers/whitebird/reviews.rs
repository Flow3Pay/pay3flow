use crate::external_reviews::{source_avatar, ExternalReview};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
async fn fetch_otzovik(http: &Client, source_url: &str) -> Result<Vec<ExternalReview>> {
    let mut reviews = Vec::new();
    for page in 1..=10 {
        let url = if page == 1 {
            source_url.to_string()
        } else {
            format!("{}{page}/", source_url)
        };
        let response = http.get(&url).send().await?;
        if page > 1 && !response.status().is_success() {
            break;
        }
        let html = response.error_for_status()?.text().await?;
        let document = Html::parse_document(&html);
        let review_selector =
            Selector::parse("[itemprop=review][itemtype='http://schema.org/Review']")
                .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let author_selector = Selector::parse("[itemprop=author] [itemprop=name]")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let avatar_selector = Selector::parse("[itemprop=author] img[itemprop=image]")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let title_selector =
            Selector::parse(".review-title").map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let text_selector =
            Selector::parse(".review-teaser").map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let date_selector = Selector::parse("[itemprop=datePublished]")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let rating_selector = Selector::parse("[itemprop=ratingValue]")
            .map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let link_selector =
            Selector::parse("link[itemprop=url]").map_err(|error| anyhow::anyhow!("{error:?}"))?;
        let mut count = 0;
        for item in document.select(&review_selector) {
            let Some(link) = item
                .select(&link_selector)
                .next()
                .and_then(|node| node.value().attr("href"))
            else {
                continue;
            };
            let Some(id) = link
                .trim_start_matches("https://otzovik.com/review_")
                .strip_suffix(".html")
            else {
                continue;
            };
            let title = item
                .select(&title_selector)
                .next()
                .map(|node| node.text().collect::<String>())
                .unwrap_or_default();
            let body = item
                .select(&text_selector)
                .next()
                .map(|node| node.text().collect::<String>())
                .unwrap_or_default();
            let text = [title.trim(), body.trim()]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            if text.is_empty() {
                continue;
            }
            count += 1;
            reviews.push(ExternalReview {
                id: id.into(),
                author: item
                    .select(&author_selector)
                    .next()
                    .map(|node| node.text().collect::<String>())
                    .unwrap_or_else(|| "Anonymous".into()),
                text,
                rating: item
                    .select(&rating_selector)
                    .next()
                    .and_then(|node| node.value().attr("content"))
                    .and_then(|value| value.parse().ok()),
                created_at: item
                    .select(&date_selector)
                    .next()
                    .and_then(|node| node.value().attr("content"))
                    .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                url: link.into(),
                avatar_url: item
                    .select(&avatar_selector)
                    .next()
                    .and_then(|node| node.value().attr("src"))
                    .and_then(|url| source_avatar(url, source_url, "otzovik.com")),
            });
        }
        if count < 40 {
            break;
        }
    }
    Ok(reviews)
}

async fn fetch_forum(http: &Client, source_url: &str) -> Result<Vec<ExternalReview>> {
    let html = http
        .get(source_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let document = Html::parse_document(&html);
    let posts = Selector::parse("article.cPost[id^=elComment_]")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let authors =
        Selector::parse(".cAuthorPane_author a").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let bodies = Selector::parse("[data-role=commentContent]")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let dates = Selector::parse(".ipsComment_meta time[datetime]")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let avatars = Selector::parse(".cAuthorPane_photo img[itemprop=image]")
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let mut reviews = Vec::new();
    for post in document.select(&posts) {
        let Some(id) = post
            .value()
            .id()
            .and_then(|id| id.strip_prefix("elComment_"))
        else {
            continue;
        };
        let author = post
            .select(&authors)
            .next()
            .map(|node| node.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();
        if author.is_empty() || author.eq_ignore_ascii_case("Whitebird") {
            continue;
        }
        let text = post
            .select(&bodies)
            .next()
            .map(|body| {
                body.children()
                    .filter_map(scraper::ElementRef::wrap)
                    .filter(|child| matches!(child.value().name(), "p" | "div"))
                    .filter(|child| !child.value().classes().any(|class| class.contains("Quote")))
                    .map(|child| child.text().collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let lower = text.to_lowercase();
        let firsthand = [
            "завел",
            "завёл",
            "создал заявку",
            "обменял",
            "перевел",
            "перевёл",
            "вывел",
            "получил",
            "пополнил",
            "потерял",
            "мои usdt",
            "мой перевод",
            "вернули",
        ]
        .iter()
        .any(|word| lower.contains(word));
        if text.len() < 60 || !firsthand {
            continue;
        }
        reviews.push(ExternalReview {
            id: id.into(),
            author,
            text: text.chars().take(3000).collect(),
            rating: None,
            created_at: post
                .select(&dates)
                .next()
                .and_then(|node| node.value().attr("datetime"))
                .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                .map(|date| date.with_timezone(&Utc)),
            url: format!("{source_url}#elComment_{id}"),
            avatar_url: post
                .select(&avatars)
                .next()
                .and_then(|node| node.value().attr("src"))
                .filter(|url| !url.contains("default_photo"))
                .and_then(|url| source_avatar(url, source_url, "forum.bits.media")),
        });
    }
    reviews.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(reviews)
}

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    let url = Url::parse(source_url)?;
    match url.host_str() {
        Some("otzovik.com") => fetch_otzovik(http, source_url).await,
        Some("forum.bits.media") => fetch_forum(http, source_url).await,
        _ => bail!("unsupported Whitebird review source"),
    }
}

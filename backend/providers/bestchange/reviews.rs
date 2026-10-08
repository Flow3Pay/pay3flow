use crate::external_reviews::{source_avatar, ExternalReview};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::OnceCell;

static LEGACY_EXCHANGER_IDS: OnceCell<HashMap<String, u64>> = OnceCell::const_new();

fn exchanger_slug(source_url: &str) -> Result<String> {
    let url = Url::parse(source_url)?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some(
                "www.bestchange.com"
                    | "www.bestchange.pro"
                    | "www.bestchange.net"
                    | "bestchange.biz"
            )
        )
        || url.path().matches('/').count() != 1
    {
        bail!("invalid BestChange profile URL");
    }
    let path = url.path().trim_start_matches('/');
    let slug = path
        .strip_suffix("-exchanger.html")
        .or_else(|| path.strip_suffix("-exchanger"))
        .context("invalid BestChange exchanger path")?;
    if slug.is_empty()
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid BestChange exchanger slug");
    }
    Ok(slug.to_string())
}

fn modern_profile(slug: &str) -> String {
    format!("https://bestchange.biz/{slug}-exchanger")
}

fn exchanger_id(html: &str, slug: &str) -> Result<u64> {
    let document = Html::parse_document(html);
    let selector =
        Selector::parse("script#__NEXT_DATA__").map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let next_data = document
        .select(&selector)
        .next()
        .context("BestChange profile data missing")?
        .text()
        .collect::<String>();
    let page: Value = serde_json::from_str(&next_data)?;
    let props = &page["props"]["pageProps"];
    let exchanger_id = props["exchangerId"]
        .as_u64()
        .context("BestChange exchanger ID missing")?;
    if props["exchangerInfo"]["data"]["slug"].as_str() != Some(slug)
        || props["exchangerInfo"]["data"]["id"].as_u64() != Some(exchanger_id)
    {
        bail!("BestChange exchanger identity mismatch");
    }
    Ok(exchanger_id)
}

fn catalog_exchanger_ids(data: &Value) -> Result<HashMap<String, u64>> {
    let changers = data["changers"]
        .as_array()
        .context("BestChange exchanger catalog is missing")?;
    let mut ids = HashMap::new();
    for changer in changers {
        let Some(id) = changer["id"].as_u64() else {
            continue;
        };
        let Some(pages) = changer["pages"].as_object() else {
            continue;
        };
        for page in pages.values().filter_map(Value::as_str) {
            if let Ok(page) = Url::parse(page) {
                if let Some(host) = page.host_str() {
                    ids.insert(format!("{host}{}", page.path()), id);
                }
            }
        }
    }
    Ok(ids)
}

async fn legacy_exchanger_id(http: &Client, source_url: &str) -> Result<u64> {
    let ids = LEGACY_EXCHANGER_IDS
        .get_or_try_init(|| async {
            let key =
                std::env::var("BESTCHANGE_API_KEY").context("BestChange API key is unavailable")?;
            let mut url = Url::parse("https://bestchange.app")?;
            url.path_segments_mut()
                .map_err(|_| anyhow::anyhow!("invalid BestChange API URL"))?
                .pop_if_empty()
                .push("v2")
                .push(key.trim())
                .push("changers")
                .push("ru");
            let response = http
                .get(url)
                .send()
                .await
                .map_err(|error| anyhow::anyhow!(error.without_url()))?;
            if !response.status().is_success() {
                bail!("BestChange exchanger catalog is unavailable");
            }
            let data: Value = response
                .json()
                .await
                .map_err(|error| anyhow::anyhow!(error.without_url()))?;
            catalog_exchanger_ids(&data)
        })
        .await?;
    let url = Url::parse(source_url)?;
    let host = url.host_str().context("BestChange profile host missing")?;
    ids.get(&format!("{host}{}", url.path()))
        .copied()
        .context("BestChange exchanger is absent from its official catalog")
}

pub fn profile_identity(url: &Url) -> Result<(String, String)> {
    if !matches!(
        url.host_str(),
        Some("www.bestchange.com" | "www.bestchange.pro" | "www.bestchange.net")
    ) || !url.path().ends_with("-exchanger.html")
    {
        bail!("invalid BestChange profile URL");
    }
    let slug = exchanger_slug(url.as_str())?;
    if let Some(marker) = url.fragment() {
        let id = marker
            .strip_prefix("pay3flow-id=")
            .context("invalid BestChange exchanger marker")?
            .parse::<u64>()?;
        if id == 0 {
            bail!("invalid BestChange exchanger ID");
        }
        return Ok((url.as_str().to_string(), format!("id:{id}")));
    }
    let host = url.host_str().context("BestChange profile host missing")?;
    Ok((url.as_str().to_string(), format!("legacy:{host}:{slug}")))
}

pub async fn fetch_profile(
    http: &Client,
    source_url: &str,
    identity: &str,
) -> Result<Vec<ExternalReview>> {
    let slug = exchanger_slug(source_url)?;
    let profile = modern_profile(&slug);
    let exchanger_id = if let Some(id) = identity.strip_prefix("id:") {
        id.parse::<u64>()?
    } else if Url::parse(source_url)?.host_str() != Some("bestchange.biz") {
        legacy_exchanger_id(http, source_url).await?
    } else {
        let html = http
            .get(&profile)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        exchanger_id(&html, &slug)?
    };

    let mut reviews = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for page_number in 1..=10 {
        let mut url = Url::parse("https://bestchange.biz/api/exchanger/review")?;
        url.query_pairs_mut()
            .append_pair("exchangerId", &exchanger_id.to_string())
            .append_pair("page", &page_number.to_string())
            .append_pair("limit", "100");
        let data: Value = http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let items = data["items"]
            .as_array()
            .context("BestChange reviews missing")?;
        if items.is_empty() {
            break;
        }
        for item in items {
            if item["exchangerId"].as_u64() != Some(exchanger_id) {
                continue;
            }
            let Some(id) = item["id"].as_u64() else {
                continue;
            };
            let Some(body) = item["text"]
                .as_str()
                .map(str::trim)
                .filter(|body| !body.is_empty())
            else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            let author = item["authorId"]
                .as_str()
                .and_then(|id| data["entityRepo"]["user"].get(id));
            let created_at = item["createdAt"]
                .as_str()
                .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                .map(|date| date.with_timezone(&Utc));
            reviews.push(ExternalReview {
                id: id.to_string(),
                author: author
                    .and_then(|user| user["name"].as_str())
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or("Anonymous")
                    .to_string(),
                text: body.to_string(),
                rating: item["rating"]
                    .as_u64()
                    .filter(|rating| (1..=5).contains(rating))
                    .map(|rating| rating as u8),
                created_at,
                url: source_url.to_string(),
                avatar_url: author
                    .and_then(|user| user["avatarUrl"].as_str())
                    .and_then(|avatar| source_avatar(avatar, &profile, "bestchange.biz")),
            });
            if reviews.len() == 100 {
                return Ok(reviews);
            }
        }
        if data["nav"]["pages"]
            .as_u64()
            .is_some_and(|pages| page_number >= pages)
        {
            break;
        }
    }
    Ok(reviews)
}

pub async fn fetch_source(
    _http: &Client,
    _source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{catalog_exchanger_ids, exchanger_slug, fetch_profile, profile_identity};

    #[test]
    fn converts_only_bestchange_exchanger_urls() {
        assert_eq!(
            exchanger_slug("https://www.bestchange.com/papa-change-exchanger.html").unwrap(),
            "papa-change"
        );
        assert_eq!(
            exchanger_slug("https://bestchange.biz/papa-change-exchanger").unwrap(),
            "papa-change"
        );
        assert!(exchanger_slug("https://example.com/papa-change-exchanger").is_err());
        assert!(exchanger_slug("https://bestchange.biz/other/papa-change-exchanger").is_err());
    }

    #[test]
    fn uses_catalog_id_for_legacy_mirror_profiles() {
        let url = reqwest::Url::parse(
            "https://www.bestchange.pro/sapsan-exchanger.html#pay3flow-id=1446",
        )
        .unwrap();
        let (source, identity) = profile_identity(&url).unwrap();
        assert_eq!(source, url.as_str());
        assert_eq!(identity, "id:1446");
        let legacy =
            reqwest::Url::parse("https://www.bestchange.pro/sapsan-exchanger.html").unwrap();
        let (source, identity) = profile_identity(&legacy).unwrap();
        assert_eq!(source, legacy.as_str());
        assert_eq!(identity, "legacy:www.bestchange.pro:sapsan");
    }

    #[test]
    fn finds_legacy_profile_id_in_official_catalog() {
        let data = serde_json::json!({
            "changers": [{"id": 1446, "pages": {
                "ru": "https://www.bestchange.pro/sapsan-exchanger.html",
                "en": "https://www.bestchange.com/sapsan-exchanger.html"
            }}]
        });
        let ids = catalog_exchanger_ids(&data).unwrap();
        assert_eq!(ids["www.bestchange.pro/sapsan-exchanger.html"], 1446);
    }

    #[tokio::test]
    #[ignore = "requires live BestChange"]
    async fn live_bestchange_reviews_include_text_and_rating() {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let reviews = fetch_profile(&http, "https://bestchange.biz/papa-change-exchanger", "")
            .await
            .unwrap();
        assert_eq!(reviews.len(), 100);
        assert!(reviews
            .iter()
            .all(|review| !review.text.is_empty() && review.rating.is_some()));
    }

    #[tokio::test]
    #[ignore = "requires live BestChange"]
    async fn live_legacy_mirror_reviews_use_catalog_id() {
        let source = "https://www.bestchange.pro/sapsan-exchanger.html#pay3flow-id=1446";
        let http = reqwest::Client::new();
        let reviews = fetch_profile(&http, source, "id:1446").await.unwrap();
        assert_eq!(reviews.len(), 100);
        assert!(reviews.iter().all(|review| review.url == source));
    }
}

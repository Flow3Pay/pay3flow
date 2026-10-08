use crate::external_reviews::{source_avatar, ExternalReview};
use anyhow::{bail, Context, Result};
use chrono::DateTime;
use reqwest::{Client, Url};
use std::collections::HashSet;

pub fn profile_identity(url: &Url) -> Result<(String, String)> {
    if url.scheme() != "https" || url.host_str() != Some("www.mexc.com") {
        bail!("invalid MEXC profile URL");
    }
    let path = url.path();
    let id = path
        .strip_prefix("/buy-crypto/user-info/")
        .or_else(|| path.strip_prefix("/buy-crypto/merchant/"))
        .context("invalid MEXC profile path")?;
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        bail!("invalid MEXC member ID");
    }
    Ok((
        format!("https://www.mexc.com/buy-crypto/user-info/{id}"),
        id.into(),
    ))
}

pub async fn fetch_profile(
    http: &Client,
    source_url: &str,
    id: &str,
) -> Result<Vec<ExternalReview>> {
    let mut reviews = Vec::new();
    let mut seen = HashSet::new();
    // The endpoint accepts 100 reviews per page, avoiding ten serial requests
    // for a busy merchant while retaining the 100 written-review cap.
    for page in 1..=100 {
        let data: serde_json::Value = http
            .get("https://www.mexc.co/api/platform/p2p/api/order/review/out/list")
            .version(reqwest::Version::HTTP_11)
            .header("Referer", source_url)
            .header("Origin", "https://www.mexc.co")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json")
            .header("X-Client", "WEB")
            .query(&[("memberId", id), ("pageNum", &page.to_string()), ("pageSize", "100")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if data.get("code").and_then(serde_json::Value::as_i64) != Some(0) {
            bail!("MEXC review endpoint returned an error");
        }
        let items = data
            .pointer("/data/result")
            .and_then(serde_json::Value::as_array)
            .context("MEXC review data missing")?;
        if items.is_empty() {
            break;
        }
        let previous_count = reviews.len();
        for item in items {
            let Some(text) = item
                .get("comment")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let Some(review_id) = item.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !seen.insert(review_id.to_string()) {
                continue;
            }
            let anonymous =
                item.get("anonymous").and_then(serde_json::Value::as_bool) == Some(true);
            let avatar_url = (!anonymous)
                .then(|| {
                    item.get("profilePhoto")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|raw| {
                            source_avatar(raw, source_url, "mexc.com")
                                .or_else(|| source_avatar(raw, source_url, "mexc.co"))
                        })
                })
                .flatten();
            reviews.push(ExternalReview {
                id: review_id.into(),
                author: if anonymous {
                    "Anonymous".into()
                } else {
                    item.get("userName")
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or("Anonymous")
                        .into()
                },
                text: text.into(),
                rating: item
                    .get("rating")
                    .and_then(serde_json::Value::as_bool)
                    .map(|positive| if positive { 5 } else { 1 }),
                created_at: item
                    .get("updateTime")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(DateTime::from_timestamp_millis),
                url: source_url.into(),
                avatar_url,
            });
            if reviews.len() == 100 {
                return Ok(reviews);
            }
        }
        if data
            .pointer("/data/totalPage")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|last_page| page >= last_page)
            || reviews.len() == previous_count
            || (data.pointer("/data/totalPage").is_none() && items.len() < 100)
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
    use super::fetch_profile;

    #[tokio::test]
    #[ignore = "requires live MEXC"]
    async fn live_mexc_reviews_have_unique_ids() {
        let id = "56432472a6f14d1c968c0726a9070ef8";
        let url = format!("https://www.mexc.com/buy-crypto/user-info/{id}");
        let reviews = fetch_profile(&reqwest::Client::new(), &url, id)
            .await
            .unwrap();
        assert!(!reviews.is_empty());
        let ids = reviews
            .iter()
            .map(|review| &review.id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), reviews.len());
    }
}

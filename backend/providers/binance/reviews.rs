use crate::external_reviews::ExternalReview;
use anyhow::{bail, Context, Result};
use chrono::DateTime;
use reqwest::{Client, Url};
async fn fetch_binance(http: &Client, user_no: &str) -> Result<Vec<ExternalReview>> {
    let endpoint = "https://p2p.binance.com/bapi/c2c/v1/friendly/c2c/review/list-by-page";
    let mut reviews = Vec::new();
    for page in 1..=100 {
        let data: serde_json::Value = http
            .post(endpoint)
            .header("Origin", "https://c2c.binance.com")
            .header("Referer", "https://c2c.binance.com/")
            .json(&serde_json::json!({"userNo": user_no, "page": page, "rows": 20}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if data.get("code").and_then(serde_json::Value::as_str) != Some("000000") {
            bail!("Binance review endpoint returned an error");
        }
        let items = data
            .get("data")
            .and_then(serde_json::Value::as_array)
            .context("Binance review data missing")?;
        for item in items {
            let Some(text) = item
                .get("comments")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let Some(id) = item.get("reviewId").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let rating = match item.get("rating").and_then(serde_json::Value::as_u64) {
                Some(1) => Some(5), // Binance's positive feedback code.
                Some(3) => Some(1), // Binance's negative feedback code.
                _ => None,
            };
            reviews.push(ExternalReview {
                id: id.to_string(),
                author: if item.get("isAnonymous").and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    "Anonymous".into()
                } else {
                    item.pointer("/reviewer/nickname")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Anonymous")
                        .into()
                },
                text: text.into(),
                rating,
                created_at: item
                    .get("createTime")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(DateTime::from_timestamp_millis),
                url: format!("https://c2c.binance.com/en/advertiserDetail?advertiserNo={user_no}"),
                avatar_url: None,
            });
            if reviews.len() == 100 {
                return Ok(reviews);
            }
        }
        let total = data
            .get("total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if items.is_empty() || (page as u64) * 20 >= total {
            break;
        }
    }
    Ok(reviews)
}

pub fn profile_identity(url: &Url) -> Result<(String, String)> {
    if url.scheme() != "https"
        || url.host_str() != Some("c2c.binance.com")
        || !url.path().ends_with("/advertiserDetail")
    {
        bail!("invalid Binance profile URL");
    }
    let user_no = url
        .query_pairs()
        .find(|(key, _)| key == "advertiserNo")
        .map(|(_, value)| value.to_string())
        .context("advertiser number is missing")?;
    if user_no.len() > 64
        || user_no.is_empty()
        || !user_no.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        bail!("invalid advertiser number");
    }
    Ok((
        format!("https://c2c.binance.com/en/advertiserDetail?advertiserNo={user_no}"),
        user_no,
    ))
}
pub async fn fetch_profile(
    http: &Client,
    _source_url: &str,
    user_no: &str,
) -> Result<Vec<ExternalReview>> {
    fetch_binance(http, user_no).await
}
pub async fn fetch_source(
    _http: &Client,
    _source_url: &str,
    _key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    Ok(Vec::new())
}

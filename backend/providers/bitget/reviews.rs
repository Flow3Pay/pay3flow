use crate::external_reviews::ExternalReview;
use anyhow::{bail, Context, Result};
use chrono::DateTime;
use reqwest::{Client, StatusCode, Url};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

static NEXT_REVIEW_REQUEST: OnceLock<Mutex<Instant>> = OnceLock::new();

async fn wait_for_review_slot() {
    let gate = NEXT_REVIEW_REQUEST.get_or_init(|| Mutex::new(Instant::now()));
    let mut next = gate.lock().await;
    let delay = next.saturating_duration_since(Instant::now());
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    *next = Instant::now() + Duration::from_millis(800);
}

async fn fetch_page(
    http: &Client,
    source_url: &str,
    id: &str,
    page: usize,
) -> Result<serde_json::Value> {
    for retry in 0..=2 {
        wait_for_review_slot().await;
        let response = http
            .post("https://www.bitget.com/v1/p2p/pub/evaluation/query-evaluation-page")
            .version(reqwest::Version::HTTP_11)
            .header("Origin", "https://www.bitget.com")
            .header("Referer", source_url)
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "encryptUserId": id,
                "evaluationType": "all",
                "pageNo": page,
                "pageSize": 10,
                "languageType": 0
            }))
            .send()
            .await?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS && retry < 2 {
            tokio::time::sleep(Duration::from_secs(2 << retry)).await;
            continue;
        }
        return Ok(response.error_for_status()?.json().await?);
    }
    unreachable!("last rate-limit response is returned as an error")
}

pub fn profile_identity(url: &Url) -> Result<(String, String)> {
    if url.scheme() != "https" || url.host_str() != Some("www.bitget.com") {
        bail!("invalid Bitget profile URL");
    }
    let id = url
        .path()
        .strip_prefix("/p2p-trade/user/")
        .context("invalid Bitget profile path")?;
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        bail!("invalid Bitget user ID");
    }
    Ok((
        format!("https://www.bitget.com/p2p-trade/user/{id}"),
        id.into(),
    ))
}

pub struct InitialReviews {
    pub reviews: Vec<ExternalReview>,
    pub has_more: bool,
}

fn append_page(
    reviews: &mut Vec<ExternalReview>,
    data: &serde_json::Value,
    source_url: &str,
    page: usize,
) -> Result<bool> {
    if data.get("code").and_then(serde_json::Value::as_str) != Some("00000") {
        bail!("Bitget review endpoint returned an error");
    }
    let items = data
        .pointer("/data/dataList")
        .and_then(serde_json::Value::as_array)
        .context("Bitget review data missing")?;
    for (index, item) in items.iter().enumerate() {
        let Some(text) = item
            .get("evaluationContentValue")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let timestamp = item
            .get("createTime")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse::<i64>().ok());
        reviews.push(ExternalReview {
            id: format!("{}-{page}-{index}", timestamp.unwrap_or_default()),
            author: item
                .get("fromUserNickName")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("Anonymous")
                .into(),
            text: text.into(),
            rating: match item
                .get("evaluationType")
                .and_then(serde_json::Value::as_str)
            {
                Some("good") => Some(5),
                Some("bad") => Some(1),
                _ => None,
            },
            created_at: timestamp.and_then(DateTime::from_timestamp_millis),
            url: source_url.into(),
            avatar_url: None,
        });
        if reviews.len() == 100 {
            return Ok(false);
        }
    }
    let total = data
        .pointer("/data/totalCount")
        .and_then(serde_json::Value::as_u64);
    Ok(items.len() == 10 && total.is_none_or(|total| (page as u64) * 10 < total))
}

pub async fn fetch_profile_initial(
    http: &Client,
    source_url: &str,
    id: &str,
) -> Result<InitialReviews> {
    // The Feedback tab returns at most ten entries per request. Serve its
    // first page immediately and collect further pages in the background.
    let data = fetch_page(http, source_url, id, 1).await?;
    let mut reviews = Vec::new();
    let has_more = append_page(&mut reviews, &data, source_url, 1)?;
    Ok(InitialReviews { reviews, has_more })
}

pub async fn fetch_profile_remaining(
    http: &Client,
    source_url: &str,
    id: &str,
    mut reviews: Vec<ExternalReview>,
) -> Result<Vec<ExternalReview>> {
    for page in 2..=100 {
        let data = match fetch_page(http, source_url, id, page).await {
            Ok(data) => data,
            Err(error) if !reviews.is_empty() => {
                tracing::warn!(page, %error, "Bitget review collection paused after a partial result");
                break;
            }
            Err(error) => return Err(error),
        };
        if !append_page(&mut reviews, &data, source_url, page)? {
            break;
        }
    }
    Ok(reviews)
}

pub async fn fetch_profile(
    http: &Client,
    source_url: &str,
    id: &str,
) -> Result<Vec<ExternalReview>> {
    let initial = fetch_profile_initial(http, source_url, id).await?;
    if !initial.has_more {
        return Ok(initial.reviews);
    }
    fetch_profile_remaining(http, source_url, id, initial.reviews).await
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
    #[test]
    fn total_count_controls_pagination_when_has_next_page_is_false() {
        let items = (0..10)
            .map(|index| {
                serde_json::json!({
                    "createTime": (1_790_000_000_000_i64 + index).to_string(),
                    "evaluationContentValue": "Fast trade",
                    "evaluationType": "good",
                    "fromUserNickName": "Trader"
                })
            })
            .collect::<Vec<_>>();
        let page = serde_json::json!({
            "code": "00000",
            "data": {"dataList": items, "totalCount": 230, "hasNextPage": false}
        });
        let mut reviews = Vec::new();
        assert!(super::append_page(
            &mut reviews,
            &page,
            "https://www.bitget.com/p2p-trade/user/example",
            1
        )
        .unwrap());
        assert_eq!(reviews.len(), 10);
    }
}

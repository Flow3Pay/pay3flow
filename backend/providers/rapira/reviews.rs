use crate::external_reviews::ExternalReview;
use anyhow::{bail, Context, Result};
use chrono::{NaiveDateTime, TimeZone, Utc};
use reqwest::{Client, Url};

pub fn profile_identity(url: &Url) -> Result<(String, String)> {
    if url.scheme() != "https"
        || url.host_str() != Some("rapira.net")
        || !matches!(url.path(), "/p2p/profileUser" | "/ru/p2p/profileUser")
    {
        bail!("invalid Rapira profile URL");
    }
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "profileUid")
        .map(|(_, value)| value.to_string())
        .context("Rapira profile ID is missing")?;
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, byte)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
    {
        bail!("invalid Rapira profile ID");
    }
    Ok((
        format!("https://rapira.net/ru/p2p/profileUser?profileUid={id}"),
        id,
    ))
}

pub async fn fetch_profile(
    http: &Client,
    source_url: &str,
    id: &str,
) -> Result<Vec<ExternalReview>> {
    let mut reviews = Vec::new();
    // Rapira's public profile requests this endpoint. shouldHaveComment keeps
    // rating-only records out of the written-review list.
    for page in 1..=100 {
        let data: serde_json::Value = http
            .post("https://api.rapira.net/otc/review/page-query-with-counts")
            .header("Origin", "https://rapira.net")
            .header("Referer", source_url)
            .json(&serde_json::json!({
                "profileUid": id,
                "shouldHaveComment": true,
                "reviewType": null,
                "counterAgentSide": null,
                "isFromMe": false,
                "pageNo": page,
                "pageSize": 10
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if data.get("code").and_then(serde_json::Value::as_i64) != Some(0) {
            bail!("Rapira review endpoint returned an error");
        }
        let items = data
            .pointer("/data/reviews/content")
            .and_then(serde_json::Value::as_array)
            .context("Rapira review data missing")?;
        if items.is_empty() {
            break;
        }
        for item in items {
            let Some(text) = item
                .get("comment")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
            else {
                continue;
            };
            let Some(review_id) = item.get("reviewUid").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let anonymous = item
                .get("isReviewerAnonymous")
                .and_then(serde_json::Value::as_bool)
                == Some(true);
            reviews.push(ExternalReview {
                id: review_id.into(),
                author: if anonymous {
                    "Anonymous".into()
                } else {
                    item.pointer("/counterAgent/username")
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or("Anonymous")
                        .into()
                },
                text: text.into(),
                rating: match item.get("reviewType").and_then(serde_json::Value::as_str) {
                    Some("POSITIVE") => Some(5),
                    Some("NEGATIVE") => Some(1),
                    Some("NEUTRAL") => Some(3),
                    _ => None,
                },
                created_at: item
                    .get("updateTime")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
                    .map(|date| Utc.from_utc_datetime(&date)),
                url: source_url.into(),
                avatar_url: None,
            });
            if reviews.len() == 100 {
                return Ok(reviews);
            }
        }
        if data
            .pointer("/data/reviews/last")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
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

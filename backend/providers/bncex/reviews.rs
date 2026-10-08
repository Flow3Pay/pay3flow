use crate::external_reviews::ExternalReview;
use anyhow::Result;
use reqwest::Client;

pub async fn fetch_source(
    http: &Client,
    source_url: &str,
    key: Option<&str>,
) -> Result<Vec<ExternalReview>> {
    super::skylabs::fetch_source(http, source_url, key).await
}

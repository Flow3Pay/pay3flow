//! gRPC client for the crw search service (step 82).
//!
//! Thin wrapper over the tonic client generated from `crw/proto/search.proto`.
//! `search()` applies a 10 s overall timeout and retries once when the first
//! attempt fails because crw was unreachable.

use std::time::Duration;

use tonic::transport::Channel;

pub mod pb {
    tonic::include_proto!("crw.search");
}

use pb::search_service_client::SearchServiceClient;
use pb::{SearchRequest, SearchResult as ProtoSearchResult};

const SEARCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ATTEMPTS: u32 = 2;

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("crw search: connect failed: {0}")]
    Connect(String),
    #[error("crw search: rpc failed: {0}")]
    Rpc(String),
    #[error("crw search: timed out after {0:?}")]
    Timeout(Duration),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub position: u32,
    pub engine: Option<String>,
}

impl From<ProtoSearchResult> for SearchResult {
    fn from(r: ProtoSearchResult) -> Self {
        Self {
            url: r.url,
            title: r.title,
            snippet: if !r.snippet.is_empty() { r.snippet } else { r.description },
            position: r.position,
            engine: r.engine,
        }
    }
}

fn is_connect_failure(err: &SearchError) -> bool {
    matches!(err, SearchError::Connect(_))
}

/// A connectable search client. The tonic channel is created lazily per `search`
/// call so a crw that restarts between calls is reconnected automatically.
#[derive(Debug, Clone)]
pub struct CrwClient {
    endpoint: String,
}

impl CrwClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    async fn connect(&self) -> Result<SearchServiceClient<Channel>, SearchError> {
        let conn = Channel::from_shared(self.endpoint.clone())
            .map_err(|e| SearchError::Connect(e.to_string()))?
            .timeout(SEARCH_TIMEOUT)
            .connect_lazy();
        Ok(SearchServiceClient::new(conn))
    }

    /// Search crw for `query` and return up to `max_results` results.
    ///
    /// Applies a 10 s overall timeout and retries once when the initial attempt
    /// could not reach crw (connection-level failure).
    pub async fn search(
        &self,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let req = SearchRequest {
            query: query.to_string(),
            limit: Some(max_results),
            language: None,
            engines: None,
        };

        for attempt in 1..=MAX_ATTEMPTS {
            let mut client = self.connect().await?;
            let fut = client.search(req.clone());

            match tokio::time::timeout(SEARCH_TIMEOUT, fut).await {
                Ok(Ok(resp)) => {
                    let results = resp.into_inner().results.into_iter().map(Into::into).collect();
                    return Ok(results);
                }
                Ok(Err(e)) => {
                    let err = SearchError::Rpc(e.to_string());
                    if attempt < MAX_ATTEMPTS && is_connect_failure(&err) {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                    return Err(err);
                }
                Err(_) => return Err(SearchError::Timeout(SEARCH_TIMEOUT)),
            }
        }

        unreachable!("search loop must return before exhausting attempts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_failure_is_recognized() {
        assert!(is_connect_failure(&SearchError::Connect("boom".into())));
        assert!(!is_connect_failure(&SearchError::Rpc("boom".into())));
        assert!(!is_connect_failure(&SearchError::Timeout(SEARCH_TIMEOUT)));
    }
}
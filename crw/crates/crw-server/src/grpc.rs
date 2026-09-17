//! gRPC search service for CRW.
//!
//! Exposes a minimal [`SearchService`] over gRPC (port 3031 by default)
//! alongside the HTTP API. The handler delegates to the same
//! `routes::search::search_inner` logic that powers `POST /v1/search`,
//! so gRPC and HTTP stay behaviorally identical.

use crate::state::AppState;
use crw_core::types::{SearchData, SearchRequest, SearchResult};

pub mod pb {
    tonic::include_proto!("crw.search");
}

use pb::acquirer_discovery_service_server::{
    AcquirerDiscoveryService, AcquirerDiscoveryServiceServer,
};
use pb::search_service_server::{SearchService, SearchServiceServer};
use pb::{
    Acquirer as GrpcAcquirer, AcquirerDiff, DiscoverAcquirersRequest,
    SearchRequest as GrpcSearchRequest, SearchResponse as GrpcSearchResponse,
    SearchResult as GrpcSearchResult,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

/// Concrete gRPC service implementation.
pub struct CrwGrpcService {
    state: Arc<AppState>,
}

impl CrwGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Return the tonic server object ready to be bound to a listener.
    pub fn service(state: Arc<AppState>) -> SearchServiceServer<Self> {
        SearchServiceServer::new(Self::new(state))
    }
}

fn map_result(r: &SearchResult) -> GrpcSearchResult {
    GrpcSearchResult {
        url: r.url.clone(),
        title: r.title.clone(),
        description: r.description.clone(),
        snippet: r.snippet.clone(),
        position: r.position,
        score: r.score,
        published_date: r.published_date.clone(),
        category: r.category.clone(),
        engine: None,
    }
}

#[tonic::async_trait]
impl SearchService for CrwGrpcService {
    async fn search(
        &self,
        req: Request<GrpcSearchRequest>,
    ) -> Result<Response<GrpcSearchResponse>, Status> {
        let req = req.into_inner();

        let crw_req = SearchRequest {
            query: req.query.clone(),
            limit: req.limit,
            lang: req.language,
            sources: None,
            categories: None,
            scrape_options: None,
            summarize_results: None,
            answer: None,
            answer_top_n: None,
            max_chars_per_source: None,
            llm_api_key: None,
            llm_provider: None,
            llm_model: None,
            base_url: None,
            summary_prompt: None,
            answer_prompt: None,
            answer_temperature: None,
            query_expand_variants: None,
            query_expand: None,
            multi_round: None,
            snippet_first: None,
            answer_list_format: None,
            max_content_chars: None,
            tbs: None,
            paid_rescue: false,
        };

        let resp = crate::routes::search::search_inner(&self.state, crw_req)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let data = resp.data.ok_or_else(|| Status::internal("no data"))?;

        let mut results = Vec::new();
        match data.results {
            SearchData::Flat(rows) => {
                results = rows.iter().map(map_result).collect();
            }
            SearchData::Grouped(g) => {
                if let Some(web) = g.web.as_ref() {
                    results = web.iter().map(map_result).collect();
                }
            }
        }

        Ok(Response::new(GrpcSearchResponse {
            query: req.query,
            number_of_results: results.len() as u64,
            results,
            suggestions: Vec::new(),
            degraded: false,
        }))
    }
}

/// gRPC implementation of the AcquirerDiscoveryService. Delegates to
/// `crate::acquirer::discover`, which runs the internet scan AND computes the
/// diff against the backend database before coming back.
pub struct CrwAcquirerDiscovery {
    state: Arc<AppState>,
}

impl CrwAcquirerDiscovery {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Return the tonic server object ready to be bound to a listener.
    pub fn service(state: Arc<AppState>) -> AcquirerDiscoveryServiceServer<Self> {
        AcquirerDiscoveryServiceServer::new(Self::new(state))
    }
}

fn to_grpc_acquirer(a: &crate::acquirer::DiscoveredAcquirer) -> GrpcAcquirer {
    GrpcAcquirer {
        slug: a.slug.clone(),
        name: a.name.clone(),
        geo: a.geo.clone(),
        currencies: a.currencies.clone(),
        fee_percent: a.fee_percent,
        fee_fixed: a.fee_fixed,
        min_amount: a.min_amount,
        max_amount: a.max_amount,
        amount_currency: a.amount_currency.clone(),
        website_url: a.website_url.clone(),
        api_docs_url: a.api_docs_url.clone(),
        description: a.description.clone(),
        source: a.source.clone(),
        api_json: a.api_json.clone(),
    }
}

#[tonic::async_trait]
impl AcquirerDiscoveryService for CrwAcquirerDiscovery {
    async fn discover_acquirers(
        &self,
        req: Request<DiscoverAcquirersRequest>,
    ) -> Result<Response<AcquirerDiff>, Status> {
        let target = req.into_inner().target_count.unwrap_or(500);
        let start = std::time::Instant::now();
        let result = crate::acquirer::discover(&self.state, target).await;

        if result.search_disabled {
            return Err(Status::unavailable(
                "search disabled: set CRW_SEARCH__SEARXNG_URL to point at a search backend",
            ));
        }
        if let Some(err) = &result.error {
            tracing::warn!(
                error = %err,
                "acquirer discovery: DB sync unavailable; treating scan as adds"
            );
        }

        let elapsed_ms = start.elapsed().as_millis();
        tracing::info!(
            found = result.found,
            added = result.added.len(),
            updated = result.updated.len(),
            deleted = result.deleted.len(),
            elapsed_ms,
            "acquirer discovery scan complete"
        );

        Ok(Response::new(AcquirerDiff {
            found: result.found as u32,
            added: result.added.iter().map(to_grpc_acquirer).collect(),
            updated: result.updated.iter().map(to_grpc_acquirer).collect(),
            deleted: result.deleted.clone(),
            scanned_at: result.scanned_at,
        }))
    }
}
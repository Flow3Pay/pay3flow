use std::sync::Arc;

use futures::StreamExt;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::activitypub::actor::ActorIdentity;
use crate::activitypub::delivery::{DeliveryClient, DeliveryOutcome};
use crate::activitypub::error::ActivityPubError;
use crate::activitypub::fep8fba;
use crate::activitypub::model::{request_proposal, AcquirerCandidate};
use crate::db::DbPool;
use crate::route_engine::RouteCapability;

/// Centralized state for the ActivityPub module.
#[derive(Clone)]
pub struct Service {
    pub identity: ActorIdentity,
    pub delivery: DeliveryClient,
    pub require_signatures: bool,
    pub fmatch_inbox: String,
    pub fmatch_actor_id: String,
    pub marketplace_resource: String,
    pub origin: String,
    latest_candidates: Arc<RwLock<Vec<AcquirerCandidate>>>,
}

impl Service {
    pub fn new(
        pool: DbPool,
        identity: ActorIdentity,
        require_signatures: bool,
        fmatch_inbox: String,
        fmatch_actor_id: String,
        marketplace_resource: String,
        origin: String,
    ) -> Self {
        Self {
            identity,
            delivery: DeliveryClient::new(pool),
            require_signatures,
            fmatch_inbox,
            fmatch_actor_id,
            marketplace_resource,
            origin,
            latest_candidates: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if the given keyId belongs to our own actor.
    pub fn is_local_activitypub_actor(&self, key_id: &str) -> bool {
        key_id.starts_with(&self.origin) && key_id.contains("#")
    }

    /// Store candidates received from fmatch (called from inbox handler).
    pub async fn received_candidates(&self, candidates: &[AcquirerCandidate]) {
        let mut guard = self.latest_candidates.write().await;
        guard.extend(candidates.iter().cloned());
    }

    /// Snapshot the most recently received candidates.
    pub async fn latest_candidates(&self) -> Vec<AcquirerCandidate> {
        self.latest_candidates.read().await.clone()
    }

    /// Clear the candidate list (e.g. for re-fetching).
    pub async fn clear_candidates(&self) {
        self.latest_candidates.write().await.clear();
    }

    /// Submit a fep/0837 request proposal into the fmatch inbox and return the
    /// inline response (fmatch answers discovery commands with candidate lists).
    pub async fn submit_request(
        &self,
        command: &str,
        content: &str,
    ) -> Result<(DeliveryOutcome, Option<Value>), ActivityPubError> {
        let id = format!("{}/requests/{}", self.origin, uuid::Uuid::new_v4());
        let activity = request_proposal(
            &id,
            &self.identity.actor_id,
            &self.marketplace_resource,
            command,
            content,
            &format!("{}/inbox", self.origin),
        );
        self.delivery
            .deliver_with_response(&self.identity, &self.fmatch_inbox, &activity)
            .await
    }

    /// Advertise the exchange capability to the configured Fmatch actor.
    pub async fn publish_exchange_proposal(&self) -> Result<DeliveryOutcome, ActivityPubError> {
        let activity = fep8fba::create_exchange_proposal(
            &self.origin,
            &self.identity.actor_id,
            &self.fmatch_actor_id,
        );
        self.delivery
            .deliver(&self.identity, &self.fmatch_inbox, &activity)
            .await
    }

    /// Publish one stable route capability to fmatch.
    ///
    /// The proposal contains topology and capability metadata only.  It never
    /// contains a live price; fmatch must request a private Pay3Flow quote
    /// after matching this capability.  Reusing the same proposal id makes
    /// publication idempotent and lets fmatch upsert a refreshed capability.
    pub async fn publish_route_capability(
        &self,
        route: &RouteCapability,
    ) -> Result<DeliveryOutcome, ActivityPubError> {
        let route_slug = route.id.replace(':', "-");
        let proposal_id = format!(
            "{}/marketplace/routes/{}",
            self.origin.trim_end_matches('/'),
            route_slug
        );
        let status = if route.enabled { "enabled" } else { "disabled" };
        let path = route
            .path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ");
        let content = format!(
            "route_id={}; from={}; to={}; path={}; capabilities={}; status={}; quote=internal",
            route.id,
            route.from,
            route.to,
            path,
            route.capabilities.join(","),
            status
        );
        let proposal = crate::activitypub::model::Proposal {
            id: proposal_id,
            purpose: "offer".into(),
            attributed_to: self.identity.actor_id.clone(),
            name: format!("Pay3Flow route {}", route.id),
            content,
            resource_conforms_to: format!(
                "{}/marketplace/resources/exchange",
                self.origin.trim_end_matches('/')
            ),
            action: "deliverService".into(),
            resource_unit: "route".into(),
            attachments: vec![serde_json::json!({
                "type": "PropertyValue",
                "name": "pay3flow:routeCapability",
                "value": route,
            })],
        };
        self.delivery
            .deliver(&self.identity, &self.fmatch_inbox, &proposal.to_activity())
            .await
    }

    /// Reconcile and publish capabilities, including disabled tombstones for
    /// routes that disappeared during the latest provider refresh.
    pub async fn publish_route_reconciliation(
        &self,
        reconciliation: &crate::route_engine::RouteReconciliation,
    ) -> Vec<Result<DeliveryOutcome, ActivityPubError>> {
        reconciliation
            .upsert
            .iter()
            .chain(reconciliation.disable.iter())
            .map(|route| self.publish_route_capability(route))
            .collect::<futures::stream::FuturesUnordered<_>>()
            .collect()
            .await
    }
}

use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::activitypub::actor::ActorIdentity;
use crate::activitypub::delivery::{DeliveryClient, DeliveryOutcome};
use crate::activitypub::error::ActivityPubError;
use crate::activitypub::model::{request_proposal, AcquirerCandidate, Proposal};
use crate::db::DbPool;

/// A solver's mock answer, recorded when our inbox answers a delivered Ticket.
/// Persisted only in memory: the smoke test has no real payment backend yet.
#[derive(Debug, Clone, Serialize)]
pub struct MockPayment {
    pub id: String,
    pub task_ref: String,
    pub execution_id: String,
    pub provider: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

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
    mock_payments: Arc<RwLock<Vec<MockPayment>>>,
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
            mock_payments: Arc::new(RwLock::new(Vec::new())),
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

    /// The offer that makes our own actor a solver in the fmatch marketplace,
    /// so fmatch can also pick "us" as the best route for a task.
    pub fn self_offer_activity(&self) -> Value {
        let id = format!("{}/acquirers/backend-solver/offer", self.identity.actor_id);
        let proposal = Proposal {
            id,
            purpose: "offer".into(),
            attributed_to: self.identity.actor_id.clone(),
            name: "Flow3 Backend Solver (quality 1.0, latency 1ms)".into(),
            content: "acquirer=pay3flow-backend; status=active; geo=Global; currencies=USD|EUR|BYN; fee=0.5% flat; limits=min 1, max 1000000 USD".into(),
            resource_conforms_to: self.marketplace_resource.clone(),
            action: "deliverService".into(),
            resource_unit: "one".into(),
            attachments: vec![
                json!({ "type": "PropertyValue", "name": "inbox", "value": format!("{}/inbox", self.origin) }),
                json!({ "type": "PropertyValue", "name": "provider", "value": "pay3flow-backend" }),
                json!({ "type": "PropertyValue", "name": "model", "value": "mock" }),
                json!({ "type": "PropertyValue", "name": "qualityScore", "value": 1.0 }),
                json!({ "type": "PropertyValue", "name": "latencyMs", "value": 1_u64 }),
                json!({ "type": "PropertyValue", "name": "capacity", "value": 1000_u64 }),
                json!({ "type": "PropertyValue", "name": "successCount", "value": 100_u64 }),
                json!({ "type": "PropertyValue", "name": "failureCount", "value": 0_u64 }),
            ],
        };
        proposal.to_activity()
    }

    /// Record a mock payment from a solved ticket. Idempotent per execution id,
    /// so fmatch retries / double delivery never double-book a payment.
    pub async fn record_mock_payment(&self, payment: MockPayment) {
        let mut guard = self.mock_payments.write().await;
        if guard.iter().any(|p| p.execution_id == payment.execution_id) {
            return;
        }
        guard.push(payment);
    }

    /// Snapshot of all mock payments recorded so far.
    pub async fn mock_payments(&self) -> Vec<MockPayment> {
        self.mock_payments.read().await.clone()
    }
}

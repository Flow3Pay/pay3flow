use std::sync::Arc;

use tokio::sync::RwLock;

use crate::activitypub::actor::ActorIdentity;
use crate::activitypub::delivery::DeliveryClient;
use crate::activitypub::model::AcquirerCandidate;
use crate::db::DbPool;

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
}
pub mod stub;

use anyhow::Result;
use async_trait::async_trait;

use crate::payments::model::Minor;

/// Execution layer contract (PLAN #38): the bounded surface every acquirer
/// adapter implements so the routing + service layer never depends on a
/// concrete provider.
///
/// Providers are resolved by `slug` — the same slug the routing layer stores on
/// a route (`acquirer_slug`). The stub provider (PLAN #39) implements this
/// contract deterministically for offline/CI and for e2e smoke tests.
#[async_trait]
pub trait AcquireProvider: Send + Sync {
    /// Registry key, matched against `route.acquirer_slug` / `transaction.provider`.
    fn slug(&self) -> &str;

    /// Execute a payment at the provider.
    async fn execute(&self, request: &ExecuteRequest) -> Result<ExecuteOutcome>;

    /// Poll a previously-submitted payment's state by provider id.
    async fn status(&self, external_id: &str) -> Result<AcquireResult>;
}

/// Everything the provider needs to actually move money.
#[derive(Debug, Clone)]
pub struct ExecuteRequest {
    /// Amount in minor units of `currency`.
    pub amount: Minor,
    pub currency: String,
    pub from_account: String,
    pub to_account: String,
    pub method: Option<String>,
    /// Opaque string we hand over and get back as `external_id` (correlation).
    pub provider_ref: String,
}

/// Result of an execution attempt. Providers decide success synchronously for
/// the stub; real adapters can answer `Pending` and settle later via webhooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteOutcome {
    pub external_id: String,
    /// Provider-driven status (may still be executing).
    pub status: AcquireResult,
}

/// Mirror of the provider-side state machine (subset of transaction statuses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireResult {
    Succeeded,
    Pending,
    Failed,
}

// Routing/execution registry: a shared bag of providers keyed by slug.
#[derive(Default, Clone)]
pub struct ProviderRegistry {
    providers: std::sync::Arc<Vec<Box<dyn AcquireProvider>>>,
}

impl ProviderRegistry {
    pub fn from_providers(providers: Vec<Box<dyn AcquireProvider>>) -> Self {
        Self {
            providers: std::sync::Arc::new(providers),
        }
    }

    pub fn get(&self, slug: &str) -> Option<&dyn AcquireProvider> {
        self.providers
            .iter()
            .find(|p| p.slug() == slug)
            .map(|p| p.as_ref())
    }

    pub fn slugs(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.slug()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_by_slug() {
        let registry = ProviderRegistry::from_providers(vec![Box::new(stub::StubProvider::new())]);
        assert!(registry.get("stub").is_some());
        assert!(registry.get("stripe").is_none());
        assert_eq!(registry.slugs(), vec!["stub"]);
    }
}

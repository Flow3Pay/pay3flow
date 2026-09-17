use anyhow::{anyhow, Result};
use async_trait::async_trait;

use super::{AcquireProvider, AcquireResult, ExecuteOutcome, ExecuteRequest};

/// Deterministic test/offline provider (PLAN #39).
///
/// The outcome is driven by the destination account so smoke tests can script
/// every branch without network:
/// * `to` starts with `stub-fail:` → `Failed`
/// * `to` starts with `stub-hold:` → `Pending` (stays executing on the record)
/// * otherwise → `Succeeded`, external_id = `stub-<provider_ref>`
pub struct StubProvider;

impl Default for StubProvider {
    fn default() -> Self {
        Self
    }
}

impl StubProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AcquireProvider for StubProvider {
    fn slug(&self) -> &str {
        "stub"
    }

    async fn execute(&self, request: &ExecuteRequest) -> Result<ExecuteOutcome> {
        let to = request.to_account.trim();
        let status = if to.starts_with("stub-fail:") {
            AcquireResult::Failed
        } else if to.starts_with("stub-hold:") {
            AcquireResult::Pending
        } else {
            AcquireResult::Succeeded
        };
        Ok(ExecuteOutcome {
            external_id: format!("stub-{}", request.provider_ref),
            status,
        })
    }

    async fn status(&self, external_id: &str) -> Result<AcquireResult> {
        Err(anyhow!("stub status({external_id}): execution is synchronous, no async status"))
    }
}
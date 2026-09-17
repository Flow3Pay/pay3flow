use serde::{Deserialize, Serialize};

/// Transaction lifecycle statuses (PLAN #32, #35).
///
/// Transitions are forward-only:
/// ```text
/// pending -> matched -> executing -> done
///                            \-> failed
/// ```
/// `matched -> failed` is allowed when routing/execution fails before a
/// terminal acquirer state is reached (e.g. provider rejected the payment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Matched,
    Executing,
    Done,
    Failed,
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Pending => "pending",
            TransactionStatus::Matched => "matched",
            TransactionStatus::Executing => "executing",
            TransactionStatus::Done => "done",
            TransactionStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "matched" => Some(Self::Matched),
            "executing" => Some(Self::Executing),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// Whether `self` may move to `next` in a single step.
    pub fn can_transition(&self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Matched)
                | (Self::Pending, Self::Failed)
                | (Self::Matched, Self::Executing)
                | (Self::Matched, Self::Failed)
                | (Self::Executing, Self::Done)
                | (Self::Executing, Self::Failed)
        )
    }

    /// Terminal statuses are immutable.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed)
    }
}

/// Route status mirrors the transaction lifecycle but tracks the acquirer leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteStatus {
    Pending,
    Matched,
    Executing,
    Done,
    Failed,
}

impl RouteStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteStatus::Pending => "pending",
            RouteStatus::Matched => "matched",
            RouteStatus::Executing => "executing",
            RouteStatus::Done => "done",
            RouteStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "matched" => Some(Self::Matched),
            "executing" => Some(Self::Executing),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions() {
        assert!(TransactionStatus::Pending.can_transition(TransactionStatus::Matched));
        assert!(TransactionStatus::Matched.can_transition(TransactionStatus::Executing));
        assert!(TransactionStatus::Executing.can_transition(TransactionStatus::Done));
    }

    #[test]
    fn rollback_transitions_rejected() {
        assert!(!TransactionStatus::Matched.can_transition(TransactionStatus::Pending));
        assert!(!TransactionStatus::Done.can_transition(TransactionStatus::Executing));
        assert!(!TransactionStatus::Failed.can_transition(TransactionStatus::Pending));
    }

    #[test]
    fn failed_reachable_from_matched_and_executing() {
        assert!(TransactionStatus::Matched.can_transition(TransactionStatus::Failed));
        assert!(TransactionStatus::Executing.can_transition(TransactionStatus::Failed));
    }

    #[test]
    fn terminal_is_immutable() {
        assert!(TransactionStatus::Done.is_terminal());
        assert!(TransactionStatus::Failed.is_terminal());
        assert!(!TransactionStatus::Pending.is_terminal());
    }

    #[test]
    fn parses_known_and_rejects_unknown() {
        assert_eq!(TransactionStatus::parse("pending"), Some(TransactionStatus::Pending));
        assert_eq!(TransactionStatus::parse("done"), Some(TransactionStatus::Done));
        assert!(TransactionStatus::parse("inflight").is_none());
    }
}
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Created,
    Discovering,
    Quoting,
    Quoted,
    Locked,
    TokenSettling,
    MoneySettling,
    ProofPending,
    Done,
    Failed,
    Expired,
    Cancelled,
    Disputed,
}

impl OrderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Discovering => "discovering",
            Self::Quoting => "quoting",
            Self::Quoted => "quoted",
            Self::Locked => "locked",
            Self::TokenSettling => "token_settling",
            Self::MoneySettling => "money_settling",
            Self::ProofPending => "proof_pending",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
            Self::Disputed => "disputed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "created" => Some(Self::Created),
            "discovering" => Some(Self::Discovering),
            "quoting" => Some(Self::Quoting),
            "quoted" => Some(Self::Quoted),
            "locked" => Some(Self::Locked),
            "token_settling" => Some(Self::TokenSettling),
            "money_settling" => Some(Self::MoneySettling),
            "proof_pending" => Some(Self::ProofPending),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            "disputed" => Some(Self::Disputed),
            _ => None,
        }
    }

    pub fn can_transition(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Discovering)
                | (Self::Discovering, Self::Quoting)
                | (Self::Discovering, Self::Failed)
                | (Self::Quoting, Self::Quoted)
                | (Self::Quoting, Self::Failed)
                | (Self::Quoting, Self::Expired)
                | (Self::Quoted, Self::Locked)
                | (Self::Quoted, Self::Expired)
                | (Self::Quoted, Self::Cancelled)
                | (Self::Locked, Self::TokenSettling)
                | (Self::TokenSettling, Self::MoneySettling)
                | (Self::TokenSettling, Self::Disputed)
                | (Self::TokenSettling, Self::Failed)
                | (Self::MoneySettling, Self::ProofPending)
                | (Self::MoneySettling, Self::Disputed)
                | (Self::MoneySettling, Self::Failed)
                | (Self::ProofPending, Self::Done)
                | (Self::ProofPending, Self::Disputed)
                | (Self::ProofPending, Self::Failed)
                | (Self::Disputed, Self::Done)
                | (Self::Disputed, Self::Failed)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Expired | Self::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStatus {
    Received,
    Valid,
    Invalid,
    Selected,
    Expired,
    Rejected,
}

impl QuoteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Selected => "selected",
            Self::Expired => "expired",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "received" => Some(Self::Received),
            "valid" => Some(Self::Valid),
            "invalid" => Some(Self::Invalid),
            "selected" => Some(Self::Selected),
            "expired" => Some(Self::Expired),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverStatus {
    Discovered,
    Active,
    Paused,
    Broken,
    Blocked,
}

impl SolverStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Broken => "broken",
            Self::Blocked => "blocked",
        }
    }

    pub fn accepts_quotes(self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "discovered" => Some(Self::Discovered),
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "broken" => Some(Self::Broken),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    Created,
    FundingPending,
    TokenSettling,
    MoneySettling,
    ProofPending,
    Done,
    Failed,
    Disputed,
}

impl SettlementStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::FundingPending => "funding_pending",
            Self::TokenSettling => "token_settling",
            Self::MoneySettling => "money_settling",
            Self::ProofPending => "proof_pending",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Disputed => "disputed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "created" => Some(Self::Created),
            "funding_pending" => Some(Self::FundingPending),
            "token_settling" => Some(Self::TokenSettling),
            "money_settling" => Some(Self::MoneySettling),
            "proof_pending" => Some(Self::ProofPending),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "disputed" => Some(Self::Disputed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegStatus {
    NotStarted,
    Pending,
    Done,
    Failed,
    Disputed,
}

impl LegStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Disputed => "disputed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "not_started" => Some(Self::NotStarted),
            "pending" => Some(Self::Pending),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "disputed" => Some(Self::Disputed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FundingInstructionStatus {
    NotStarted,
    Created,
    ShownToUser,
    UserConfirmed,
    SolverAcknowledged,
    ReceivedBySolver,
    Expired,
    Cancelled,
    Failed,
}

impl FundingInstructionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Created => "created",
            Self::ShownToUser => "shown_to_user",
            Self::UserConfirmed => "user_confirmed",
            Self::SolverAcknowledged => "solver_acknowledged",
            Self::ReceivedBySolver => "received_by_solver",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "not_started" => Some(Self::NotStarted),
            "created" => Some(Self::Created),
            "shown_to_user" => Some(Self::ShownToUser),
            "user_confirmed" => Some(Self::UserConfirmed),
            "solver_acknowledged" => Some(Self::SolverAcknowledged),
            "received_by_solver" => Some(Self::ReceivedBySolver),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn can_transition(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::NotStarted, Self::Created)
                | (Self::Created, Self::ShownToUser)
                | (Self::Created, Self::Expired)
                | (Self::Created, Self::Cancelled)
                | (Self::ShownToUser, Self::UserConfirmed)
                | (Self::ShownToUser, Self::Expired)
                | (Self::ShownToUser, Self::Cancelled)
                | (Self::UserConfirmed, Self::SolverAcknowledged)
                | (Self::UserConfirmed, Self::Failed)
                | (Self::SolverAcknowledged, Self::ReceivedBySolver)
                | (Self::SolverAcknowledged, Self::Failed)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::ReceivedBySolver | Self::Expired | Self::Cancelled | Self::Failed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofVerificationStatus {
    Pending,
    Verified,
    Rejected,
}

impl ProofVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "verified" => Some(Self::Verified),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_allows_happy_path() {
        let path = [
            OrderStatus::Created,
            OrderStatus::Discovering,
            OrderStatus::Quoting,
            OrderStatus::Quoted,
            OrderStatus::Locked,
            OrderStatus::TokenSettling,
            OrderStatus::MoneySettling,
            OrderStatus::ProofPending,
            OrderStatus::Done,
        ];

        for pair in path.windows(2) {
            assert!(
                pair[0].can_transition(pair[1]),
                "expected {:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn order_rejects_skips_and_terminal_mutation() {
        assert!(!OrderStatus::Created.can_transition(OrderStatus::Done));
        assert!(!OrderStatus::Quoted.can_transition(OrderStatus::Done));
        assert!(!OrderStatus::Locked.can_transition(OrderStatus::Quoted));
        assert!(!OrderStatus::Done.can_transition(OrderStatus::Failed));
        assert!(OrderStatus::Done.is_terminal());
        assert!(OrderStatus::Failed.is_terminal());
        assert!(OrderStatus::Expired.is_terminal());
        assert!(OrderStatus::Cancelled.is_terminal());
        assert!(!OrderStatus::Disputed.is_terminal());
    }

    #[test]
    fn order_allows_failure_and_dispute_branches() {
        let allowed = [
            (OrderStatus::Discovering, OrderStatus::Failed),
            (OrderStatus::Quoting, OrderStatus::Failed),
            (OrderStatus::Quoting, OrderStatus::Expired),
            (OrderStatus::Quoted, OrderStatus::Cancelled),
            (OrderStatus::TokenSettling, OrderStatus::Disputed),
            (OrderStatus::MoneySettling, OrderStatus::Disputed),
            (OrderStatus::ProofPending, OrderStatus::Disputed),
            (OrderStatus::Disputed, OrderStatus::Done),
            (OrderStatus::Disputed, OrderStatus::Failed),
        ];

        for (from, to) in allowed {
            assert!(from.can_transition(to), "expected {from:?} -> {to:?}");
        }
    }

    #[test]
    fn funding_instruction_requires_user_confirmation_before_solver_ack() {
        assert!(
            FundingInstructionStatus::NotStarted.can_transition(FundingInstructionStatus::Created)
        );
        assert!(
            FundingInstructionStatus::Created.can_transition(FundingInstructionStatus::ShownToUser)
        );
        assert!(FundingInstructionStatus::ShownToUser
            .can_transition(FundingInstructionStatus::UserConfirmed));
        assert!(FundingInstructionStatus::UserConfirmed
            .can_transition(FundingInstructionStatus::SolverAcknowledged));
        assert!(!FundingInstructionStatus::NotStarted
            .can_transition(FundingInstructionStatus::SolverAcknowledged));
        assert!(!FundingInstructionStatus::Created
            .can_transition(FundingInstructionStatus::SolverAcknowledged));
        assert!(!FundingInstructionStatus::ShownToUser
            .can_transition(FundingInstructionStatus::SolverAcknowledged));
    }

    #[test]
    fn parses_order_status_strings() {
        assert_eq!(OrderStatus::parse("created"), Some(OrderStatus::Created));
        assert_eq!(
            OrderStatus::parse("token_settling"),
            Some(OrderStatus::TokenSettling)
        );
        assert!(OrderStatus::parse("pending").is_none());
    }

    #[test]
    fn solver_must_be_active_to_quote() {
        assert!(SolverStatus::Active.accepts_quotes());
        assert!(!SolverStatus::Discovered.accepts_quotes());
        assert!(!SolverStatus::Paused.accepts_quotes());
        assert!(!SolverStatus::Broken.accepts_quotes());
        assert!(!SolverStatus::Blocked.accepts_quotes());
    }
}

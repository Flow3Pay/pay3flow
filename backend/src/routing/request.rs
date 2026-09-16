use serde::{Deserialize, Serialize};

/// A payment task submitted for matching: "send N <currency> from <from> to <to>".
///
/// Mirrors the structured content of the fmatch `Proposal(purpose="request")`:
/// amount/currency plus source, destination, optional target territory and method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub amount: f64,
    pub currency: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub to_geo: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

impl PaymentRequest {
    pub fn new(
        amount: f64,
        currency: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        Self {
            amount,
            currency: currency.into(),
            from: from.into(),
            to: to.into(),
            to_geo: None,
            method: None,
        }
    }
}

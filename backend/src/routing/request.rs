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
    /// Destination currency of the swap pair, when the payment converts
    /// (e.g. "USD" for a EUR→USD exchange). Optional: not set = send the
    /// same currency.
    #[serde(default)]
    pub to_currency: Option<String>,
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
            to_currency: None,
        }
    }

    pub fn with_to_currency(mut self, to_currency: impl Into<String>) -> Self {
        self.to_currency = Some(to_currency.into());
        self
    }
}

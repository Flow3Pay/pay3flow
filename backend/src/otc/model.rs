use super::error::{invalid, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    Buy,
    Sell,
}
impl Direction {
    pub fn input_chain(self) -> &'static str {
        match self {
            Self::Buy => "ethereum",
            Self::Sell => "everscale",
        }
    }
    pub fn output_chain(self) -> &'static str {
        match self {
            Self::Buy => "everscale",
            Self::Sell => "ethereum",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Terms {
    pub demo: bool,
    pub direction: Direction,
    pub input: String,
    pub output: String,
    pub input_units: String,
    pub output_units: String,
    pub customer_actor: String,
    pub desk_actor: String,
    pub customer_ever: String,
    pub customer_usdt: String,
    pub desk_ever: String,
    pub desk_usdt: String,
    pub quote_by: DateTime<Utc>,
    pub pay_by: DateTime<Utc>,
    pub payout_by: DateTime<Utc>,
    pub network_costs: String,
    pub ever_receiving_cost_units: String,
    pub refund_policy: String,
    pub fee_policy: FeePolicy,
    pub proposal: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FeePolicy {
    pub version: String,
    pub rate: String,
    pub floor: String,
    pub basis_units: String,
    pub fee_units: String,
    pub precision: u32,
}
impl Terms {
    pub fn leg(&self, kind: &str) -> Result<Leg> {
        let (chain, amount, sender, recipient) = match (self.direction, kind) {
            (Direction::Buy, "payment") => (
                "ethereum",
                &self.input_units,
                &self.customer_usdt,
                &self.desk_usdt,
            ),
            (Direction::Buy, "payout") => (
                "everscale",
                &self.output_units,
                &self.desk_ever,
                &self.customer_ever,
            ),
            (Direction::Sell, "payment") => (
                "everscale",
                &self.input_units,
                &self.customer_ever,
                &self.desk_ever,
            ),
            (Direction::Sell, "payout") => (
                "ethereum",
                &self.output_units,
                &self.desk_usdt,
                &self.customer_usdt,
            ),
            (_, "refund") => {
                let leg = self.leg("payment")?;
                return Ok(Leg {
                    sender: leg.recipient,
                    recipient: leg.sender,
                    ..leg
                });
            }
            _ => return Err(invalid("invalid transfer kind")),
        };
        if sender == recipient {
            return Err(invalid("customer and desk must use distinct wallets"));
        }
        let transfer_amount = if chain == "everscale" {
            super::amount::integer(amount)?
                .checked_add(super::amount::integer(&self.ever_receiving_cost_units)?)
                .ok_or_else(|| invalid("EVER network cost overflow"))?
                .to_string()
        } else {
            amount.clone()
        };
        Ok(Leg {
            chain: chain.into(),
            amount: amount.clone(),
            transfer_amount,
            sender: sender.clone(),
            recipient: recipient.clone(),
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Leg {
    pub chain: String,
    pub amount: String,
    pub transfer_amount: String,
    pub sender: String,
    pub recipient: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Trade {
    pub id: Uuid,
    pub rfq_id: Uuid,
    pub state: String,
    pub terms: Terms,
    pub offer: Option<Value>,
    pub decision: Option<Value>,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Evidence {
    pub chain: String,
    pub identity: String,
    pub leg: Leg,
    pub included_at: DateTime<Utc>,
    pub canonical: bool,
    pub finalized: bool,
    pub successful: bool,
    pub raw: Value,
}

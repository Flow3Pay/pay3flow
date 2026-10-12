//! Four-hour USDT vaults authorized by immutable 2-of-3 trade Safes.
mod abi;
mod observer;
mod service;
#[cfg(test)]
mod tests;

use super::{
    amount,
    error::{invalid, Result},
    model::{Direction, Terms},
    wallet,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const MAX_EVER_UNITS: u128 = 100_000_000_000_000;
pub(crate) const LIFETIME_SECONDS: i64 = 14_400;
pub(crate) const EVER_CUTOFF_SECONDS: i64 = 1_800;

/// A quote snapshots the deployment identity so configuration changes cannot retarget it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct EscrowPolicy {
    pub factory: String,
    pub factory_code_hash: String,
    pub arbitrator: String,
    pub singleton: String,
    pub singleton_code_hash: String,
}
impl EscrowPolicy {
    pub(crate) fn validate(&self) -> Result<()> {
        for value in [&self.factory, &self.arbitrator, &self.singleton] {
            if wallet::address("ethereum", value)? != *value
                || value == &format!("0x{}", "0".repeat(40))
            {
                return Err(invalid(
                    "escrow addresses must be nonzero canonical Ethereum addresses",
                ));
            }
        }
        for value in [&self.factory_code_hash, &self.singleton_code_hash] {
            if !value.starts_with("0x") || wallet::unhex(value)?.len() != 32 {
                return Err(invalid("escrow requires pinned deployment bytecode hashes"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Deployment {
    pub trade_id: String,
    pub terms_hash: String,
    pub buyer: String,
    pub seller: String,
    pub usdt_amount: String,
    pub ever_amount: String,
    pub valid_until: i64,
}
impl Deployment {
    pub(crate) fn from_terms(id: uuid::Uuid, terms: &Terms) -> Result<Self> {
        validate_amounts(terms)?;
        let (buyer, seller) = match terms.direction {
            Direction::Buy => (&terms.customer_usdt, &terms.desk_usdt),
            Direction::Sell => (&terms.desk_usdt, &terms.customer_usdt),
        };
        let policy = terms
            .escrow
            .as_ref()
            .ok_or_else(|| invalid("trade uses direct settlement"))?;
        if buyer == seller || buyer == &policy.arbitrator || seller == &policy.arbitrator {
            return Err(invalid("escrow requires three independent owner addresses"));
        }
        Ok(Self {
            trade_id: format!("{:#x}", alloy_primitives::keccak256(id.as_bytes())),
            terms_hash: format!(
                "{:#x}",
                alloy_primitives::keccak256(serde_json::to_vec(terms)?)
            ),
            buyer: buyer.clone(),
            seller: seller.clone(),
            usdt_amount: terms.fee_policy.basis_units.clone(),
            ever_amount: ever_units(terms).to_string(),
            valid_until: terms.pay_by.timestamp(),
        })
    }
}

pub(crate) fn ever_units(terms: &Terms) -> &str {
    match terms.direction {
        Direction::Buy => &terms.output_units,
        Direction::Sell => &terms.input_units,
    }
}
pub(crate) fn validate_amounts(terms: &Terms) -> Result<()> {
    let ever = amount::integer(ever_units(terms))?;
    if ever == 0 || ever > MAX_EVER_UNITS || amount::integer(&terms.fee_policy.basis_units)? == 0 {
        return Err(invalid(
            "escrow trade must be positive and at most 100,000 EVER",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConsentRequest {
    pub signature: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProposalRequest {
    pub action: String,
    #[serde(default)]
    pub dispute: bool,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub evidence: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApprovalRequest {
    pub proposal_id: uuid::Uuid,
    pub signature: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CallRequest {
    pub action: String,
    pub proposal_id: Option<uuid::Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferenceRequest {
    pub call_id: uuid::Uuid,
    pub reference: String,
}

//! Display-only balance reads for the customer's own verified wallets.
//!
//! These never gate settlement, so they never fail as a whole: an unlinked
//! wallet or a dead RPC reports a labelled gap per chain instead of blanking
//! the portfolio panel or returning an error to the browser.
use super::{amount, chain::number, error::Error, Service};
use serde_json::{json, Value};
use std::future::Future;

const EVER_DECIMALS: u32 = 9;
const USDT_DECIMALS: u32 = 6;

/// `state` is `ok`, `unverified` (no linked wallet yet) or `unavailable`
/// (the chain could not be read). Only `ok` carries an amount.
fn entry(chain: &str, symbol: &str, network: &str, amount: Option<String>, state: &str) -> Value {
    json!({"chain": chain, "symbol": symbol, "network": network,
           "amount": amount, "state": state})
}

impl Service {
    pub(crate) async fn balances(&self, actor: &str) -> Value {
        let network = if self.cfg.ethereum_chain_id == 11155111 {
            "Sepolia"
        } else {
            "Ethereum"
        };
        let ever = self
            .balance("everscale", "EVER", "Everscale", EVER_DECIMALS, async {
                let address = self.wallet(actor, "everscale").await?;
                number(&self.ever_account(&address).await?["balance"])
            })
            .await;
        let usdt = self
            .balance("ethereum", "USDT", network, USDT_DECIMALS, async {
                let address = self.wallet(actor, "ethereum").await?;
                self.erc20_balance_of(&self.desk_evm(), &address, &json!("finalized"))
                    .await
            })
            .await;
        json!({"balances": [ever, usdt]})
    }

    async fn balance(
        &self,
        chain: &'static str,
        symbol: &'static str,
        network: &'static str,
        decimals: u32,
        read: impl Future<Output = Result<u128, Error>>,
    ) -> Value {
        match read.await {
            Ok(units) => entry(
                chain,
                symbol,
                network,
                Some(amount::decimal(units, decimals)),
                "ok",
            ),
            Err(Error::VerificationRequired(_)) => {
                entry(chain, symbol, network, None, "unverified")
            }
            Err(error) => {
                tracing::warn!(%error, chain, "otc.balance_read_failed");
                entry(chain, symbol, network, None, "unavailable")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_chain_precision_is_rendered_independently() {
        assert_eq!(amount::decimal(378_357_000_000, EVER_DECIMALS), "378.357");
        assert_eq!(amount::decimal(15_812_490_000, USDT_DECIMALS), "15812.49");
    }

    #[test]
    fn a_gap_carries_no_amount_for_the_portfolio_to_render() {
        for state in ["unverified", "unavailable"] {
            let gap = entry("everscale", "EVER", "Everscale", None, state);
            assert_eq!(gap["amount"], Value::Null, "{state}");
            assert_eq!(gap["state"], json!(state));
            assert_eq!(gap["symbol"], json!("EVER"));
        }
        let funded = entry("ethereum", "USDT", "Sepolia", Some("15812.49".into()), "ok");
        assert_eq!(funded["amount"], json!("15812.49"));
        assert_eq!(funded["network"], json!("Sepolia"));
    }
}

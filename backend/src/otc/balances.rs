//! Display-only balance reads for the customer's own verified wallets.
//!
//! These never gate settlement, so they never fail as a whole: an unlinked
//! wallet or a dead RPC reports a labelled gap per chain instead of blanking
//! the portfolio panel or returning an error to the browser.
use super::{amount, chain::number, error::Error, Service};
use crate::route_engine::{canonical_network_id, Asset};
use futures::future::BoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use std::future::Future;

const EVER_DECIMALS: u32 = 9;
const STABLECOIN_DECIMALS: u32 = 6;
const STABLECOINS: [&str; 2] = ["USDT", "USDC"];

/// One stablecoin row bound to its wallet network and reader.
struct Read {
    network: String,
    symbol: String,
    units: BoxFuture<'static, Result<u128, Error>>,
}

/// `state` is `ok`, `unverified` (no linked wallet yet) or `unavailable`
/// (the chain could not be read). Only `ok` carries an amount.
fn entry(chain: &str, symbol: &str, network: &str, amount: Option<String>, state: &str) -> Value {
    json!({"chain": chain, "symbol": symbol, "network": network,
           "amount": amount, "state": state})
}

/// Canonical network id to the label the portfolio renders.
fn network_label(network: &str) -> &'static str {
    match network {
        "ethereum" => "Ethereum",
        "optimism" => "Optimism",
        "bnb-smart-chain" => "BNB Smart Chain",
        "gnosis" => "Gnosis",
        "polygon-pos" => "Polygon",
        "avalanche-c" => "Avalanche",
        "arbitrum-one" => "Arbitrum One",
        "base" => "Base",
        "sei" => "Sei",
        "tron" => "TRON",
        "solana" => "Solana",
        "ton" => "TON",
        "near" => "NEAR",
        "sui" => "Sui",
        "aptos" => "Aptos",
        _ => "Ethereum",
    }
}

impl Service {
    pub(crate) async fn balances(&self, actor: &str) -> Value {
        let ever = self
            .balance("everscale", "EVER", "Everscale", EVER_DECIMALS, async {
                let address = self.wallet(actor, "everscale").await?;
                number(&self.ever_account(&address).await?["balance"])
            })
            .await;
        let mut balances = vec![ever];
        for read in self.stablecoin_reads(actor).await {
            balances.push(
                self.balance(
                    "wallet",
                    &read.symbol,
                    network_label(&read.network),
                    STABLECOIN_DECIMALS,
                    read.units,
                )
                .await,
            );
        }
        json!({"balances": balances})
    }

    /// Every stablecoin pair the provider catalog knows on a wallet network the
    /// chain readers cover. A pair with no reader is still listed so the user
    /// sees a labelled gap instead of a wrong zero.
    async fn stablecoin_reads(&self, actor: &str) -> Vec<Read> {
        let Some(provider) = self.near_intents.as_ref() else {
            return self.legacy_reads(actor).await;
        };
        let mut pairs: Vec<(String, String)> = Vec::new();
        for token in provider.supported_tokens().await {
            let symbol = token.symbol.to_ascii_uppercase();
            if !STABLECOINS.contains(&symbol.as_str()) {
                continue;
            }
            let network = canonical_network_id(&token.blockchain);
            if !crate::route_execution::supports_otc_wallet_network(&network)
                || pairs
                    .iter()
                    .any(|(known, known_symbol)| *known == network && *known_symbol == symbol)
            {
                continue;
            }
            pairs.push((network, symbol));
        }
        if pairs.is_empty() {
            return self.legacy_reads(actor).await;
        }
        pairs.sort();
        let mut reads = Vec::with_capacity(pairs.len());
        for (network, symbol) in pairs {
            let contract = if network == "ethereum" {
                Some(
                    if symbol == "USDC" {
                        super::config::USDC
                    } else {
                        super::config::USDT
                    }
                    .to_string(),
                )
            } else {
                provider
                    .unique_token_for_asset(&Asset {
                        symbol: symbol.clone(),
                        location: Some(network.clone()),
                    })
                    .await
                    .ok()
                    .and_then(|token| token.contract_address)
                    .filter(|contract| !contract.trim().is_empty())
            };
            let service = self.clone();
            let holder_actor = actor.to_string();
            let read_network = network.clone();
            let units = async move {
                let contract = contract.ok_or(Error::Unavailable)?;
                service
                    .read_stablecoin(&holder_actor, &read_network, &contract)
                    .await
            }
            .boxed();
            reads.push(Read {
                network,
                symbol,
                units,
            });
        }
        reads
    }

    /// The historical Ethereum-only portfolio, kept when the provider catalog
    /// is not loaded so the panel never collapses to a single row.
    async fn legacy_reads(&self, actor: &str) -> Vec<Read> {
        let mut reads = Vec::with_capacity(STABLECOINS.len());
        for symbol in STABLECOINS {
            let contract = if symbol == "USDC" {
                super::config::USDC
            } else {
                self.cfg.p2p_usdt()
            }
            .to_string();
            let service = self.clone();
            let holder_actor = actor.to_string();
            let units = async move {
                service
                    .read_stablecoin(&holder_actor, "ethereum", &contract)
                    .await
            }
            .boxed();
            reads.push(Read {
                network: "ethereum".into(),
                symbol: symbol.into(),
                units,
            });
        }
        reads
    }

    /// The stablecoin payer signs from the verified Ethereum wallet on every
    /// network, exactly as the deal books `usdt_payer`.
    async fn read_stablecoin(
        &self,
        actor: &str,
        network: &str,
        contract: &str,
    ) -> Result<u128, Error> {
        let address = self.wallet(actor, "ethereum").await?;
        if super::config::evm_chain_id(network).is_some() {
            let rpcs = self.cfg.p2p_evm_rpcs(network).ok_or(Error::Unavailable)?;
            return self.token_balance(rpcs, contract, &address).await;
        }
        let endpoints = self
            .cfg
            .p2p_chain_endpoints(network)
            .ok_or(Error::Unavailable)?;
        match network {
            "tron" => self.tron_token_balance(endpoints, contract, &address).await,
            "solana" => {
                self.solana_token_balance(endpoints, &address, contract)
                    .await
            }
            "ton" => self.ton_token_balance(endpoints, &address, contract).await,
            _ => Err(Error::Unavailable),
        }
    }

    async fn balance(
        &self,
        chain: &str,
        symbol: &str,
        network: &str,
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
                tracing::warn!(%error, chain, symbol, network, "otc.balance_read_failed");
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
        assert_eq!(
            amount::decimal(15_812_490_000, STABLECOIN_DECIMALS),
            "15812.49"
        );
    }

    #[test]
    fn a_gap_carries_no_amount_for_the_portfolio_to_render() {
        for state in ["unverified", "unavailable"] {
            let gap = entry("everscale", "EVER", "Everscale", None, state);
            assert_eq!(gap["amount"], Value::Null, "{state}");
            assert_eq!(gap["state"], json!(state));
            assert_eq!(gap["symbol"], json!("EVER"));
        }
        let funded = entry(
            "wallet",
            "USDT",
            "BNB Smart Chain",
            Some("15812.49".into()),
            "ok",
        );
        assert_eq!(funded["amount"], json!("15812.49"));
        assert_eq!(funded["network"], json!("BNB Smart Chain"));
    }

    #[test]
    fn every_settling_network_has_its_own_label() {
        let labels = [
            "bnb-smart-chain",
            "tron",
            "solana",
            "ton",
            "near",
            "sei",
            "base",
        ];
        for network in labels {
            assert_ne!(network_label(network), "Ethereum", "{network}");
        }
        assert_eq!(network_label("bnb-smart-chain"), "BNB Smart Chain");
    }
}

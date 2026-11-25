use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::core::state::AppState;
use crate::route_engine::{canonical_network_id, NearToken};

#[derive(Debug, Default, Deserialize)]
pub struct TokenFilters {
    pub network: Option<String>,
}

/// One tradable token identity a browser wallet can hold.
#[derive(Debug, Clone, Serialize)]
pub struct WalletToken {
    /// Canonical network id from `/api/networks`.
    pub network: String,
    pub symbol: String,
    pub decimals: u8,
    /// Contract or mint address. `None` means the network's native coin.
    pub contract_address: Option<String>,
}

/// `GET /api/crypto-tokens` — token addresses and decimals for the cached
/// NEAR Intents catalog, so clients can read wallet balances of the exact asset.
pub async fn list(
    State(state): State<AppState>,
    Query(filters): Query<TokenFilters>,
) -> Json<Vec<WalletToken>> {
    let network = filters
        .network
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonical_network_id);
    Json(wallet_tokens(
        state.near_intents.supported_tokens().await,
        network.as_deref(),
    ))
}

/// Canonical, de-duplicated view of the catalog. Entries without decimals are
/// unusable for balance reads, and a real contract wins over a native
/// placeholder that shares the ticker.
fn wallet_tokens(tokens: Vec<NearToken>, network: Option<&str>) -> Vec<WalletToken> {
    let mut tokens = tokens
        .into_iter()
        .filter(|token| !token.symbol.ends_with("(DEPRECATED)"))
        .filter_map(|token| {
            Some(WalletToken {
                network: canonical_network_id(&token.blockchain),
                symbol: token.symbol,
                decimals: token.decimals?,
                contract_address: token.contract_address.filter(|value| !value.is_empty()),
            })
        })
        .filter(|token| network.is_none_or(|id| token.network == id))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| {
        left.network
            .cmp(&right.network)
            .then_with(|| left.symbol.cmp(&right.symbol))
            .then_with(|| {
                right
                    .contract_address
                    .is_some()
                    .cmp(&left.contract_address.is_some())
            })
            .then_with(|| left.contract_address.cmp(&right.contract_address))
    });
    tokens.dedup_by(|left, right| left.network == right.network && left.symbol == right.symbol);
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(blockchain: &str, symbol: &str, contract: Option<&str>) -> NearToken {
        NearToken {
            asset_id: format!("{symbol}-{blockchain}"),
            blockchain: blockchain.into(),
            symbol: symbol.into(),
            decimals: Some(6),
            contract_address: contract.map(str::to_string),
        }
    }

    #[test]
    fn tokens_are_canonicalized_and_deprecated_entries_are_dropped() {
        let tokens = wallet_tokens(
            vec![
                token("eth", "USDT", Some("0xdac")),
                token("eth", "USDT(DEPRECATED)", Some("0x000")),
                token("eth", "ETH", None),
            ],
            None,
        );
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].network, "ethereum");
        assert_eq!(tokens[0].symbol, "ETH");
        assert_eq!(tokens[0].contract_address, None);
        assert_eq!(tokens[1].contract_address.as_deref(), Some("0xdac"));
    }

    #[test]
    fn a_duplicate_ticker_keeps_the_real_contract() {
        let tokens = wallet_tokens(
            vec![
                token("tron", "USDT", None),
                token("tron", "USDT", Some("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t")),
            ],
            Some("tron"),
        );
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[0].contract_address.as_deref(),
            Some("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t")
        );
    }

    #[test]
    fn another_network_is_filtered_out() {
        let tokens = wallet_tokens(
            vec![
                token("eth", "USDT", Some("0xdac")),
                token("sol", "USDT", Some("Es9")),
            ],
            Some("solana"),
        );
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].network, "solana");
    }
}

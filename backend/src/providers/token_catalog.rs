use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// Public token metadata, independent of a quote provider's curated picker.
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum TokenCatalog {
    Wrapped { tokens: Vec<CatalogToken> },
    Flat(Vec<CatalogToken>),
}

impl TokenCatalog {
    pub(crate) fn tokens(self) -> Vec<CatalogToken> {
        let tokens = match self {
            Self::Wrapped { tokens } | Self::Flat(tokens) => tokens,
        };
        let tokens = tokens
            .into_iter()
            .filter_map(|mut token| {
                let verified_gram = match token.chain_id {
                    1 => token
                        .address
                        .eq_ignore_ascii_case("0x582d872a1b094fc48f5de31d3b73f2d9be47def1"),
                    56 => token
                        .address
                        .eq_ignore_ascii_case("0x76a797a59ba2c17726896976b7b3747bfd1d220f"),
                    85918 => token.address.is_empty(),
                    _ => false,
                };
                if verified_gram {
                    token.symbol = "GRAM".into();
                } else if matches!(token.symbol.to_ascii_uppercase().as_str(), "TON" | "GRAM") {
                    return None;
                }
                Some(token)
            })
            .collect::<Vec<_>>();
        let mut identities = HashMap::new();
        let mut ambiguous = HashSet::new();
        for token in &tokens {
            let key = (token.chain_id, token.symbol.to_ascii_uppercase());
            let address = if token.address.starts_with("0x") {
                token.address.to_ascii_lowercase()
            } else {
                token.address.clone()
            };
            let identity = (address, token.decimals);
            if let Some(previous) = identities.get(&key) {
                if *previous != identity {
                    ambiguous.insert(key);
                }
            } else {
                identities.insert(key, identity);
            }
        }
        if !ambiguous.is_empty() {
            tracing::warn!(
                count = ambiguous.len(),
                "excluding ambiguous token/network metadata from shared DEX catalog"
            );
        }
        tokens
            .into_iter()
            .filter(|token| {
                !ambiguous.contains(&(token.chain_id, token.symbol.to_ascii_uppercase()))
            })
            .collect()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogToken {
    pub(crate) chain_id: u64,
    pub(crate) symbol: String,
    pub(crate) address: String,
    pub(crate) decimals: u8,
}

impl CatalogToken {
    pub(crate) fn is_erc20(&self) -> bool {
        self.decimals <= 38
            && self.address.len() == 42
            && self.address.starts_with("0x")
            && self.address[2..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && self.address[2..].bytes().any(|byte| byte != b'0')
    }
}

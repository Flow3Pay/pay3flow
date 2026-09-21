use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CryptoNetwork {
    pub id: String,
    pub name: String,
    pub currencies: Vec<String>,
}

/// Assets supported by the exchange and by the intermediary selector.
pub const CRYPTO_ASSETS: &[&str] = &[
    "USDT", "USDC", "BTC", "ETH", "BNB", "SOL", "TRX", "TON", "DOGE", "LTC", "DAI", "FDUSD", "XRP",
    "ADA", "DOT", "LINK", "AVAX", "MATIC", "BCH", "NEAR", "APT", "ATOM", "UNI", "SUI",
];

/// Only list assets that can actually be deposited or withdrawn on a network.
/// Multi-chain tokens deliberately occur in more than one entry.
const NETWORKS: &[(&str, &str, &[&str])] = &[
    ("bitcoin", "Bitcoin", &["BTC"]),
    (
        "ethereum",
        "Ethereum (ERC-20)",
        &[
            "ETH", "USDT", "USDC", "BNB", "DAI", "FDUSD", "LINK", "MATIC", "UNI",
        ],
    ),
    (
        "arbitrum-one",
        "Arbitrum One",
        &["ETH", "USDT", "USDC", "DAI", "LINK", "UNI"],
    ),
    (
        "optimism",
        "Optimism",
        &["ETH", "USDT", "USDC", "DAI", "LINK", "UNI"],
    ),
    ("base", "Base", &["ETH", "USDC", "DAI", "LINK", "UNI"]),
    (
        "bnb-smart-chain",
        "BNB Smart Chain (BEP-20)",
        &[
            "BNB", "USDT", "USDC", "BTC", "ETH", "DAI", "FDUSD", "XRP", "ADA", "DOT", "LINK",
            "DOGE", "LTC", "BCH", "UNI",
        ],
    ),
    ("solana", "Solana", &["SOL", "USDT", "USDC"]),
    ("tron", "TRON (TRC-20)", &["TRX", "USDT"]),
    ("ton", "TON", &["TON", "USDT"]),
    ("dogecoin", "Dogecoin", &["DOGE"]),
    ("litecoin", "Litecoin", &["LTC"]),
    ("xrpl", "XRP Ledger", &["XRP"]),
    ("cardano", "Cardano", &["ADA"]),
    ("polkadot", "Polkadot", &["DOT"]),
    (
        "avalanche-c",
        "Avalanche C-Chain",
        &["AVAX", "USDT", "USDC", "DAI", "LINK"],
    ),
    (
        "polygon-pos",
        "Polygon PoS",
        &["MATIC", "USDT", "USDC", "DAI", "LINK", "UNI"],
    ),
    ("bitcoin-cash", "Bitcoin Cash", &["BCH"]),
    ("near", "NEAR", &["NEAR", "USDT", "USDC"]),
    ("aptos", "Aptos", &["APT", "USDT", "USDC"]),
    ("cosmos-hub", "Cosmos Hub", &["ATOM"]),
    ("sui", "Sui", &["SUI", "USDT", "USDC"]),
];

pub fn is_supported_asset(currency: &str) -> bool {
    CRYPTO_ASSETS
        .iter()
        .any(|asset| asset.eq_ignore_ascii_case(currency.trim()))
}

pub fn catalog() -> Vec<CryptoNetwork> {
    NETWORKS
        .iter()
        .map(|(id, name, currencies)| CryptoNetwork {
            id: (*id).to_string(),
            name: (*name).to_string(),
            currencies: currencies
                .iter()
                .map(|currency| (*currency).to_string())
                .collect(),
        })
        .collect()
}

pub fn for_currency(currency: Option<&str>) -> Vec<CryptoNetwork> {
    let currency = currency.map(|value| value.trim().to_ascii_uppercase());
    let networks = catalog()
        .into_iter()
        .filter(|network| {
            currency
                .as_deref()
                .is_none_or(|currency| network.currencies.iter().any(|item| item == currency))
        })
        .collect();
    networks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ethereum_supports_erc20_assets() {
        let network = catalog()
            .into_iter()
            .find(|network| network.id == "ethereum")
            .expect("Ethereum network is seeded");

        assert_eq!(network.id, "ethereum");
        assert_eq!(network.name, "Ethereum (ERC-20)");
        assert!(network.currencies.contains(&"ETH".to_string()));
        assert!(network.currencies.contains(&"USDC".to_string()));
        assert!(!network.currencies.contains(&"BTC".to_string()));
    }

    #[test]
    fn currency_filter_keeps_only_compatible_networks() {
        let eth = for_currency(Some("eth"));
        assert!(eth.iter().any(|network| network.id == "ethereum"));
        assert!(eth.iter().any(|network| network.id == "arbitrum-one"));
        assert!(!eth.iter().any(|network| network.id == "bitcoin"));
        assert_eq!(for_currency(Some("btc"))[0].id, "bitcoin");
        assert!(for_currency(Some("AMD")).is_empty());
    }

    #[test]
    fn every_exchange_asset_has_at_least_one_network() {
        for asset in CRYPTO_ASSETS {
            assert!(
                !for_currency(Some(asset)).is_empty(),
                "missing network for {asset}"
            );
            assert!(is_supported_asset(asset));
        }
    }
}

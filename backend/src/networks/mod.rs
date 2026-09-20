use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CryptoNetwork {
    pub id: String,
    pub name: String,
    pub currencies: Vec<String>,
}

const NETWORKS: &[(&str, &str, &[&str])] = &[("ethereum", "Ethereum", &["ETH", "USDC"])];

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
    fn ethereum_supports_eth_and_usdc() {
        let network = catalog().into_iter().next().expect("network catalog is seeded");

        assert_eq!(network.id, "ethereum");
        assert_eq!(network.name, "Ethereum");
        assert_eq!(network.currencies, ["ETH", "USDC"]);
    }

    #[test]
    fn currency_filter_keeps_only_compatible_networks() {
        assert_eq!(for_currency(Some("eth")), catalog());
        assert!(for_currency(Some("AMD")).is_empty());
    }
}

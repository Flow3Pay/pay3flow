use std::collections::BTreeSet;

use anyhow::Result;
use serde::Serialize;

use crate::db::DbPool;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CryptoNetwork {
    pub id: String,
    pub name: String,
    pub currencies: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkCatalog(Vec<CryptoNetwork>);

impl NetworkCatalog {
    pub async fn load(pool: &DbPool) -> Result<Self> {
        Ok(Self(list(pool, None).await?))
    }

    pub fn is_supported_asset(&self, currency: &str) -> bool {
        self.0.iter().any(|network| {
            network
                .currencies
                .iter()
                .any(|asset| asset.eq_ignore_ascii_case(currency.trim()))
        })
    }

    pub fn compatible_network(&self, network_id: &str, currency: &str) -> Option<&CryptoNetwork> {
        self.0.iter().find(|network| {
            network.id.eq_ignore_ascii_case(network_id)
                && network
                    .currencies
                    .iter()
                    .any(|asset| asset.eq_ignore_ascii_case(currency))
        })
    }

    pub fn assets(&self) -> Vec<String> {
        self.0
            .iter()
            .flat_map(|network| network.currencies.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_default() -> Self {
        Self(vec![
            CryptoNetwork {
                id: "bitcoin".into(),
                name: "Bitcoin".into(),
                currencies: vec!["BTC".into()],
            },
            CryptoNetwork {
                id: "ethereum".into(),
                name: "Ethereum (ERC-20)".into(),
                currencies: vec!["ETH".into(), "USDT".into(), "USDC".into()],
            },
            CryptoNetwork {
                id: "tron".into(),
                name: "TRON (TRC-20)".into(),
                currencies: vec!["TRX".into(), "USDT".into()],
            },
            CryptoNetwork {
                id: "ton".into(),
                name: "TON".into(),
                currencies: vec!["TON".into(), "USDT".into()],
            },
        ])
    }
}

pub async fn list(pool: &DbPool, currency: Option<&str>) -> Result<Vec<CryptoNetwork>> {
    let currency = currency
        .map(str::trim)
        .filter(|currency| !currency.is_empty())
        .map(str::to_ascii_uppercase);
    let client = pool.get().await?;
    let statement = client
        .prepare_cached(
            r#"
SELECT slug, name, currencies
FROM crypto_networks
WHERE status = 'enabled'
  AND ($1::TEXT IS NULL OR $1 = ANY(currencies))
ORDER BY slug
"#,
        )
        .await?;
    let rows = client.query(&statement, &[&currency]).await?;
    Ok(rows
        .into_iter()
        .map(|row| CryptoNetwork {
            id: row.get("slug"),
            name: row.get("name"),
            currencies: row.get("currencies"),
        })
        .collect())
}

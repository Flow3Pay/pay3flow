use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;

#[derive(Debug, Clone, Serialize)]
pub struct Provider {
    pub id: Uuid,
    pub slug: String,
    pub operation: String,
    pub source_url: String,
    pub name: String,
    pub currencies: Vec<String>,
    pub banks: Vec<String>,
    pub searchable: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderFilters {
    pub operation: Option<String>,
    pub currency: Option<String>,
    pub bank: Option<String>,
}

pub async fn list(pool: &DbPool, filters: ProviderFilters) -> Result<Vec<Provider>> {
    let operation = normalized(filters.operation, false);
    let currency = normalized(filters.currency, true);
    let bank = normalized(filters.bank, false);
    let client = pool.get().await?;
    let statement = client
        .prepare_cached(
            r#"
SELECT id, slug, operation, source_url, name, currencies, banks
FROM providers
WHERE status = 'enabled'
  AND ($1::TEXT IS NULL OR operation = $1)
  AND ($2::TEXT IS NULL OR $2 = ANY(currencies))
  AND ($3::TEXT IS NULL OR $3 = ANY(banks))
ORDER BY name, operation, slug
"#,
        )
        .await?;
    let rows = client
        .query(&statement, &[&operation, &currency, &bank])
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| Provider {
            id: row.get("id"),
            slug: row.get("slug"),
            operation: row.get("operation"),
            source_url: row.get("source_url"),
            name: row.get("name"),
            currencies: row.get("currencies"),
            banks: row.get("banks"),
            searchable: false,
        })
        .collect())
}

fn normalized(value: Option<String>, uppercase: bool) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| {
            if uppercase {
                value.to_ascii_uppercase()
            } else {
                value
            }
        })
}

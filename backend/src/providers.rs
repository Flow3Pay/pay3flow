use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::provider_adapter::{ProviderAdapters, WorkflowConfig};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSearchMode {
    Selectable,
    AlwaysOn,
    CatalogOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProviderFeeModel {
    pub kind: String,
    pub description: String,
    pub docs_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Provider {
    pub id: Uuid,
    pub slug: String,
    pub operation: String,
    pub source_url: String,
    pub name: String,
    pub currencies: Vec<String>,
    pub banks: Vec<String>,
    pub fee_model: Option<ProviderFeeModel>,
    pub searchable: bool,
    pub search_mode: ProviderSearchMode,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderAdapterRecord {
    pub slug: String,
    pub source_url: String,
    pub display_name: String,
    pub config: Option<ProviderAdapters>,
    pub workflow: Option<WorkflowConfig>,
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
SELECT id, slug, operation, source_url, name, currencies, banks, fee_model
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
    rows.into_iter()
        .map(|row| {
            let fee_value: serde_json::Value = row.get("fee_model");
            let fee_model = (!fee_value.as_object().is_some_and(serde_json::Map::is_empty))
                .then(|| serde_json::from_value(fee_value))
                .transpose()
                .with_context(|| "invalid Providerfile fee model stored in providers")?;
            Ok(Provider {
                id: row.get("id"),
                slug: row.get("slug"),
                operation: row.get("operation"),
                source_url: row.get("source_url"),
                name: row.get("name"),
                currencies: row.get("currencies"),
                banks: row.get("banks"),
                fee_model,
                searchable: false,
                search_mode: ProviderSearchMode::CatalogOnly,
            })
        })
        .collect()
}

pub(crate) async fn adapters(pool: &DbPool) -> Result<Vec<ProviderAdapterRecord>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            r#"
SELECT slug,
       MIN(source_url) AS source_url,
       MIN(name) AS display_name,
       array_agg(operation ORDER BY operation) AS operations,
       adapter,
       workflow
FROM providers
WHERE status = 'enabled'
  AND (adapter <> '{}'::JSONB OR workflow <> '{}'::JSONB)
GROUP BY slug, adapter, workflow
ORDER BY slug
"#,
            &[],
        )
        .await?;

    rows.into_iter()
        .map(|row| {
            let slug: String = row.get("slug");
            let value: serde_json::Value = row.get("adapter");
            let config: Option<ProviderAdapters> =
                (!value.as_object().is_some_and(serde_json::Map::is_empty))
                    .then(|| serde_json::from_value(value))
                    .transpose()
                    .with_context(|| format!("invalid Providerfile adapter stored for `{slug}`"))?;
            let workflow_value: serde_json::Value = row.get("workflow");
            let workflow: Option<WorkflowConfig> = (!workflow_value
                .as_object()
                .is_some_and(serde_json::Map::is_empty))
            .then(|| serde_json::from_value(workflow_value))
            .transpose()
            .with_context(|| format!("invalid Providerfile workflow stored for `{slug}`"))?;
            let operations: Vec<String> = row.get("operations");
            let has_buy = operations.iter().any(|operation| operation == "buy");
            let has_sell = operations.iter().any(|operation| operation == "sell");
            if let Some(config) = &config {
                config
                    .validate(has_buy, has_sell, &slug)
                    .map_err(anyhow::Error::msg)?;
            }
            if let Some(workflow) = &workflow {
                workflow
                    .validate(has_buy, has_sell, &slug)
                    .map_err(anyhow::Error::msg)?;
            }
            let display_name: String = row.get("display_name");
            Ok(ProviderAdapterRecord {
                slug,
                source_url: row.get("source_url"),
                display_name: provider_display_name(&display_name),
                config,
                workflow,
            })
        })
        .collect()
}

fn provider_display_name(name: &str) -> String {
    name.trim()
        .strip_suffix(" Buy")
        .or_else(|| name.trim().strip_suffix(" Sell"))
        .unwrap_or(name.trim())
        .to_string()
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

//! Build-time embedded provider definitions.
//!
//! `providers/reg.json` is the manifest. The referenced files are validated by
//! `build.rs` and embedded into the executable, so the deployed backend does
//! not need the provider directory at runtime.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

mod compiled {
    include!(concat!(env!("OUT_DIR"), "/providers.rs"));
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub enabled: bool,
    pub endpoint: String,
    pub request: RequestConfig,
    pub response: ResponseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestConfig {
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub query: BTreeMap<String, Value>,
    #[serde(default)]
    pub body: Option<Value>,
    #[serde(default)]
    pub side: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseConfig {
    pub success: SuccessConfig,
    pub items_path: String,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldConfig>,
    #[serde(default)]
    pub advertiser: BTreeMap<String, FieldConfig>,
    #[serde(default)]
    pub merchant_rules: Vec<MatchRule>,
    #[serde(default)]
    pub verified_rules: Option<Vec<MatchRule>>,
    #[serde(default)]
    pub urls: BTreeMap<String, String>,
    #[serde(default)]
    pub source_url_is_exact: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuccessConfig {
    pub path: String,
    pub equals: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FieldConfig {
    Path(String),
    Detailed {
        path: String,
        #[serde(default)]
        transform: Option<String>,
    },
}

impl FieldConfig {
    pub fn path(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::Detailed { path, .. } => path,
        }
    }

    pub fn transform(&self) -> Option<&str> {
        match self {
            Self::Path(_) => None,
            Self::Detailed { transform, .. } => transform.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatchRule {
    pub path: String,
    pub op: String,
    #[serde(default)]
    pub value: Option<Value>,
}

pub fn configs() -> Result<&'static [ProviderConfig]> {
    static CONFIGS: OnceLock<Result<Vec<ProviderConfig>, String>> = OnceLock::new();
    match CONFIGS.get_or_init(load_configs) {
        Ok(configs) => Ok(configs),
        Err(error) => Err(anyhow!(error.clone())),
    }
}

fn load_configs() -> Result<Vec<ProviderConfig>, String> {
    let registry: BTreeMap<String, String> = serde_json::from_str(compiled::REGISTRY_JSON)
        .map_err(|error| format!("invalid embedded providers/reg.json: {error}"))?;

    registry
        .keys()
        .map(|name| {
            let (_, text) = compiled::CONFIGS
                .iter()
                .find(|(config_name, _)| config_name == name)
                .ok_or_else(|| format!("provider {name:?} was not embedded"))?;
            let config: ProviderConfig = serde_json::from_str(text)
                .map_err(|error| format!("invalid embedded config for {name:?}: {error}"))?;
            if config.name != *name {
                return Err(format!(
                    "embedded provider name {:?} does not match registry key {:?}",
                    config.name, name
                ));
            }
            Ok(config)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::configs;

    #[test]
    fn registry_embeds_and_parses_all_provider_configs() {
        let names = configs()
            .unwrap()
            .iter()
            .map(|provider| provider.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["binance", "bitget", "bybit", "okx"]);
        assert!(configs()
            .unwrap()
            .iter()
            .all(|provider| provider.kind == "p2p"));
    }
}

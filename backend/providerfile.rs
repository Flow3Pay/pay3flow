use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProviderFile {
    buy: Option<RawProvider>,
    sell: Option<RawProvider>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProvider {
    source_url: String,
    name: String,
    #[serde(rename = "currency")]
    currencies: Vec<String>,
    #[serde(default)]
    banks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Buy,
    Sell,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDefinition {
    pub slug: String,
    pub operation: Operation,
    pub source_url: String,
    pub name: String,
    pub currencies: Vec<String>,
    pub banks: Vec<String>,
    pub source_file: String,
}

#[derive(Debug)]
pub struct ProviderFileError(String);

impl fmt::Display for ProviderFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProviderFileError {}

pub fn parse(
    contents: &str,
    slug: &str,
    source_file: &str,
) -> Result<Vec<ProviderDefinition>, ProviderFileError> {
    let raw: RawProviderFile = toml::from_str(contents)
        .map_err(|error| ProviderFileError(format!("{source_file}: {error}")))?;
    if raw.buy.is_none() && raw.sell.is_none() {
        return Err(ProviderFileError(format!(
            "{source_file}: at least one [buy] or [sell] section is required"
        )));
    }

    [(Operation::Buy, raw.buy), (Operation::Sell, raw.sell)]
        .into_iter()
        .filter_map(|(operation, provider)| provider.map(|provider| (operation, provider)))
        .map(|(operation, provider)| normalize(provider, slug, operation, source_file))
        .collect()
}

pub fn compile_dir(root: &Path) -> Result<String, ProviderFileError> {
    let mut files = Vec::new();
    collect_providerfiles(root, &mut files)?;
    files.sort();

    let mut definitions = Vec::new();
    let mut identities = HashSet::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let slug = slug_for(relative)?;
        let source_file = relative.to_string_lossy().replace('\\', "/");
        let contents = fs::read_to_string(&path).map_err(|error| {
            ProviderFileError(format!("cannot read {}: {error}", path.display()))
        })?;
        for definition in parse(&contents, &slug, &source_file)? {
            let identity = (definition.slug.clone(), definition.operation);
            if !identities.insert(identity) {
                return Err(ProviderFileError(format!(
                    "duplicate provider section: {}/{}",
                    definition.slug,
                    definition.operation.as_str()
                )));
            }
            definitions.push(definition);
        }
    }
    Ok(render_sql(&definitions))
}

pub fn render_sql(definitions: &[ProviderDefinition]) -> String {
    let mut sql = String::from(
        "-- Generated from providers/**/Providerfile. Do not edit by hand.\n\
         -- Regenerate with: cargo run --bin providerfile\n\
         -- The application embeds this migration; Providerfiles are not read at runtime.\n",
    );
    for definition in definitions {
        let currencies = sql_array(&definition.currencies);
        let banks = sql_array(&definition.banks);
        sql.push_str(&format!(
            "INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)\n\
             VALUES ({}, {}, {}, {}, {currencies}, {banks}, {})\n\
             ON CONFLICT (slug, operation) DO UPDATE SET\n\
                 source_url = EXCLUDED.source_url,\n\
                 name = EXCLUDED.name,\n\
                 currencies = EXCLUDED.currencies,\n\
                 banks = EXCLUDED.banks,\n\
                 source_file = EXCLUDED.source_file,\n\
                 updated_at = now();\n\n",
            sql_string(&definition.slug),
            sql_string(definition.operation.as_str()),
            sql_string(&definition.source_url),
            sql_string(&definition.name),
            sql_string(&definition.source_file),
        ));
    }
    if !definitions.is_empty() {
        sql.pop();
    }
    sql
}

fn normalize(
    raw: RawProvider,
    slug: &str,
    operation: Operation,
    source_file: &str,
) -> Result<ProviderDefinition, ProviderFileError> {
    let source_url = raw.source_url.trim().to_string();
    if !source_url.starts_with("https://") && !source_url.starts_with("http://") {
        return Err(ProviderFileError(format!(
            "{source_file}: {}/source_url must use http or https",
            operation.as_str()
        )));
    }
    let name = raw.name.trim().to_string();
    if name.is_empty() {
        return Err(ProviderFileError(format!(
            "{source_file}: {}/name must not be empty",
            operation.as_str()
        )));
    }
    let currencies = normalized_values(raw.currencies, true);
    if currencies.is_empty() {
        return Err(ProviderFileError(format!(
            "{source_file}: {}/currency must contain at least one value",
            operation.as_str()
        )));
    }
    if currencies.iter().any(|currency| {
        !(2..=12).contains(&currency.len())
            || !currency.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }) {
        return Err(ProviderFileError(format!(
            "{source_file}: {}/currency values must be 2-12 ASCII letters or digits",
            operation.as_str()
        )));
    }

    Ok(ProviderDefinition {
        slug: slug.to_string(),
        operation,
        source_url,
        name,
        currencies,
        banks: normalized_values(raw.banks, false),
        source_file: source_file.to_string(),
    })
}

fn normalized_values(values: Vec<String>, uppercase: bool) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| {
            if uppercase {
                value.to_ascii_uppercase()
            } else {
                value
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_providerfiles(
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), ProviderFileError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ProviderFileError(format!("cannot read {}: {error}", directory.display()))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| ProviderFileError(error.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_providerfiles(&path, files)?;
        } else if path.file_name().is_some_and(|name| name == "Providerfile") {
            files.push(path);
        }
    }
    Ok(())
}

fn slug_for(relative: &Path) -> Result<String, ProviderFileError> {
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let slug = parent
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("-")
        .to_ascii_lowercase();
    if slug.is_empty()
        || slug.len() > 64
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ProviderFileError(format!(
            "{}: parent directory must form a 1-64 character provider slug",
            relative.display()
        )));
    }
    Ok(slug)
}

fn sql_array(values: &[String]) -> String {
    if values.is_empty() {
        "ARRAY[]::TEXT[]".to_string()
    } else {
        format!(
            "ARRAY[{}]::TEXT[]",
            values
                .iter()
                .map(|value| sql_string(value))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
[sell]
source_url = "https://rate.am"
name = "Rate Sell"
currency = ["usd", "rub", "eur", "amd"]
banks = [""]

[buy]
source_url = "https://rate.am"
banks = [""]
currency = ["usd", "rub", "eur", "amd"]
name = "Rate Buy"
"#;

    #[test]
    fn parses_and_normalizes_the_documented_shape() {
        let definitions = parse(EXAMPLE, "rate-am", "rate-am/Providerfile").unwrap();

        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].operation, Operation::Buy);
        assert_eq!(definitions[0].currencies, ["AMD", "EUR", "RUB", "USD"]);
        assert!(definitions[0].banks.is_empty());
        assert_eq!(definitions[1].operation, Operation::Sell);
    }

    #[test]
    fn rejects_unknown_fields_instead_of_ignoring_typos() {
        let invalid = EXAMPLE.replace("source_url", "sorce_url");
        assert!(parse(&invalid, "rate-am", "Providerfile").is_err());
    }

    #[test]
    fn escapes_values_in_generated_sql() {
        let definitions = parse(
            &EXAMPLE.replace("Rate Buy", "Banker's Buy"),
            "rate-am",
            "rate-am/Providerfile",
        )
        .unwrap();

        assert!(render_sql(&definitions).contains("Banker''s Buy"));
    }
}

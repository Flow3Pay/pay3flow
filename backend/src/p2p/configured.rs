use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::p2p::service::{Advertiser, P2pOffer, P2pSearchQuery, P2pSide, P2pSource};
use crate::providers::{FieldConfig, MatchRule, ProviderConfig};

pub(crate) struct ConfiguredP2pSource {
    client: reqwest::Client,
    config: ProviderConfig,
    endpoint: String,
}

impl ConfiguredP2pSource {
    pub(crate) fn new(client: reqwest::Client, config: ProviderConfig, endpoint: String) -> Self {
        Self {
            client,
            config,
            endpoint,
        }
    }
}

#[async_trait]
impl P2pSource for ConfiguredP2pSource {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let context = RequestContext::new(query, &self.config);
        let mut request = match self.config.request.method.to_ascii_uppercase().as_str() {
            "GET" => self.client.get(&self.endpoint),
            "POST" => self.client.post(&self.endpoint),
            method => bail!(
                "provider {} uses unsupported HTTP method {method}",
                self.name()
            ),
        };

        for (name, value) in &self.config.request.headers {
            request = request.header(name, render_string(value, &context));
        }

        if !self.config.request.query.is_empty() {
            let query_params = self
                .config
                .request
                .query
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        render_value(value, &context)
                            .ok()
                            .and_then(value_to_string)
                            .unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>();
            request = request.query(&query_params);
        }

        if let Some(body) = &self.config.request.body {
            request = request.json(&render_value(body, &context)?);
        }

        let response: Value = request
            .send()
            .await
            .with_context(|| format!("{} P2P request failed", self.name()))?
            .error_for_status()
            .with_context(|| format!("{} P2P returned an HTTP error", self.name()))?
            .json()
            .await
            .with_context(|| format!("invalid {} P2P response", self.name()))?;

        let success = response
            .pointer(&render_path(&self.config.response.success.path, &context))
            .ok_or_else(|| anyhow::anyhow!("{} response has no success field", self.name()))?;
        if success != &self.config.response.success.equals {
            let message = response
                .pointer("/detailMsg")
                .or_else(|| response.pointer("/msg"))
                .and_then(|value| value_to_string(value.clone()))
                .unwrap_or_else(|| "unknown provider error".into());
            bail!("{} P2P provider error: {message}", self.name());
        }

        let items = response
            .pointer(&render_path(&self.config.response.items_path, &context))
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("{} response items are not an array", self.name()))?;

        items
            .iter()
            .map(|item| self.into_offer(item, query, &context))
            .collect()
    }
}

impl ConfiguredP2pSource {
    fn into_offer(
        &self,
        item: &Value,
        query: &P2pSearchQuery,
        context: &RequestContext<'_>,
    ) -> Result<P2pOffer> {
        let fields = &self.config.response.fields;
        let advertiser_fields = &self.config.response.advertiser;
        let field = |name: &str| -> Result<Value> {
            let spec = fields
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("provider {} has no field {name:?}", self.name()))?;
            read_field(item, spec, context)
                .ok_or_else(|| anyhow::anyhow!("{} item has no field {name:?}", self.name()))
        };
        let optional_field = |name: &str| -> Option<Value> {
            fields
                .get(name)
                .and_then(|spec| read_field(item, spec, context))
        };
        let advertiser_field = |name: &str| -> Option<Value> {
            advertiser_fields
                .get(name)
                .and_then(|spec| read_field(item, spec, context))
        };

        let ad_id = required_string(field("ad_id")?, "ad_id")?;
        let fiat = required_string(field("fiat")?, "fiat")?;
        let asset = required_string(field("asset")?, "asset")?;
        let advertiser_id = advertiser_field("id").and_then(value_to_string);
        let nickname = advertiser_field("nickname")
            .and_then(value_to_string)
            .unwrap_or_else(|| "unknown advertiser".into());
        let user_type = advertiser_field("user_type").and_then(value_to_string);
        let is_merchant = matches_rules(item, &self.config.response.merchant_rules, context);
        let is_verified = self
            .config
            .response
            .verified_rules
            .as_deref()
            .map(|rules| matches_rules(item, rules, context))
            .unwrap_or(is_merchant);

        let mut output_context = context.values();
        output_context.insert("ad_id".into(), ad_id.clone());
        output_context.insert("fiat".into(), fiat.clone());
        output_context.insert("asset".into(), asset.clone());
        if let Some(id) = &advertiser_id {
            output_context.insert("advertiser_id".into(), id.clone());
        }
        output_context.insert("nickname".into(), nickname.clone());

        let source_url = self
            .config
            .response
            .urls
            .get("source")
            .map(|url| render_output(url, &output_context))
            .unwrap_or_else(|| self.config.endpoint.clone());
        let advertiser_profile_url = self
            .config
            .response
            .urls
            .get("advertiser_profile")
            .filter(|_| advertiser_id.is_some())
            .map(|url| render_output(url, &output_context));

        Ok(P2pOffer {
            source: self.config.name.clone(),
            ad_id,
            side: query.side,
            fiat,
            asset,
            price: required_string(field("price")?, "price")?,
            available_asset: required_string(field("available_asset")?, "available_asset")?,
            min_fiat: required_string(field("min_fiat")?, "min_fiat")?,
            max_fiat: required_string(field("max_fiat")?, "max_fiat")?,
            payment_methods: optional_field("payment_methods")
                .map(value_to_strings)
                .unwrap_or_default(),
            pay_time_limit_minutes: optional_field("pay_time_limit_minutes")
                .and_then(|value| value_to_string(value).and_then(|value| value.parse().ok())),
            advertiser: Advertiser {
                id: advertiser_id,
                nickname,
                user_type,
                is_merchant,
                is_verified,
                completed_orders_30d: advertiser_field("completed_orders_30d")
                    .and_then(value_to_string)
                    .and_then(|value| value.parse().ok()),
                completion_rate_30d: advertiser_field("completion_rate_30d")
                    .and_then(value_to_string)
                    .and_then(|value| value.parse().ok()),
                positive_rate: advertiser_field("positive_rate")
                    .and_then(value_to_string)
                    .and_then(|value| value.parse().ok()),
            },
            advertiser_profile_url,
            source_url,
            source_url_is_exact: self.config.response.source_url_is_exact,
        })
    }
}

struct RequestContext<'a> {
    query: &'a P2pSearchQuery,
    side: String,
    fetch_limit: usize,
}

impl<'a> RequestContext<'a> {
    fn new(query: &'a P2pSearchQuery, config: &ProviderConfig) -> Self {
        let side = config
            .request
            .side
            .get(match query.side {
                P2pSide::BuyCrypto => "buy",
                P2pSide::SellCrypto => "sell",
            })
            .cloned()
            .unwrap_or_else(|| match query.side {
                P2pSide::BuyCrypto => "buy".into(),
                P2pSide::SellCrypto => "sell".into(),
            });
        Self {
            query,
            side,
            fetch_limit: query.fetch_limit(),
        }
    }

    fn values(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::from([
            ("fiat".into(), self.query.fiat.clone()),
            ("asset".into(), self.query.asset.clone()),
            ("side".into(), self.side.clone()),
            (
                "user_side".into(),
                match self.query.side {
                    P2pSide::BuyCrypto => "buy".into(),
                    P2pSide::SellCrypto => "sell".into(),
                },
            ),
            ("fetch_limit".into(), self.fetch_limit.to_string()),
            ("amount".into(), String::new()),
            ("payment_method".into(), String::new()),
        ]);
        if let Some(amount) = self.query.amount {
            values.insert("amount".into(), amount.to_string());
        }
        if let Some(payment_method) = &self.query.payment_method {
            values.insert("payment_method".into(), payment_method.clone());
        }
        values
    }
}

fn render_value(value: &Value, context: &RequestContext<'_>) -> Result<Value> {
    match value {
        Value::String(string) => Ok(Value::String(render_string(string, context))),
        Value::Array(values) => values
            .iter()
            .map(|value| render_value(value, context))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::Object(values) => {
            if let Some(template) = values.get("$template").and_then(Value::as_str) {
                let rendered = context.values().get(template).cloned().unwrap_or_default();
                return match values.get("$type").and_then(Value::as_str) {
                    Some("number") => {
                        if let Ok(value) = rendered.parse::<u64>() {
                            Ok(Value::Number(value.into()))
                        } else if let Ok(value) = rendered.parse::<f64>() {
                            serde_json::Number::from_f64(value)
                                .map(Value::Number)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("template {template:?} is not a number")
                                })
                        } else {
                            Err(anyhow::anyhow!("template {template:?} is not a number"))
                        }
                    }
                    Some("bool") => rendered
                        .parse::<bool>()
                        .map(Value::Bool)
                        .map_err(|_| anyhow::anyhow!("template {template:?} is not a bool")),
                    _ => Ok(Value::String(rendered)),
                };
            }
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), render_value(value, context)?)))
                .collect::<Result<Map<_, _>>>()
                .map(Value::Object)
        }
        other => Ok(other.clone()),
    }
}

fn render_string(template: &str, context: &RequestContext<'_>) -> String {
    render_output(template, &context.values())
}

fn render_path(template: &str, context: &RequestContext<'_>) -> String {
    render_output(template, &context.values())
}

fn render_output(template: &str, values: &BTreeMap<String, String>) -> String {
    let mut output = template.to_owned();
    for (name, value) in values {
        output = output.replace(&format!("{{{{{name}}}}}"), value);
    }
    output
}

fn read_field(item: &Value, field: &FieldConfig, context: &RequestContext<'_>) -> Option<Value> {
    let value = read_json_path(item, &render_path(field.path(), context))?;
    Some(apply_transform(value, field.transform()))
}

fn read_json_path(root: &Value, path: &str) -> Option<Value> {
    let mut values = vec![root.clone()];
    for segment in path.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        let segment = segment.replace("~1", "/").replace("~0", "~");
        values = values
            .into_iter()
            .flat_map(|value| match value {
                Value::Object(object) if segment == "*" => object.into_values().collect(),
                Value::Object(object) => object.get(&segment).cloned().into_iter().collect(),
                Value::Array(array) if segment == "*" => array,
                Value::Array(array) => segment
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| array.get(index).cloned())
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        if values.is_empty() {
            return None;
        }
    }
    if values.len() == 1 {
        Some(values.remove(0))
    } else {
        Some(Value::Array(values))
    }
}

fn apply_transform(value: Value, transform: Option<&str>) -> Value {
    match transform {
        Some("percentage") => value_to_string(value)
            .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
            .and_then(|value| {
                serde_json::Number::from_f64(if value > 1.0 { value / 100.0 } else { value })
            })
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Some("ratio") => value_to_string(value)
            .and_then(|value| value.parse::<f64>().ok())
            .and_then(|value| {
                serde_json::Number::from_f64(if value > 1.0 { value / 100.0 } else { value })
            })
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => value,
    }
}

fn matches_rules(item: &Value, rules: &[MatchRule], context: &RequestContext<'_>) -> bool {
    rules.iter().any(|rule| {
        let Some(actual) = item.pointer(&render_path(&rule.path, context)) else {
            return false;
        };
        match rule.op.as_str() {
            "present" => {
                !actual.is_null()
                    && value_to_string(actual.clone()).is_some_and(|value| !value.is_empty())
            }
            "nonempty" => match actual {
                Value::Array(values) => !values.is_empty(),
                Value::Object(values) => !values.is_empty(),
                Value::String(value) => !value.is_empty(),
                _ => false,
            },
            "truthy" => actual.as_bool().unwrap_or(false),
            "equals" => rule
                .value
                .as_ref()
                .is_some_and(|expected| actual == expected),
            "equals_ci" => rule
                .value
                .as_ref()
                .and_then(|value| value_to_string(value.clone()))
                .and_then(|expected| {
                    value_to_string(actual.clone()).map(|actual| (actual, expected))
                })
                .is_some_and(|(actual, expected)| actual.eq_ignore_ascii_case(&expected)),
            "gt" => rule
                .value
                .as_ref()
                .and_then(|expected| expected.as_f64())
                .and_then(|expected| actual.as_f64().map(|actual| actual > expected))
                .unwrap_or(false),
            _ => false,
        }
    })
}

fn value_to_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn required_string(value: Value, field: &str) -> Result<String> {
    value_to_string(value).ok_or_else(|| anyhow::anyhow!("provider field {field:?} is not scalar"))
}

fn value_to_strings(value: Value) -> Vec<String> {
    match value {
        Value::Array(values) => values.into_iter().filter_map(value_to_string).collect(),
        value => value_to_string(value).into_iter().collect(),
    }
}

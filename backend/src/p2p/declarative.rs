use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::{Client, Method};
use serde_json::{Number, Value};
use sha2::Sha512;

use crate::p2p::service::{
    Advertiser, P2pOffer, P2pOfferMarket, P2pSearchQuery, P2pSide, P2pSource,
};
use crate::p2p::spot::{CryptoMarketSource, CryptoTicker};
use crate::provider_adapter::{
    environment_variable, MarketAdapterConfig, OfferMapping, P2pAdapterConfig, P2pAdapterMarket,
    P2pOperation, RateTableConfig, ValueCondition,
};
use crate::providers::ProviderAdapterRecord;
use crate::route_engine::canonical_network_id;

pub(crate) struct DeclarativeP2pSource {
    client: Client,
    slug: String,
    source_url: String,
    display_name: String,
    config: P2pAdapterConfig,
}

impl DeclarativeP2pSource {
    pub(crate) fn from_record(client: Client, record: &ProviderAdapterRecord) -> Option<Self> {
        record.config.as_ref()?.p2p.clone().map(|config| Self {
            client,
            slug: record.slug.clone(),
            source_url: record.source_url.clone(),
            display_name: record.display_name.clone(),
            config,
        })
    }

    fn operation(&self, side: P2pSide) -> Result<&P2pOperation> {
        match side {
            P2pSide::BuyCrypto => self.config.buy.as_ref(),
            P2pSide::SellCrypto => self.config.sell.as_ref(),
        }
        .ok_or_else(|| anyhow!("{} does not support this operation", self.slug))
    }

    fn supports_fiat(&self, fiat: &str) -> bool {
        self.config.supported_fiats.is_empty()
            || self
                .config
                .supported_fiats
                .iter()
                .any(|supported| supported.eq_ignore_ascii_case(fiat))
    }

    fn template_values<'a>(
        &'a self,
        query: &'a P2pSearchQuery,
        operation: &P2pOperation,
    ) -> Result<TemplateValues<'a>> {
        let asset = self
            .config
            .asset_codes
            .get(&query.asset)
            .map(String::as_str)
            .unwrap_or(&query.asset);
        let amount = match operation.amount_mode.as_str() {
            "query_or_empty" => query.amount,
            "fiat_probe" => query.amount.or(self.config.fiat_probe_amount),
            "asset_probe" => self.config.asset_probe_amount,
            mode => bail!("unsupported amount mode `{mode}`"),
        };
        Ok(TemplateValues {
            fiat: &query.fiat,
            asset,
            amount,
            limit: query
                .fetch_limit()
                .min(self.config.max_results.unwrap_or(100)),
        })
    }

    fn into_offer(
        &self,
        item: &Value,
        query: &P2pSearchQuery,
        mapping: &OfferMapping,
    ) -> Result<P2pOffer> {
        let fiat = optional_string(item, mapping.fiat_pointer.as_deref())
            .unwrap_or_else(|| query.fiat.clone());
        let remote_asset = optional_string(item, mapping.asset_pointer.as_deref())
            .unwrap_or_else(|| query.asset.clone());
        let asset = self
            .config
            .asset_codes
            .iter()
            .find_map(|(canonical, remote)| (remote == &remote_asset).then(|| canonical.clone()))
            .unwrap_or(remote_asset);
        let network = optional_string(item, mapping.network_pointer.as_deref())
            .or_else(|| mapping.network.clone())
            .map(|network| canonical_network_id(&network));
        let operation = self.operation(query.side)?;
        let input_amount = self.template_values(query, operation)?.amount;
        let price = mapped_price(item, mapping, query.side, input_amount)?;
        let available_asset = mapped_number_string(
            item,
            mapping.available_asset_pointer.as_deref(),
            self.config.default_available_asset,
            "available asset",
        )?;
        let min_fiat = mapped_number_string(
            item,
            mapping.min_fiat_pointer.as_deref(),
            self.config.default_min_fiat,
            "minimum fiat",
        )?;
        let max_fiat = mapped_number_string(
            item,
            mapping.max_fiat_pointer.as_deref(),
            self.config.default_max_fiat,
            "maximum fiat",
        )?;
        let advertiser_id = optional_string(item, mapping.advertiser_id_pointer.as_deref());
        let user_type = if self.config.market == P2pAdapterMarket::DirectExchange {
            Some("service".into())
        } else {
            optional_string(item, mapping.advertiser_user_type_pointer.as_deref())
        };
        let is_merchant = mapping.merchant_default
            || mapping
                .merchant_conditions
                .iter()
                .any(|condition| condition_matches(item, condition));
        let is_verified = mapping.verified_default
            || (mapping.verified_from_merchant && is_merchant)
            || mapping
                .verified_conditions
                .iter()
                .any(|condition| condition_matches(item, condition));
        let variables = LinkVariables {
            fiat: &fiat,
            asset: &asset,
            side: query.side,
        };
        let advertiser_profile_url = mapping
            .advertiser_profile_url_template
            .as_deref()
            .map(|template| render_link(template, item, &variables))
            .transpose()?;
        let source_url = mapping
            .source_url_template
            .as_deref()
            .map(|template| render_link(template, item, &variables))
            .transpose()?
            .unwrap_or_else(|| self.source_url.clone());

        Ok(P2pOffer {
            market: match self.config.market {
                P2pAdapterMarket::P2p => P2pOfferMarket::P2p,
                P2pAdapterMarket::DirectExchange => P2pOfferMarket::DirectExchange,
            },
            source: self.slug.clone(),
            ad_id: optional_string(item, mapping.ad_id_pointer.as_deref()).unwrap_or_else(|| {
                format!("{}-{}-{}-{}", self.slug, side_name(query.side), fiat, asset)
            }),
            side: query.side,
            fiat,
            asset,
            network,
            price,
            available_asset,
            min_fiat,
            max_fiat,
            payment_methods: payment_methods(item, mapping),
            pay_time_limit_minutes: optional_u64(item, mapping.pay_time_limit_pointer.as_deref())
                .and_then(|value| u32::try_from(value).ok()),
            advertiser: Advertiser {
                id: advertiser_id,
                nickname: optional_string(item, mapping.advertiser_nickname_pointer.as_deref())
                    .unwrap_or_else(|| self.display_name.clone()),
                user_type,
                is_merchant,
                is_verified,
                completed_orders_30d: optional_u64(
                    item,
                    mapping.completed_orders_pointer.as_deref(),
                ),
                completion_rate_30d: optional_rate(
                    item,
                    mapping.completion_rate_pointer.as_deref(),
                ),
                positive_rate: optional_rate(item, mapping.positive_rate_pointer.as_deref()),
            },
            advertiser_profile_url,
            source_url,
            source_url_is_exact: mapping.source_url_is_exact,
        })
    }

    fn rate_table_offer(
        &self,
        response: &Value,
        query: &P2pSearchQuery,
        table: &RateTableConfig,
    ) -> Result<Option<P2pOffer>> {
        let remote_fiat = table
            .fiat_codes
            .get(&query.fiat)
            .map(String::as_str)
            .unwrap_or(&query.fiat);
        let fiat_rate = response
            .pointer(&table.fiat_items_pointer)
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find_map(|item| {
                    (optional_string(item, Some(&table.fiat_code_pointer)).as_deref()
                        == Some(remote_fiat))
                    .then(|| required_number(item, &table.fiat_rate_pointer, "fiat rate"))
                })
            })
            .transpose()?;
        let asset_index = response
            .pointer(&table.asset_items_pointer)
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().position(|item| {
                    optional_string(item, Some(&table.asset_code_pointer)).as_deref()
                        == Some(query.asset.as_str())
                })
            });
        let asset_rate = asset_index
            .and_then(|index| {
                response
                    .pointer(&table.asset_rates_pointer)
                    .and_then(Value::as_array)
                    .and_then(|rates| rates.get(index))
                    .and_then(value_number)
            })
            .filter(|rate| rate.is_finite() && *rate > 0.0);
        let Some(price) = fiat_rate
            .zip(asset_rate)
            .map(|(fiat_rate, asset_rate)| fiat_rate * asset_rate)
            .filter(|price| price.is_finite() && *price > 0.0)
        else {
            return Ok(None);
        };

        Ok(Some(P2pOffer {
            market: P2pOfferMarket::DirectExchange,
            source: self.slug.clone(),
            ad_id: format!(
                "{}-{}-{}-{}",
                self.slug,
                side_name(query.side),
                query.fiat,
                query.asset
            ),
            side: query.side,
            fiat: query.fiat.clone(),
            asset: query.asset.clone(),
            network: None,
            price: number_to_string(price),
            available_asset: number_to_string(
                self.config
                    .default_available_asset
                    .unwrap_or(1_000_000_000.0),
            ),
            min_fiat: number_to_string(self.config.default_min_fiat.unwrap_or(1.0)),
            max_fiat: number_to_string(self.config.default_max_fiat.unwrap_or(1_000_000_000.0)),
            payment_methods: Vec::new(),
            pay_time_limit_minutes: None,
            advertiser: Advertiser {
                id: None,
                nickname: self.display_name.clone(),
                user_type: Some("service".into()),
                is_merchant: true,
                is_verified: true,
                completed_orders_30d: None,
                completion_rate_30d: None,
                positive_rate: None,
            },
            advertiser_profile_url: None,
            source_url: table
                .source_url
                .clone()
                .unwrap_or_else(|| self.source_url.clone()),
            source_url_is_exact: false,
        }))
    }
}

#[async_trait]
impl P2pSource for DeclarativeP2pSource {
    fn name(&self) -> &str {
        &self.slug
    }

    fn timeout(&self, _default: Duration) -> Duration {
        Duration::from_millis(self.config.timeout_ms)
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        if !self.supports_fiat(&query.fiat) {
            return Ok(Vec::new());
        }
        if !self.config.supported_assets.is_empty()
            && !self
                .config
                .supported_assets
                .iter()
                .any(|asset| asset == &query.asset)
        {
            return Ok(Vec::new());
        }
        if query.amount.is_some_and(|amount| {
            self.config
                .default_min_fiat
                .is_some_and(|minimum| amount < minimum)
                || self
                    .config
                    .default_max_fiat
                    .is_some_and(|maximum| amount > maximum)
        }) {
            return Ok(Vec::new());
        }

        let operation = self.operation(query.side)?;
        let values = self.template_values(query, operation)?;
        let endpoint = render_request_string(
            operation
                .endpoint
                .as_deref()
                .unwrap_or(&self.config.endpoint),
            &values,
        );
        let response = send_json(
            &self.client,
            &self.config.method,
            &endpoint,
            &self.config.headers,
            self.config.auth.as_ref(),
            &operation.query,
            operation.request_json.as_deref(),
            &values,
            &self.slug,
        )
        .await?;
        validate_response(
            &response,
            operation.success_pointer.as_deref(),
            operation.success_value.as_deref(),
            operation.success_missing_allowed,
            operation.error_pointer.as_deref(),
            &self.slug,
        )?;
        if let Some(table) = &self.config.rate_table {
            return self
                .rate_table_offer(&response, query, table)
                .map(|offer| offer.into_iter().collect());
        }
        let mapping = operation
            .offer
            .as_ref()
            .or(self.config.offer.as_ref())
            .ok_or_else(|| anyhow!("{} has no offer mapping for this operation", self.slug))?;
        response_items(&response, operation.items_pointer.as_deref())?
            .into_iter()
            .take(query.fetch_limit())
            .map(|item| self.into_offer(item, query, mapping))
            .collect()
    }
}

pub(crate) struct DeclarativeMarketSource {
    client: Client,
    slug: String,
    config: MarketAdapterConfig,
}

impl DeclarativeMarketSource {
    pub(crate) fn from_record(client: Client, record: &ProviderAdapterRecord) -> Option<Self> {
        record.config.as_ref()?.market.clone().map(|config| Self {
            client,
            slug: record.slug.clone(),
            config,
        })
    }
}

#[async_trait]
impl CryptoMarketSource for DeclarativeMarketSource {
    fn name(&self) -> &str {
        &self.slug
    }

    fn timeout(&self, _default: Duration) -> Duration {
        Duration::from_millis(self.config.timeout_ms)
    }

    async fn tickers(&self) -> Result<Vec<CryptoTicker>> {
        let values = TemplateValues {
            fiat: "",
            asset: "",
            amount: None,
            limit: 100,
        };
        let response = send_json(
            &self.client,
            &self.config.method,
            &self.config.endpoint,
            &self.config.headers,
            None,
            &self.config.query,
            self.config.request_json.as_deref(),
            &values,
            &self.slug,
        )
        .await?;
        validate_response(
            &response,
            self.config.success_pointer.as_deref(),
            self.config.success_value.as_deref(),
            self.config.success_missing_allowed,
            self.config.error_pointer.as_deref(),
            &self.slug,
        )?;
        let tickers = response_items(&response, self.config.items_pointer.as_deref())?
            .into_iter()
            .filter_map(|item| {
                let mut symbol =
                    required_string(item, &self.config.symbol_pointer, "symbol").ok()?;
                if let Some(remove) = &self.config.symbol_remove {
                    symbol = symbol.replace(remove, "");
                }
                let bid = required_number(item, &self.config.bid_pointer, "bid").ok()?;
                let ask = required_number(item, &self.config.ask_pointer, "ask").ok()?;
                (bid > 0.0 && ask > 0.0).then_some(CryptoTicker { symbol, bid, ask })
            })
            .collect::<Vec<_>>();
        Ok(tickers)
    }
}

struct TemplateValues<'a> {
    fiat: &'a str,
    asset: &'a str,
    amount: Option<f64>,
    limit: usize,
}

async fn send_json(
    client: &Client,
    method: &str,
    endpoint: &str,
    headers: &BTreeMap<String, String>,
    auth: Option<&crate::provider_adapter::HmacAuthConfig>,
    query: &BTreeMap<String, String>,
    request_json: Option<&str>,
    values: &TemplateValues<'_>,
    source: &str,
) -> Result<Value> {
    let method = Method::from_bytes(method.as_bytes()).context("invalid Providerfile method")?;
    let mut request = client.request(method, endpoint);
    for (name, value) in headers {
        let value = if let Some(variable) = environment_variable(value) {
            std::env::var(variable)
                .with_context(|| format!("{source} requires environment variable `{variable}`"))?
        } else {
            value.clone()
        };
        request = request.header(name, value);
    }
    if !query.is_empty() {
        let query = query
            .iter()
            .map(|(key, value)| (key, render_request_string(value, values)))
            .collect::<Vec<_>>();
        request = request.query(&query);
    }
    let mut request_body = None;
    if let Some(template) = request_json {
        let mut body: Value = serde_json::from_str(template)
            .with_context(|| format!("invalid request template for {source}"))?;
        render_request_json(&mut body, values)?;
        request = request.json(&body);
        request_body = Some(body);
    }
    if let Some(auth) = auth {
        let public_key = std::env::var(&auth.public_key_env).with_context(|| {
            format!(
                "{source} requires environment variable `{}`",
                auth.public_key_env
            )
        })?;
        let private_key = std::env::var(&auth.private_key_env).with_context(|| {
            format!(
                "{source} requires environment variable `{}`",
                auth.private_key_env
            )
        })?;
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let body = request_body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .context("failed to serialize authenticated request body")?
            .unwrap_or_default();
        let mut mac = Hmac::<Sha512>::new_from_slice(private_key.as_bytes())
            .map_err(|_| anyhow!("invalid HMAC key for {source}"))?;
        mac.update(timestamp.as_bytes());
        mac.update(&body);
        let signature = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        request = request
            .header(&auth.public_key_header, public_key)
            .header(&auth.timestamp_header, timestamp)
            .header(&auth.signature_header, signature);
    }
    let response = request
        .send()
        .await
        .with_context(|| format!("{source} request failed"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("cannot read {source} response"))?;
    let body: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("{source} returned invalid JSON with HTTP {status}"))?;
    if !status.is_success() {
        bail!("{source} returned HTTP {status}: {}", compact_json(&body));
    }
    Ok(body)
}

fn render_request_json(value: &mut Value, values: &TemplateValues<'_>) -> Result<()> {
    match value {
        Value::String(template) if template == "{{amount_number}}" => {
            let amount = values
                .amount
                .ok_or_else(|| anyhow!("numeric amount placeholder has no value"))?;
            *value = Value::Number(
                Number::from_f64(amount).ok_or_else(|| anyhow!("amount is not finite"))?,
            );
        }
        Value::String(template) if template == "{{limit_number}}" => {
            *value = Value::Number(Number::from(values.limit));
        }
        Value::String(template) => *template = render_request_string(template, values),
        Value::Array(values_json) => {
            for value in values_json {
                render_request_json(value, values)?;
            }
        }
        Value::Object(values_json) => {
            for value in values_json.values_mut() {
                render_request_json(value, values)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn render_request_string(template: &str, values: &TemplateValues<'_>) -> String {
    template
        .replace("{{fiat}}", values.fiat)
        .replace("{{asset}}", values.asset)
        .replace(
            "{{amount_number}}",
            &values.amount.map(number_to_string).unwrap_or_default(),
        )
        .replace(
            "{{amount}}",
            &values.amount.map(number_to_string).unwrap_or_default(),
        )
        .replace("{{limit_number}}", &values.limit.to_string())
        .replace("{{limit}}", &values.limit.to_string())
}

fn validate_response(
    response: &Value,
    success_pointer: Option<&str>,
    success_value: Option<&str>,
    success_missing_allowed: bool,
    error_pointer: Option<&str>,
    source: &str,
) -> Result<()> {
    let Some(pointer) = success_pointer else {
        return Ok(());
    };
    let expected = success_value.expect("validated success pair");
    let actual = response.pointer(pointer).and_then(scalar_string);
    if actual.is_none() && success_missing_allowed {
        return Ok(());
    }
    let actual = actual.unwrap_or_default();
    if actual == expected {
        return Ok(());
    }
    let error = error_pointer
        .and_then(|pointer| response.pointer(pointer))
        .and_then(scalar_string)
        .unwrap_or_else(|| compact_json(response));
    bail!("{source} response condition failed ({actual} != {expected}): {error}")
}

fn response_items<'a>(response: &'a Value, pointer: Option<&str>) -> Result<Vec<&'a Value>> {
    let value = pointer
        .map(|pointer| {
            response
                .pointer(pointer)
                .ok_or_else(|| anyhow!("response does not contain `{pointer}`"))
        })
        .transpose()?
        .unwrap_or(response);
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => Ok(items.iter().collect()),
        Value::Object(_) => Ok(vec![value]),
        _ => bail!("configured items value is neither an object nor an array"),
    }
}

fn mapped_price(
    item: &Value,
    mapping: &OfferMapping,
    side: P2pSide,
    input_amount: Option<f64>,
) -> Result<String> {
    let mut price = if let Some(pointer) = mapping.price_pointer.as_deref() {
        required_number(item, pointer, "price")?
    } else {
        let fiat = required_number(
            item,
            mapping
                .fiat_amount_pointer
                .as_deref()
                .expect("validated fiat amount pointer"),
            "fiat amount",
        )?;
        let asset = required_number(
            item,
            mapping
                .asset_amount_pointer
                .as_deref()
                .expect("validated asset amount pointer"),
            "asset amount",
        )?;
        fiat / asset
    };
    if mapping.price_inverted {
        price = 1.0 / price;
    }
    if let Some(pointer) = mapping.output_fee_pointer.as_deref() {
        let fee = required_number(item, pointer, "output fee")?;
        if fee < 0.0 {
            bail!("mapped output fee must not be negative");
        }
        let input = input_amount
            .filter(|amount| amount.is_finite() && *amount > 0.0)
            .ok_or_else(|| anyhow!("an input amount is required to apply the output fee"))?;
        let net_output = match side {
            P2pSide::BuyCrypto => input / price - fee,
            P2pSide::SellCrypto => input * price - fee,
        };
        if !net_output.is_finite() || net_output <= 0.0 {
            bail!("mapped output fee consumes the quoted output");
        }
        price = match side {
            P2pSide::BuyCrypto => input / net_output,
            P2pSide::SellCrypto => net_output / input,
        };
    }
    if !price.is_finite() || price <= 0.0 {
        bail!("mapped price is not a positive finite number");
    }
    Ok(number_to_string(price))
}

fn mapped_number_string(
    item: &Value,
    pointer: Option<&str>,
    fallback: Option<f64>,
    field: &str,
) -> Result<String> {
    if let Some(value) = pointer.and_then(|pointer| item.pointer(pointer)) {
        let number = value_number(value).ok_or_else(|| anyhow!("invalid {field}"))?;
        return Ok(number_to_string(number));
    }
    fallback
        .map(number_to_string)
        .ok_or_else(|| anyhow!("missing {field}"))
}

fn payment_methods(item: &Value, mapping: &OfferMapping) -> Vec<String> {
    let Some(value) = mapping
        .payment_methods_pointer
        .as_deref()
        .and_then(|pointer| item.pointer(pointer))
    else {
        return Vec::new();
    };
    let values = value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| std::slice::from_ref(value));
    values
        .iter()
        .filter_map(|value| {
            if let Some(value) = scalar_string(value).filter(|value| !value.is_empty()) {
                return Some(value);
            }
            mapping
                .payment_method_value_pointer
                .as_deref()
                .and_then(|pointer| value.pointer(pointer))
                .and_then(scalar_string)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    mapping
                        .payment_method_fallback_pointer
                        .as_deref()
                        .and_then(|pointer| value.pointer(pointer))
                        .and_then(scalar_string)
                        .filter(|value| !value.is_empty())
                })
        })
        .collect()
}

fn condition_matches(item: &Value, condition: &ValueCondition) -> bool {
    let Some(value) = item.pointer(&condition.pointer) else {
        return false;
    };
    match condition.operator.as_str() {
        "truthy" => match value {
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
            Value::String(value) => matches!(value.to_ascii_lowercase().as_str(), "true" | "1"),
            _ => false,
        },
        "non_empty" => match value {
            Value::String(value) => !value.is_empty(),
            Value::Array(value) => !value.is_empty(),
            Value::Object(value) => !value.is_empty(),
            Value::Null => false,
            _ => true,
        },
        "equals" => scalar_string(value).as_deref() == condition.value.as_deref(),
        "equals_ci" => scalar_string(value)
            .zip(condition.value.clone())
            .is_some_and(|(actual, expected)| actual.eq_ignore_ascii_case(&expected)),
        "not_equals" => scalar_string(value).as_deref() != condition.value.as_deref(),
        "not_equals_ci" => scalar_string(value)
            .zip(condition.value.clone())
            .is_some_and(|(actual, expected)| !actual.eq_ignore_ascii_case(&expected)),
        _ => false,
    }
}

struct LinkVariables<'a> {
    fiat: &'a str,
    asset: &'a str,
    side: P2pSide,
}

fn render_link(template: &str, item: &Value, variables: &LinkVariables<'_>) -> Result<String> {
    let mut rendered = template
        .replace("{{fiat}}", variables.fiat)
        .replace("{{asset}}", variables.asset)
        .replace("{{fiat_lower}}", &variables.fiat.to_ascii_lowercase())
        .replace("{{asset_lower}}", &variables.asset.to_ascii_lowercase())
        .replace("{{side}}", side_name(variables.side));
    while let Some(start) = rendered.find("{{item:") {
        let after_start = start + "{{item:".len();
        let end = rendered[after_start..]
            .find("}}")
            .map(|end| after_start + end)
            .ok_or_else(|| anyhow!("unclosed item placeholder in URL template"))?;
        let pointer = &rendered[after_start..end];
        let value = item
            .pointer(pointer)
            .and_then(scalar_string)
            .ok_or_else(|| anyhow!("URL template pointer `{pointer}` is missing"))?;
        rendered.replace_range(start..end + 2, &value);
    }
    Ok(rendered)
}

fn side_name(side: P2pSide) -> &'static str {
    match side {
        P2pSide::BuyCrypto => "buy",
        P2pSide::SellCrypto => "sell",
    }
}

fn optional_string(item: &Value, pointer: Option<&str>) -> Option<String> {
    pointer
        .and_then(|pointer| item.pointer(pointer))
        .and_then(scalar_string)
        .filter(|value| !value.is_empty())
}

fn required_string(item: &Value, pointer: &str, field: &str) -> Result<String> {
    item.pointer(pointer)
        .and_then(scalar_string)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing or invalid {field} at `{pointer}`"))
}

fn required_number(item: &Value, pointer: &str, field: &str) -> Result<f64> {
    item.pointer(pointer)
        .and_then(value_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| anyhow!("missing or invalid {field} at `{pointer}`"))
}

fn optional_u64(item: &Value, pointer: Option<&str>) -> Option<u64> {
    let value = pointer.and_then(|pointer| item.pointer(pointer))?;
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn optional_rate(item: &Value, pointer: Option<&str>) -> Option<f64> {
    let mut value = pointer
        .and_then(|pointer| item.pointer(pointer))
        .and_then(value_number)?;
    if value > 1.0 {
        value /= 100.0;
    }
    (0.0..=1.0).contains(&value).then_some(value)
}

fn value_number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()?
            .trim()
            .trim_end_matches('%')
            .parse::<f64>()
            .ok()
    })
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn number_to_string(value: f64) -> String {
    value.to_string()
}

fn compact_json(value: &Value) -> String {
    value.to_string().chars().take(500).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_adapter::ProviderAdapters;

    fn cifra_source() -> DeclarativeP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/cifra-broker/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "cifra-broker".into(),
            source_url: "https://cifra.by/".into(),
            display_name: "Cifra Markets".into(),
            config: Some(adapters),
            workflow: None,
        };
        DeclarativeP2pSource::from_record(Client::new(), &record).unwrap()
    }

    fn whitebird_source() -> DeclarativeP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/whitebird/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "whitebird".into(),
            source_url: "https://whitebird.io/".into(),
            display_name: "Whitebird".into(),
            config: Some(adapters),
            workflow: None,
        };
        DeclarativeP2pSource::from_record(Client::new(), &record).unwrap()
    }

    fn skylabs_source() -> DeclarativeP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/skylabs/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "skylabs".into(),
            source_url: "https://skylabs.world/".into(),
            display_name: "SkyLabs".into(),
            config: Some(adapters),
            workflow: None,
        };
        DeclarativeP2pSource::from_record(Client::new(), &record).unwrap()
    }

    fn bncex_source() -> DeclarativeP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/bncex/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "bncex".into(),
            source_url: "https://www.bncex.com/en".into(),
            display_name: "bncex".into(),
            config: Some(adapters),
            workflow: None,
        };
        DeclarativeP2pSource::from_record(Client::new(), &record).unwrap()
    }

    fn bitcoin_center_source() -> DeclarativeP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/bitcoin-center/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "bitcoin-center".into(),
            source_url: "https://www.bitcoincenter.am/en/".into(),
            display_name: "Bitcoin Center".into(),
            config: Some(adapters),
            workflow: None,
        };
        DeclarativeP2pSource::from_record(Client::new(), &record).unwrap()
    }

    #[test]
    fn renders_typed_and_string_request_placeholders() {
        let mut value: Value = serde_json::from_str(
            r#"{"amount":"{{amount}}","size":"{{limit}}","limit":"{{limit_number}}"}"#,
        )
        .unwrap();
        render_request_json(
            &mut value,
            &TemplateValues {
                fiat: "RUB",
                asset: "USDT_TRC",
                amount: Some(1_000.0),
                limit: 20,
            },
        )
        .unwrap();
        assert_eq!(value["amount"], "1000");
        assert_eq!(value["size"], "20");
        assert_eq!(value["limit"], 20);
    }

    #[test]
    fn normalizes_percentage_rates() {
        let item: Value = serde_json::from_str(r#"{"rate":"99.5%"}"#).unwrap();
        assert_eq!(optional_rate(&item, Some("/rate")), Some(0.995));
    }

    #[test]
    fn maps_a_binance_offer_using_only_its_providerfile() {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/binance/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "binance".into(),
            source_url: "https://www.binance.com".into(),
            display_name: "Binance".into(),
            config: Some(adapters),
            workflow: None,
        };
        let source = DeclarativeP2pSource::from_record(Client::new(), &record).unwrap();
        let response: Value = serde_json::from_str(
            r#"{"adv":{"advNo":"ad-1","price":"361.75","fiatUnit":"AMD","asset":"USDT","tradableQuantity":"423.41","minSingleTransAmount":"19997.54","maxSingleTransAmount":"36175.00","payTimeLimit":15,"tradeMethods":[{"tradeMethodName":"IDBank"}]},"advertiser":{"userNo":"user-1","nickName":"Trader","userType":"merchant","monthOrderCount":1204,"monthFinishRate":0.999,"positiveRate":1.0,"merchantGroupMember":false}}"#,
        )
        .unwrap();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(20_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: Some("binance".into()),
        };
        let mapping = source.config.offer.as_ref().unwrap();
        let offer = source.into_offer(&response, &query, mapping).unwrap();

        assert_eq!(offer.ad_id, "ad-1");
        assert_eq!(offer.price, "361.75");
        assert_eq!(offer.payment_methods, ["IDBank"]);
        assert!(offer.advertiser.is_merchant);
        assert!(offer.source_url_is_exact);
    }

    #[test]
    fn maps_an_aligned_rate_table_using_only_its_providerfile() {
        let source = cifra_source();
        let response: Value = serde_json::from_str(
            r#"{"data":{"currenciesReal":[{"code":"RUR","rate":{"value":84.5}}],"currenciesNotReal":[{"code":"BTC"},{"code":"USDT"}],"currenciesNotRealRate":[85000,1]}}"#,
        )
        .unwrap();
        let query = P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "BTC".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(100_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: Some("cifra-broker".into()),
        };

        let offer = source
            .rate_table_offer(
                &response,
                &query,
                source.config.rate_table.as_ref().unwrap(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(offer.fiat, "RUB");
        assert_eq!(offer.asset, "BTC");
        assert_eq!(offer.price, "7182500");
        assert_eq!(
            offer.source_url,
            "https://tradernet.by/authentication/signup"
        );
        assert!(offer.advertiser.is_verified);
    }

    #[test]
    fn maps_a_whitebird_quote_as_a_direct_exchange_offer() {
        let source = whitebird_source();
        let response: Value = serde_json::from_str(
            r#"{"rate":"ETH/RUB","actualRateValue":"235251.2189","input":{"type":"FIAT_PROVIDER","asset":"RUB","amount":"10000","feeAmount":"250","provider":"ALFA"},"output":{"type":"CRYPTO_TRANSFER","asset":"ETH","amount":"0.04250775","feeAmount":"0.00000505"}}"#,
        )
        .unwrap();
        let query = P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "ETH".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(10_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("whitebird".into()),
        };
        let mapping = source.config.buy.as_ref().unwrap().offer.as_ref().unwrap();

        let offer = source.into_offer(&response, &query, mapping).unwrap();

        assert_eq!(offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(offer.ad_id, "whitebird-buy-RUB-ETH");
        assert_eq!(offer.fiat, "RUB");
        assert_eq!(offer.asset, "ETH");
        assert!((offer.price.parse::<f64>().unwrap() - 235_251.2189).abs() < 0.01);
        assert!(offer.advertiser.is_merchant);
        assert!(offer.advertiser.is_verified);
        assert_eq!(offer.source_url, "https://whitebird.io/exchanger");
        assert!(!offer.source_url_is_exact);
    }

    #[test]
    fn maps_bncex_buy_and_sell_quotes_as_direct_amd_offers() {
        let source = bncex_source();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(100_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("bncex".into()),
        };
        let buy: Value = serde_json::from_str(
            r#"{"token":"USDT","type":"BUY_USDT","network":"TRC20","paymentMethod":"NON_CASH","amountAmd":100000,"amountUsdt":267.09143962316284,"rate":365.5,"networkFeeApplied":2.5,"exchangeFeePercentageApplied":1.5}"#,
        )
        .unwrap();
        let buy_offer = source
            .into_offer(
                &buy,
                &query,
                source.config.buy.as_ref().unwrap().offer.as_ref().unwrap(),
            )
            .unwrap();

        assert_eq!(buy_offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(buy_offer.fiat, "AMD");
        assert_eq!(buy_offer.asset, "USDT");
        assert_eq!(buy_offer.network.as_deref(), Some("tron"));
        assert!((buy_offer.price.parse::<f64>().unwrap() - 374.403613).abs() < 0.000001);

        let sell: Value = serde_json::from_str(
            r#"{"token":"USDT","type":"SELL_USDT","network":"TRC20","paymentMethod":"NON_CASH","amountAmd":35306,"amountUsdt":100,"rate":361,"networkFeeApplied":2.5,"exchangeFeePercentageApplied":2.2}"#,
        )
        .unwrap();
        let mut sell_query = query;
        sell_query.side = P2pSide::SellCrypto;
        let sell_offer = source
            .into_offer(
                &sell,
                &sell_query,
                source.config.sell.as_ref().unwrap().offer.as_ref().unwrap(),
            )
            .unwrap();

        assert_eq!(sell_offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(sell_offer.network.as_deref(), Some("tron"));
        assert_eq!(sell_offer.price, "353.06");
        assert_eq!(sell_offer.source_url, "https://www.bncex.com/en");
        assert!(sell_offer.advertiser.is_verified);
    }

    #[test]
    fn renders_bncex_quote_request_from_its_providerfile() {
        let source = bncex_source();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDC".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(250_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("bncex".into()),
        };
        let operation = source.operation(query.side).unwrap();
        let values = source.template_values(&query, operation).unwrap();
        let mut body: Value =
            serde_json::from_str(operation.request_json.as_deref().unwrap()).unwrap();

        render_request_json(&mut body, &values).unwrap();

        assert_eq!(body["type"], "BUY_USDC");
        assert_eq!(body["token"], "USDC");
        assert_eq!(body["network"], "TRC20");
        assert_eq!(body["paymentMethod"], "NON_CASH");
        assert_eq!(body["amountAmd"].as_f64(), Some(250_000.0));
    }

    #[test]
    fn limits_bncex_quotes_to_amd_legs() {
        let source = bncex_source();

        assert!(source.supports_fiat("AMD"));
        assert!(source.supports_fiat("amd"));
        assert!(!source.supports_fiat("RUB"));
    }

    #[test]
    fn maps_bitcoin_center_sol_usdt_rate_with_output_fee() {
        let source = bitcoin_center_source();
        let response: Value = serde_json::from_str(
            r#"{"success":true,"data":{"route":{"from":{"name":"Bank Transfer","symbol":"AMD","xml":"WIREAMD","min":"50000","max":"10000000"},"to":{"name":"USDT (SOL) Solana","symbol":"USDT","xml":"USDTSOL"},"rate":{"in":365.33760001,"out":1,"amount":"99996854.963213","outFeeAmount":5},"routeId":"6a75d602f50f4685ab92bfd0","orderTTL":30}}}"#,
        )
        .unwrap();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(100_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("bitcoin-center".into()),
        };
        let mapping = source.config.buy.as_ref().unwrap().offer.as_ref().unwrap();

        let offer = source.into_offer(&response, &query, mapping).unwrap();

        assert_eq!(offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(offer.fiat, "AMD");
        assert_eq!(offer.asset, "USDT");
        assert_eq!(offer.network.as_deref(), Some("solana"));
        assert_eq!(offer.payment_methods, ["Bank Transfer"]);
        assert_eq!(offer.pay_time_limit_minutes, Some(30));
        assert_eq!(
            offer.source_url,
            "https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL"
        );
        assert!(offer.source_url_is_exact);
        assert!((offer.price.parse::<f64>().unwrap() - 372.135351825745).abs() < 0.000001);

        let reverse: Value = serde_json::from_str(
            r#"{"success":true,"data":{"route":{"from":{"name":"USDT (SOL) Solana","symbol":"USDT","xml":"USDTSOL","min":"141.670335","max":"28334.069734"},"to":{"name":"Bank Transfer","symbol":"AMD","xml":"WIREAMD"},"rate":{"in":1,"out":352.93203883,"amount":"9984863629","outFeeAmount":0},"routeId":"6a75d54ef50f4685ab92bdd3","orderTTL":30}}}"#,
        )
        .unwrap();
        let mut sell_query = query;
        sell_query.side = P2pSide::SellCrypto;
        sell_query.amount = None;
        let sell_mapping = source.config.sell.as_ref().unwrap().offer.as_ref().unwrap();
        let sell_offer = source
            .into_offer(&reverse, &sell_query, sell_mapping)
            .unwrap();

        assert_eq!(sell_offer.fiat, "AMD");
        assert_eq!(sell_offer.asset, "USDT");
        assert_eq!(sell_offer.network.as_deref(), Some("solana"));
        assert_eq!(sell_offer.price, "352.93203883");
        assert_eq!(sell_offer.payment_methods, ["Bank Transfer"]);
        assert_eq!(
            sell_offer.source_url,
            "https://www.bitcoincenter.am/en/?from=USDTSOL&to=WIREAMD"
        );
    }

    #[tokio::test]
    #[ignore = "calls the live Bitcoin Center route API"]
    async fn live_bitcoin_center_api_returns_sol_usdt_routes() {
        let source = bitcoin_center_source();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "AMD".into(),
                    asset: "USDT".into(),
                    side,
                    amount: (side == P2pSide::BuyCrypto).then_some(100_000.0),
                    payment_method: Some("Bank Transfer".into()),
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("bitcoin-center".into()),
                })
                .await
                .unwrap();

            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].market, P2pOfferMarket::DirectExchange);
            assert_eq!(offers[0].side, side);
            assert!(offers[0].price.parse::<f64>().unwrap() > 0.0);
            assert!(offers[0].source_url.contains("USDTSOL"));
        }
    }

    #[tokio::test]
    #[ignore = "calls the live bncex quote API"]
    async fn live_bncex_api_returns_buy_and_sell_quotes() {
        let source = bncex_source();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "AMD".into(),
                    asset: "USDT".into(),
                    side,
                    amount: (side == P2pSide::BuyCrypto).then_some(100_000.0),
                    payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("bncex".into()),
                })
                .await
                .unwrap();

            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].market, P2pOfferMarket::DirectExchange);
            assert_eq!(offers[0].side, side);
            assert!(offers[0].price.parse::<f64>().unwrap() > 0.0);
        }
    }

    #[tokio::test]
    #[ignore = "calls the live Whitebird quote API"]
    async fn live_whitebird_api_returns_buy_and_sell_quotes() {
        let source = whitebird_source();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "RUB".into(),
                    asset: "ETH".into(),
                    side,
                    amount: (side == P2pSide::BuyCrypto).then_some(10_000.0),
                    payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("whitebird".into()),
                })
                .await
                .unwrap();

            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].market, P2pOfferMarket::DirectExchange);
            assert_eq!(offers[0].side, side);
            assert!(offers[0].price.parse::<f64>().unwrap() > 0.0);
        }
    }

    #[test]
    fn maps_skylabs_homepage_rate_as_a_direct_exchange() {
        let source = skylabs_source();
        let response: Value = serde_json::from_str(r#"{"status":true,"result":365.5}"#).unwrap();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(100_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("skylabs".into()),
        };

        let offer = source
            .into_offer(&response, &query, source.config.offer.as_ref().unwrap())
            .unwrap();

        assert_eq!(offer.market, P2pOfferMarket::DirectExchange);
        assert_eq!(offer.price, "365.5");
        assert_eq!(offer.advertiser.user_type.as_deref(), Some("service"));
        assert_eq!(offer.source_url, "https://skylabs.world/#rates");
    }

    #[test]
    fn renders_operation_specific_endpoint_placeholders() {
        let source = skylabs_source();
        let query = P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(100_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(1),
            sources: Some("skylabs".into()),
        };
        let operation = source.operation(query.side).unwrap();
        let values = source.template_values(&query, operation).unwrap();

        assert_eq!(
            render_request_string(operation.endpoint.as_deref().unwrap(), &values),
            "https://api.skylabs.world/api/rate/USDT/AMD/sell"
        );
    }

    #[tokio::test]
    #[ignore = "calls the live Cifra Markets API"]
    async fn live_cifra_rate_table_returns_buy_and_sell_quotes() {
        let source = cifra_source();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "RUB".into(),
                    asset: "USDT".into(),
                    side,
                    amount: Some(1_000.0),
                    payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("cifra-broker".into()),
                })
                .await
                .unwrap();

            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].market, P2pOfferMarket::DirectExchange);
            assert!(offers[0].price.parse::<f64>().unwrap() > 0.0);
        }
    }

    #[tokio::test]
    #[ignore = "calls the live Cifra Markets API"]
    async fn live_cifra_market_returns_imex_tickers() {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/cifra-broker/Providerfile")).unwrap();
        let adapters: ProviderAdapters =
            document.get("adapter").unwrap().clone().try_into().unwrap();
        let record = ProviderAdapterRecord {
            slug: "cifra-broker".into(),
            source_url: "https://cifra.by/".into(),
            display_name: "Cifra Markets".into(),
            config: Some(adapters),
            workflow: None,
        };
        let source = DeclarativeMarketSource::from_record(Client::new(), &record).unwrap();

        let tickers = source.tickers().await.unwrap();

        assert!(tickers
            .iter()
            .any(|ticker| ticker.symbol == "BTCUSDT" && ticker.bid > 0.0));
    }

    #[tokio::test]
    #[ignore = "calls the live SkyLabs homepage API"]
    async fn live_skylabs_returns_distinct_buy_and_sell_quotes() {
        let source = skylabs_source();
        let mut prices = Vec::new();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "AMD".into(),
                    asset: "USDT".into(),
                    side,
                    amount: Some(100_000.0),
                    payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("skylabs".into()),
                })
                .await
                .unwrap();
            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].market, P2pOfferMarket::DirectExchange);
            prices.push(offers[0].price.parse::<f64>().unwrap());
        }
        assert!(prices[0] > prices[1]);
    }
}

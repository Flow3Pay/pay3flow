use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const REQUEST_PLACEHOLDERS: [&str; 6] = [
    "fiat",
    "asset",
    "amount",
    "amount_number",
    "limit",
    "limit_number",
];

/// Live adapters compiled from a Providerfile and stored as JSON in Postgres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAdapters {
    pub p2p: Option<P2pAdapterConfig>,
    pub market: Option<MarketAdapterConfig>,
    pub bestchange: Option<BestChangeAdapterConfig>,
}

/// Configuration for the official BestChange API adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BestChangeAdapterConfig {
    /// API base URL. The API key is read from `api_key_env` and appended only
    /// while constructing requests; it is never stored in the provider row.
    pub endpoint: String,
    pub api_key_env: String,
    pub public_endpoint: String,
    pub affiliate_id: Option<String>,
    #[serde(default = "default_bestchange_language")]
    pub language: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_bestchange_max_results")]
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    pub source_url: String,
    pub get_exchange: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub navigation_retries: u8,
    #[serde(default = "default_navigation_retry_delay_ms")]
    pub navigation_retry_delay_ms: u64,
    #[serde(default)]
    pub asset_codes: BTreeMap<String, String>,
    #[serde(default)]
    pub supported_assets: Vec<String>,
    pub fiat_probe_amount: Option<f64>,
    pub asset_probe_amount: Option<f64>,
    pub default_min_fiat: f64,
    pub default_max_fiat: f64,
    pub default_available_asset: f64,
    #[serde(default)]
    pub is_merchant: bool,
    #[serde(default)]
    pub is_verified: bool,
    pub buy: Option<WorkflowOperation>,
    pub sell: Option<WorkflowOperation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowOperation {
    #[serde(default = "default_amount_mode")]
    pub amount_mode: String,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    pub fiat_amount: WorkflowRead,
    pub asset_amount: WorkflowRead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowStep {
    Fill {
        selector: String,
        value: String,
    },
    Click {
        selector: String,
    },
    Press {
        selector: String,
        key: String,
    },
    SelectOption {
        selector: String,
        value: String,
    },
    ReactSelect {
        selector: String,
        value: String,
    },
    WaitFor {
        selector: String,
    },
    Wait {
        milliseconds: u64,
    },
    Evaluate {
        script: String,
        value: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRead {
    pub selector: String,
    #[serde(default = "default_read_property")]
    pub property: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P2pAdapterConfig {
    pub kind: String,
    #[serde(default, skip_serializing_if = "P2pAdapterMarket::is_p2p")]
    pub market: P2pAdapterMarket,
    pub endpoint: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub asset_codes: BTreeMap<String, String>,
    #[serde(default)]
    pub supported_assets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_fiats: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    pub max_results: Option<usize>,
    pub fiat_probe_amount: Option<f64>,
    pub asset_probe_amount: Option<f64>,
    pub default_min_fiat: Option<f64>,
    pub default_max_fiat: Option<f64>,
    pub default_available_asset: Option<f64>,
    pub auth: Option<HmacAuthConfig>,
    pub buy: Option<P2pOperation>,
    pub sell: Option<P2pOperation>,
    pub offer: Option<OfferMapping>,
    pub rate_table: Option<RateTableConfig>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P2pAdapterMarket {
    #[default]
    P2p,
    DirectExchange,
}

impl P2pAdapterMarket {
    fn is_p2p(&self) -> bool {
        *self == Self::P2p
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmacAuthConfig {
    pub kind: String,
    pub public_key_env: String,
    pub private_key_env: String,
    #[serde(default = "default_public_key_header")]
    pub public_key_header: String,
    #[serde(default = "default_private_timestamp_header")]
    pub timestamp_header: String,
    #[serde(default = "default_signature_header")]
    pub signature_header: String,
}

/// A calculator response whose asset codes and rates are stored in parallel arrays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateTableConfig {
    pub fiat_items_pointer: String,
    pub fiat_code_pointer: String,
    pub fiat_rate_pointer: String,
    pub asset_items_pointer: String,
    pub asset_code_pointer: String,
    pub asset_rates_pointer: String,
    #[serde(default)]
    pub fiat_codes: BTreeMap<String, String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P2pOperation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    pub request_json: Option<String>,
    #[serde(default = "default_amount_mode")]
    pub amount_mode: String,
    pub items_pointer: Option<String>,
    pub success_pointer: Option<String>,
    pub success_value: Option<String>,
    #[serde(default)]
    pub success_missing_allowed: bool,
    pub error_pointer: Option<String>,
    pub offer: Option<OfferMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferMapping {
    pub ad_id_pointer: Option<String>,
    pub fiat_pointer: Option<String>,
    pub asset_pointer: Option<String>,
    pub price_pointer: Option<String>,
    pub fiat_amount_pointer: Option<String>,
    pub asset_amount_pointer: Option<String>,
    /// Fixed fee deducted from the quoted output amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_fee_pointer: Option<String>,
    #[serde(default)]
    pub price_inverted: bool,
    pub available_asset_pointer: Option<String>,
    pub min_fiat_pointer: Option<String>,
    pub max_fiat_pointer: Option<String>,
    pub payment_methods_pointer: Option<String>,
    pub payment_method_value_pointer: Option<String>,
    pub payment_method_fallback_pointer: Option<String>,
    pub pay_time_limit_pointer: Option<String>,
    pub advertiser_id_pointer: Option<String>,
    pub advertiser_nickname_pointer: Option<String>,
    pub advertiser_user_type_pointer: Option<String>,
    #[serde(default)]
    pub merchant_conditions: Vec<ValueCondition>,
    #[serde(default)]
    pub verified_conditions: Vec<ValueCondition>,
    #[serde(default)]
    pub merchant_default: bool,
    #[serde(default)]
    pub verified_default: bool,
    #[serde(default)]
    pub verified_from_merchant: bool,
    pub completed_orders_pointer: Option<String>,
    pub completion_rate_pointer: Option<String>,
    pub positive_rate_pointer: Option<String>,
    pub source_url_template: Option<String>,
    #[serde(default)]
    pub source_url_is_exact: bool,
    pub advertiser_profile_url_template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueCondition {
    pub pointer: String,
    pub operator: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketAdapterConfig {
    pub kind: String,
    pub endpoint: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    pub request_json: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    pub items_pointer: Option<String>,
    pub symbol_pointer: String,
    pub bid_pointer: String,
    pub ask_pointer: String,
    pub symbol_remove: Option<String>,
    pub success_pointer: Option<String>,
    pub success_value: Option<String>,
    #[serde(default)]
    pub success_missing_allowed: bool,
    pub error_pointer: Option<String>,
}

impl ProviderAdapters {
    pub fn validate(&self, has_buy: bool, has_sell: bool, context: &str) -> Result<(), String> {
        if self.p2p.is_none() && self.market.is_none() && self.bestchange.is_none() {
            return Err(format!(
                "{context}: adapter must contain [adapter.p2p], [adapter.market], or [adapter.bestchange]"
            ));
        }
        if let Some(p2p) = &self.p2p {
            p2p.validate(has_buy, has_sell, context)?;
        }
        if let Some(market) = &self.market {
            market.validate(context)?;
        }
        if let Some(bestchange) = &self.bestchange {
            bestchange.validate(context)?;
        }
        Ok(())
    }
}

impl BestChangeAdapterConfig {
    fn validate(&self, context: &str) -> Result<(), String> {
        for (field, value) in [
            ("endpoint", self.endpoint.as_str()),
            ("public_endpoint", self.public_endpoint.as_str()),
        ] {
            if !value.starts_with("https://") && !value.starts_with("http://") {
                return Err(format!(
                    "{context}: BestChange {field} must use http or https"
                ));
            }
        }
        if !valid_environment_variable(&self.api_key_env) {
            return Err(format!(
                "{context}: BestChange api_key_env is not a valid environment variable"
            ));
        }
        if self.affiliate_id.as_deref().is_some_and(|value| {
            value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
        }) {
            return Err(format!(
                "{context}: BestChange affiliate_id must contain only digits"
            ));
        }
        if self.language.trim().is_empty() || self.language.len() > 8 {
            return Err(format!(
                "{context}: BestChange language must be 1-8 characters"
            ));
        }
        if !(250..=30_000).contains(&self.timeout_ms) {
            return Err(format!(
                "{context}: BestChange timeout_ms must be between 250 and 30000"
            ));
        }
        if self.max_results == 0 {
            return Err(format!(
                "{context}: BestChange max_results must be positive"
            ));
        }
        Ok(())
    }
}

impl WorkflowConfig {
    pub fn validate(&self, has_buy: bool, has_sell: bool, context: &str) -> Result<(), String> {
        for (field, url) in [
            ("source_url", self.source_url.as_str()),
            ("get_exchange", self.get_exchange.as_str()),
        ] {
            if !url.starts_with("https://") && !url.starts_with("http://") {
                return Err(format!(
                    "{context}: workflow/{field} must use http or https"
                ));
            }
        }
        if !(250..=30_000).contains(&self.timeout_ms) {
            return Err(format!(
                "{context}: workflow/timeout_ms must be between 250 and 30000"
            ));
        }
        if self.navigation_retries > 3 {
            return Err(format!(
                "{context}: workflow/navigation_retries must not exceed 3"
            ));
        }
        if !(100..=5_000).contains(&self.navigation_retry_delay_ms) {
            return Err(format!(
                "{context}: workflow/navigation_retry_delay_ms must be between 100 and 5000"
            ));
        }
        for (field, value) in [
            ("fiat_probe_amount", self.fiat_probe_amount),
            ("asset_probe_amount", self.asset_probe_amount),
            ("default_min_fiat", Some(self.default_min_fiat)),
            ("default_max_fiat", Some(self.default_max_fiat)),
            (
                "default_available_asset",
                Some(self.default_available_asset),
            ),
        ] {
            if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                return Err(format!(
                    "{context}: workflow/{field} must be a positive finite number"
                ));
            }
        }
        if self.default_min_fiat > self.default_max_fiat {
            return Err(format!(
                "{context}: workflow/default_min_fiat must not exceed default_max_fiat"
            ));
        }
        for asset in &self.supported_assets {
            if !valid_asset_code(asset) {
                return Err(format!(
                    "{context}: workflow/supported_assets contains invalid code `{asset}`"
                ));
            }
        }
        for (canonical, remote) in &self.asset_codes {
            if !valid_asset_code(canonical) || !valid_remote_asset_code(remote) {
                return Err(format!("{context}: workflow/asset_codes is invalid"));
            }
        }
        if has_buy {
            self.buy
                .as_ref()
                .ok_or_else(|| format!("{context}: workflow/buy is required for [buy]"))?
                .validate(self, context, "buy")?;
        }
        if has_sell {
            self.sell
                .as_ref()
                .ok_or_else(|| format!("{context}: workflow/sell is required for [sell]"))?
                .validate(self, context, "sell")?;
        }
        Ok(())
    }
}

impl WorkflowOperation {
    fn validate(
        &self,
        workflow: &WorkflowConfig,
        context: &str,
        operation: &str,
    ) -> Result<(), String> {
        if !matches!(
            self.amount_mode.as_str(),
            "query_or_empty" | "fiat_probe" | "asset_probe"
        ) {
            return Err(format!(
                "{context}: workflow/{operation}/amount_mode is invalid"
            ));
        }
        if self.amount_mode == "fiat_probe" && workflow.fiat_probe_amount.is_none() {
            return Err(format!(
                "{context}: workflow/fiat_probe_amount is required by {operation}"
            ));
        }
        if self.amount_mode == "asset_probe" && workflow.asset_probe_amount.is_none() {
            return Err(format!(
                "{context}: workflow/asset_probe_amount is required by {operation}"
            ));
        }
        if self.steps.is_empty() {
            return Err(format!(
                "{context}: workflow/{operation}/steps must not be empty"
            ));
        }
        for step in &self.steps {
            step.validate(context, operation)?;
        }
        self.fiat_amount.validate(context)?;
        self.asset_amount.validate(context)?;
        Ok(())
    }
}

impl WorkflowStep {
    fn validate(&self, context: &str, operation: &str) -> Result<(), String> {
        let step_context = format!("{context}: workflow/{operation}/steps");
        match self {
            Self::Fill { selector, value }
            | Self::SelectOption { selector, value }
            | Self::ReactSelect { selector, value } => {
                validate_selector(selector, &step_context)?;
                validate_workflow_placeholders(value, &step_context)
            }
            Self::Click { selector } | Self::WaitFor { selector } => {
                validate_selector(selector, &step_context)
            }
            Self::Press { selector, key } => {
                validate_selector(selector, &step_context)?;
                if key.trim().is_empty() {
                    return Err(format!("{step_context}: key must not be empty"));
                }
                Ok(())
            }
            Self::Wait { milliseconds } => {
                if !(1..=30_000).contains(milliseconds) {
                    return Err(format!(
                        "{step_context}: wait milliseconds must be between 1 and 30000"
                    ));
                }
                Ok(())
            }
            Self::Evaluate { script, value } => {
                if script.trim().is_empty() {
                    return Err(format!("{step_context}: script must not be empty"));
                }
                if let Some(value) = value {
                    validate_workflow_placeholders(value, &step_context)?;
                }
                Ok(())
            }
        }
    }
}

impl WorkflowRead {
    fn validate(&self, context: &str) -> Result<(), String> {
        if self.selector.trim().is_empty() {
            return Err(format!("{context}: workflow result selector is empty"));
        }
        if self.property == "value"
            || self.property == "text"
            || self.property.starts_with("attribute:")
        {
            Ok(())
        } else {
            Err(format!(
                "{context}: workflow result property must be value, text, or attribute:<name>"
            ))
        }
    }
}

impl P2pAdapterConfig {
    fn validate(&self, has_buy: bool, has_sell: bool, context: &str) -> Result<(), String> {
        validate_http(
            &self.kind,
            &self.endpoint,
            &self.method,
            self.timeout_ms,
            context,
        )?;
        validate_headers(&self.headers, context)?;
        for fiat in &self.supported_fiats {
            if !valid_asset_code(fiat) {
                return Err(format!(
                    "{context}: adapter/p2p/supported_fiats contains invalid code `{fiat}`"
                ));
            }
        }
        for asset in &self.supported_assets {
            if !valid_asset_code(asset) {
                return Err(format!(
                    "{context}: adapter/p2p/supported_assets contains invalid code `{asset}`"
                ));
            }
        }
        for (canonical, remote) in &self.asset_codes {
            if !valid_asset_code(canonical) || !valid_remote_asset_code(remote) {
                return Err(format!(
                    "{context}: adapter/p2p/asset_codes contains an invalid mapping"
                ));
            }
        }
        if let Some(rate_table) = &self.rate_table {
            rate_table.validate(context)?;
        }
        for (field, value) in [
            ("fiat_probe_amount", self.fiat_probe_amount),
            ("asset_probe_amount", self.asset_probe_amount),
            ("default_min_fiat", self.default_min_fiat),
            ("default_max_fiat", self.default_max_fiat),
            ("default_available_asset", self.default_available_asset),
        ] {
            if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                return Err(format!(
                    "{context}: adapter/p2p/{field} must be a positive finite number"
                ));
            }
        }
        if self
            .max_results
            .is_some_and(|limit| !(1..=100).contains(&limit))
        {
            return Err(format!(
                "{context}: adapter/p2p/max_results must be between 1 and 100"
            ));
        }
        if self
            .default_min_fiat
            .zip(self.default_max_fiat)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(format!(
                "{context}: adapter/p2p/default_min_fiat must not exceed default_max_fiat"
            ));
        }
        if has_buy {
            let operation = self
                .buy
                .as_ref()
                .ok_or_else(|| format!("{context}: adapter/p2p/buy is required for [buy]"))?;
            operation.validate(&self.method, self, context, "buy")?;
            if self.rate_table.is_none() {
                operation
                    .offer
                    .as_ref()
                    .or(self.offer.as_ref())
                    .ok_or_else(|| format!("{context}: no offer mapping is configured for buy"))?
                    .validate(self, context)?;
            }
        }
        if has_sell {
            let operation = self
                .sell
                .as_ref()
                .ok_or_else(|| format!("{context}: adapter/p2p/sell is required for [sell]"))?;
            operation.validate(&self.method, self, context, "sell")?;
            if self.rate_table.is_none() {
                operation
                    .offer
                    .as_ref()
                    .or(self.offer.as_ref())
                    .ok_or_else(|| format!("{context}: no offer mapping is configured for sell"))?
                    .validate(self, context)?;
            }
        }
        Ok(())
    }
}

impl RateTableConfig {
    fn validate(&self, context: &str) -> Result<(), String> {
        validate_pointers(
            [
                Some(self.fiat_items_pointer.as_str()),
                Some(self.fiat_code_pointer.as_str()),
                Some(self.fiat_rate_pointer.as_str()),
                Some(self.asset_items_pointer.as_str()),
                Some(self.asset_code_pointer.as_str()),
                Some(self.asset_rates_pointer.as_str()),
            ],
            context,
        )?;
        for (canonical, remote) in &self.fiat_codes {
            if !valid_asset_code(canonical) || !valid_remote_asset_code(remote) {
                return Err(format!(
                    "{context}: adapter/p2p/rate_table/fiat_codes is invalid"
                ));
            }
        }
        if self
            .source_url
            .as_ref()
            .is_some_and(|url| !url.starts_with("https://") && !url.starts_with("http://"))
        {
            return Err(format!(
                "{context}: adapter/p2p/rate_table/source_url must use http or https"
            ));
        }
        Ok(())
    }
}

impl P2pOperation {
    fn validate(
        &self,
        method: &str,
        adapter: &P2pAdapterConfig,
        context: &str,
        operation: &str,
    ) -> Result<(), String> {
        if let Some(endpoint) = &self.endpoint {
            validate_http(&adapter.kind, endpoint, method, adapter.timeout_ms, context)?;
            validate_request_placeholders(endpoint, context)?;
        }
        if let Some(auth) = &adapter.auth {
            auth.validate(context)?;
        }
        validate_request(method, &self.query, self.request_json.as_deref(), context)?;
        if !matches!(
            self.amount_mode.as_str(),
            "query_or_empty" | "fiat_probe" | "asset_probe"
        ) {
            return Err(format!(
                "{context}: adapter/p2p/{operation}/amount_mode is invalid"
            ));
        }
        if self.amount_mode == "fiat_probe" && adapter.fiat_probe_amount.is_none() {
            return Err(format!(
                "{context}: adapter/p2p/fiat_probe_amount is required by {operation}"
            ));
        }
        if self.amount_mode == "asset_probe" && adapter.asset_probe_amount.is_none() {
            return Err(format!(
                "{context}: adapter/p2p/asset_probe_amount is required by {operation}"
            ));
        }
        let mapping = self.offer.as_ref().or(adapter.offer.as_ref());
        let fee_amount_mode = match operation {
            "buy" => "fiat_probe",
            "sell" => "asset_probe",
            _ => unreachable!("validated operation name"),
        };
        if mapping.is_some_and(|mapping| mapping.output_fee_pointer.is_some())
            && self.amount_mode != fee_amount_mode
        {
            return Err(format!(
                "{context}: adapter/p2p/{operation}/amount_mode must be {fee_amount_mode} when output_fee_pointer is set"
            ));
        }
        validate_success_pair(
            self.success_pointer.as_deref(),
            self.success_value.as_deref(),
            context,
        )?;
        validate_pointers(
            [
                self.items_pointer.as_deref(),
                self.success_pointer.as_deref(),
                self.error_pointer.as_deref(),
            ],
            context,
        )
    }
}

impl OfferMapping {
    fn validate(&self, adapter: &P2pAdapterConfig, context: &str) -> Result<(), String> {
        let amount_pair = self.fiat_amount_pointer.is_some() && self.asset_amount_pointer.is_some();
        if self.price_pointer.is_none() && !amount_pair {
            return Err(format!(
                "{context}: adapter/p2p/offer requires price_pointer or both amount pointers"
            ));
        }
        if self.fiat_amount_pointer.is_some() != self.asset_amount_pointer.is_some() {
            return Err(format!(
                "{context}: fiat_amount_pointer and asset_amount_pointer must be set together"
            ));
        }
        for (pointer, fallback, field) in [
            (
                self.available_asset_pointer.as_ref(),
                adapter.default_available_asset,
                "available asset",
            ),
            (
                self.min_fiat_pointer.as_ref(),
                adapter.default_min_fiat,
                "minimum fiat",
            ),
            (
                self.max_fiat_pointer.as_ref(),
                adapter.default_max_fiat,
                "maximum fiat",
            ),
        ] {
            if pointer.is_none() && fallback.is_none() {
                return Err(format!(
                    "{context}: adapter/p2p/offer needs a pointer or default for {field}"
                ));
            }
        }
        validate_pointers(
            [
                self.ad_id_pointer.as_deref(),
                self.fiat_pointer.as_deref(),
                self.asset_pointer.as_deref(),
                self.price_pointer.as_deref(),
                self.fiat_amount_pointer.as_deref(),
                self.asset_amount_pointer.as_deref(),
                self.output_fee_pointer.as_deref(),
                self.available_asset_pointer.as_deref(),
                self.min_fiat_pointer.as_deref(),
                self.max_fiat_pointer.as_deref(),
                self.payment_methods_pointer.as_deref(),
                self.payment_method_value_pointer.as_deref(),
                self.payment_method_fallback_pointer.as_deref(),
                self.pay_time_limit_pointer.as_deref(),
                self.advertiser_id_pointer.as_deref(),
                self.advertiser_nickname_pointer.as_deref(),
                self.advertiser_user_type_pointer.as_deref(),
                self.completed_orders_pointer.as_deref(),
                self.completion_rate_pointer.as_deref(),
                self.positive_rate_pointer.as_deref(),
            ],
            context,
        )?;
        for condition in self
            .merchant_conditions
            .iter()
            .chain(self.verified_conditions.iter())
        {
            condition.validate(context)?;
        }
        Ok(())
    }
}

impl ValueCondition {
    fn validate(&self, context: &str) -> Result<(), String> {
        validate_pointer(&self.pointer, context)?;
        match self.operator.as_str() {
            "truthy" | "non_empty" if self.value.is_none() => Ok(()),
            "equals" | "equals_ci" | "not_equals" | "not_equals_ci" if self.value.is_some() => {
                Ok(())
            }
            _ => Err(format!(
                "{context}: invalid condition operator/value combination"
            )),
        }
    }
}

impl MarketAdapterConfig {
    fn validate(&self, context: &str) -> Result<(), String> {
        validate_http(
            &self.kind,
            &self.endpoint,
            &self.method,
            self.timeout_ms,
            context,
        )?;
        validate_headers(&self.headers, context)?;
        validate_request(
            &self.method,
            &self.query,
            self.request_json.as_deref(),
            context,
        )?;
        validate_success_pair(
            self.success_pointer.as_deref(),
            self.success_value.as_deref(),
            context,
        )?;
        validate_pointers(
            [
                self.items_pointer.as_deref(),
                Some(self.symbol_pointer.as_str()),
                Some(self.bid_pointer.as_str()),
                Some(self.ask_pointer.as_str()),
                self.success_pointer.as_deref(),
                self.error_pointer.as_deref(),
            ],
            context,
        )
    }
}

pub fn environment_variable(value: &str) -> Option<&str> {
    value
        .strip_prefix("{{env:")
        .and_then(|value| value.strip_suffix("}}"))
}

fn validate_http(
    kind: &str,
    endpoint: &str,
    method: &str,
    timeout_ms: u64,
    context: &str,
) -> Result<(), String> {
    if kind != "http_json" {
        return Err(format!("{context}: adapter kind must be `http_json`"));
    }
    if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
        return Err(format!(
            "{context}: adapter endpoint must use http or https"
        ));
    }
    if !matches!(method, "GET" | "POST") {
        return Err(format!("{context}: adapter method must be GET or POST"));
    }
    if !(250..=30_000).contains(&timeout_ms) {
        return Err(format!(
            "{context}: adapter timeout_ms must be between 250 and 30000"
        ));
    }
    Ok(())
}

fn validate_headers(headers: &BTreeMap<String, String>, context: &str) -> Result<(), String> {
    for (name, value) in headers {
        if name.parse::<reqwest::header::HeaderName>().is_err() {
            return Err(format!("{context}: invalid adapter header name `{name}`"));
        }
        if let Some(variable) = environment_variable(value) {
            if !valid_environment_variable(variable) {
                return Err(format!(
                    "{context}: invalid environment variable in header `{name}`"
                ));
            }
        } else if value.parse::<reqwest::header::HeaderValue>().is_err() {
            return Err(format!(
                "{context}: invalid adapter header value for `{name}`"
            ));
        }
    }
    Ok(())
}

fn validate_request(
    method: &str,
    query: &BTreeMap<String, String>,
    request_json: Option<&str>,
    context: &str,
) -> Result<(), String> {
    if method == "POST" && request_json.is_none() {
        return Err(format!("{context}: POST adapter requires request_json"));
    }
    if let Some(request_json) = request_json {
        serde_json::from_str::<serde_json::Value>(request_json)
            .map_err(|error| format!("{context}: request_json is invalid JSON: {error}"))?;
    }
    for template in query
        .values()
        .map(String::as_str)
        .chain(request_json.into_iter())
    {
        validate_request_placeholders(template, context)?;
    }
    Ok(())
}

fn validate_request_placeholders(template: &str, context: &str) -> Result<(), String> {
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            return Err(format!("{context}: unclosed request placeholder"));
        };
        let placeholder = &after_start[..end];
        if !REQUEST_PLACEHOLDERS.contains(&placeholder) {
            return Err(format!(
                "{context}: unknown request placeholder `{{{{{placeholder}}}}}`"
            ));
        }
        remaining = &after_start[end + 2..];
    }
    Ok(())
}

fn validate_workflow_placeholders(template: &str, context: &str) -> Result<(), String> {
    let mut remaining = template;
    while let Some(start) = remaining.find("{{") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            return Err(format!("{context}: unclosed workflow placeholder"));
        };
        let placeholder = &after_start[..end];
        if !matches!(placeholder, "fiat" | "asset" | "amount") {
            return Err(format!(
                "{context}: unknown workflow placeholder `{{{{{placeholder}}}}}`"
            ));
        }
        remaining = &after_start[end + 2..];
    }
    Ok(())
}

fn validate_selector(selector: &str, context: &str) -> Result<(), String> {
    if selector.trim().is_empty() {
        Err(format!("{context}: selector must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_success_pair(
    pointer: Option<&str>,
    value: Option<&str>,
    context: &str,
) -> Result<(), String> {
    if pointer.is_some() != value.is_some() {
        return Err(format!(
            "{context}: success_pointer and success_value must be set together"
        ));
    }
    Ok(())
}

fn validate_pointers<'a>(
    pointers: impl IntoIterator<Item = Option<&'a str>>,
    context: &str,
) -> Result<(), String> {
    for pointer in pointers.into_iter().flatten() {
        validate_pointer(pointer, context)?;
    }
    Ok(())
}

fn validate_pointer(pointer: &str, context: &str) -> Result<(), String> {
    if pointer.starts_with('/') {
        Ok(())
    } else {
        Err(format!(
            "{context}: JSON pointer `{pointer}` must begin with `/`"
        ))
    }
}

fn valid_asset_code(value: &str) -> bool {
    (2..=12).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_remote_asset_code(value: &str) -> bool {
    (2..=32).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn valid_environment_variable(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

impl HmacAuthConfig {
    fn validate(&self, context: &str) -> Result<(), String> {
        if self.kind != "hmac_sha512_timestamp_body" {
            return Err(format!(
                "{context}: adapter/p2p/auth kind must be `hmac_sha512_timestamp_body`"
            ));
        }
        for (field, value) in [
            ("public_key_env", self.public_key_env.as_str()),
            ("private_key_env", self.private_key_env.as_str()),
        ] {
            if !valid_environment_variable(value) {
                return Err(format!(
                    "{context}: adapter/p2p/auth/{field} is not a valid environment variable"
                ));
            }
        }
        for (field, value) in [
            ("public_key_header", self.public_key_header.as_str()),
            ("timestamp_header", self.timestamp_header.as_str()),
            ("signature_header", self.signature_header.as_str()),
        ] {
            if value.parse::<reqwest::header::HeaderName>().is_err() {
                return Err(format!(
                    "{context}: invalid adapter/p2p/auth header `{field}`"
                ));
            }
        }
        Ok(())
    }
}

fn default_method() -> String {
    "GET".into()
}

fn default_public_key_header() -> String {
    "ApiPublic".into()
}

fn default_private_timestamp_header() -> String {
    "Timestamp".into()
}

fn default_signature_header() -> String {
    "Signature".into()
}

fn default_timeout_ms() -> u64 {
    10_000
}

fn default_bestchange_language() -> String {
    "en".into()
}

fn default_bestchange_max_results() -> usize {
    100
}

fn default_navigation_retry_delay_ms() -> u64 {
    500
}

fn default_amount_mode() -> String {
    "query_or_empty".into()
}

fn default_read_property() -> String {
    "value".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_environment_backed_headers() {
        assert_eq!(
            environment_variable("{{env:QUOTE_API_KEY}}"),
            Some("QUOTE_API_KEY")
        );
        assert_eq!(environment_variable("Bearer public-value"), None);
    }
}

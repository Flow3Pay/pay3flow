//! Pure data model shared by the build-time Trustfile validator and the
//! compiled registry. It deliberately has no dependency on the rest of the
//! crate so `build.rs` can include it directly.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Trustfile {
    pub schema: u32,
    pub provider: String,
    pub trust: Trust,
    pub quote: Request,
    pub build: Build,
    pub submit: Submit,
    pub status: Request,
    pub networks: Networks,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Trust {
    pub state: TrustState,
    pub transport: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TrustState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub request: Option<String>,
    #[serde(default)]
    pub response: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub kind: BuildKind,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub approval_field: Option<String>,
    #[serde(default)]
    pub expiry_field: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BuildKind {
    ProviderTransaction,
    DepositAddress,
    HttpOrder,
    Unsupported,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    pub kind: SubmitKind,
    pub reference: ReferenceKind,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SubmitKind {
    WalletTransaction,
    WalletDeposit,
    WalletOrder,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    TransactionHash,
    OrderUid,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Networks {
    pub source: Vec<String>,
    pub destination: Vec<String>,
}

impl Trustfile {
    /// A Trustfile enables wallet execution only when it describes a complete
    /// quote → build → wallet submit → status path.
    pub fn executable(&self) -> bool {
        self.trust.state == TrustState::Enabled
            && self.build.kind != BuildKind::Unsupported
            && self.submit.kind != SubmitKind::Unsupported
    }

    pub fn validate(&self, expected_slug: &str) -> Result<(), String> {
        if self.schema != 1 {
            return Err(format!("schema must be 1, got {}", self.schema));
        }
        if self.provider != expected_slug {
            return Err(format!(
                "provider must match its directory slug `{expected_slug}`"
            ));
        }
        if !valid_slug(&self.provider) {
            return Err("provider must contain only a-z, 0-9, '-' or '_'".into());
        }
        if self.trust.transport != "http-json" {
            return Err("trust.transport must be `http-json`".into());
        }
        if self.trust.state == TrustState::Enabled && !self.executable() {
            return Err("enabled Trustfiles need a supported build and submit path".into());
        }
        validate_request("quote", &self.quote)?;
        validate_request("status", &self.status)?;
        let fields = &self.build.fields;
        if self.build.kind == BuildKind::ProviderTransaction {
            for required in ["to", "data", "value"] {
                if !fields.iter().any(|field| field == required) {
                    return Err(format!("build.fields must include `{required}`"));
                }
            }
            if self.build.approval_field.as_deref().is_none_or(str::is_empty) {
                return Err("build.approval_field is required for provider transactions".into());
            }
        }
        if self.build.kind == BuildKind::DepositAddress && fields.is_empty() {
            return Err("build.fields must name the deposit address field".into());
        }
        if self.networks.source.is_empty() || self.networks.destination.is_empty() {
            return Err("networks.source and networks.destination cannot be empty".into());
        }
        for network in self
            .networks
            .source
            .iter()
            .chain(self.networks.destination.iter())
        {
            if !valid_slug(network) {
                return Err(format!("invalid network id `{network}`"));
            }
        }
        Ok(())
    }
}

fn validate_request(section: &str, request: &Request) -> Result<(), String> {
    let method = request.method.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST") {
        return Err(format!("{section}.method must be GET or POST"));
    }
    if !(request.url.starts_with("https://") || request.url.starts_with("http://")) {
        return Err(format!("{section}.url must use http or https"));
    }
    if request
        .request
        .as_deref()
        .is_some_and(|value| !matches!(value, "json" | "query"))
    {
        return Err(format!("{section}.request must be `json` or `query`"));
    }
    Ok(())
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

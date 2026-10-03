use chrono::{DateTime, Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::db::DbPool;
use crate::p2p::{P2pRoute, RouteExecutionDescriptor};
use crate::route_engine::{
    Amount, Asset, CowRouteProvider, NearIntentsProvider, NearQuoteRequest, SymbiosisRouteProvider,
};

const DESCRIPTOR_TTL_MINUTES: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionFee {
    pub asset: String,
    pub amount: String,
}

#[derive(Debug, Error)]
pub enum RouteExecutionError {
    #[error("wallet execution is disabled")]
    Disabled,
    #[error("invalid or expired route execution token")]
    InvalidToken,
    #[error("unsupported executable route")]
    UnsupportedRoute,
    #[error("invalid execution amount")]
    InvalidAmount,
    #[error("route execution was not found")]
    NotFound,
    #[error("invalid execution submission")]
    InvalidSubmission,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<tokio_postgres::Error> for RouteExecutionError {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<deadpool_postgres::PoolError> for RouteExecutionError {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Internal(error.into())
    }
}

impl From<serde_json::Error> for RouteExecutionError {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionClaims {
    route_id: String,
    provider: String,
    from_asset: String,
    to_asset: String,
    input_amount: String,
    exp: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRouteExecution {
    pub anonymous_id: Uuid,
    pub route_token: String,
    pub source_address: String,
    pub recipient: String,
    pub refund_to: Option<String>,
    pub amount: Option<String>,
    pub slippage_bps: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitRouteExecution {
    pub anonymous_id: Uuid,
    pub reference: String,
    pub kind: SubmissionKind,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionKind {
    TransactionHash,
    OrderUid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionAction {
    NearDeposit {
        network: String,
        asset: String,
        amount: String,
        deposit_address: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        deposit_memo: Option<String>,
        asset_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        decimals: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_contract: Option<String>,
        #[serde(default)]
        expected_output: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_fee: Option<ExecutionFee>,
    },
    CowOrder {
        chain: String,
        chain_id: u64,
        api_url: String,
        sell_token: String,
        buy_token: String,
        sell_amount: String,
        #[serde(default)]
        expected_output: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_fee: Option<ExecutionFee>,
        quote: Value,
    },
    SymbiosisTransaction {
        chain_id: u64,
        source_token: String,
        input_amount: String,
        #[serde(default)]
        expected_output: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_fee: Option<ExecutionFee>,
        #[serde(skip_serializing_if = "Option::is_none")]
        approval_spender: Option<String>,
        transaction: Value,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteExecutionView {
    pub id: Uuid,
    pub route_id: String,
    pub provider: String,
    pub status: String,
    pub from_asset: String,
    pub to_asset: String,
    pub input_amount: String,
    pub expected_output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_fee: Option<ExecutionFee>,
    pub source_address: String,
    pub recipient: String,
    pub action: ExecutionAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_status: Option<Value>,
    pub quote_expires_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct RouteExecutionService {
    pool: DbPool,
    encode_key: EncodingKey,
    decode_key: DecodingKey,
    enabled: bool,
    near: NearIntentsProvider,
    cow: Option<CowRouteProvider>,
    symbiosis: SymbiosisRouteProvider,
}

impl RouteExecutionService {
    pub fn new(
        pool: DbPool,
        secret: &str,
        enabled: bool,
        near: NearIntentsProvider,
        cow: Option<CowRouteProvider>,
        symbiosis: SymbiosisRouteProvider,
    ) -> Self {
        Self {
            pool,
            encode_key: EncodingKey::from_secret(secret.as_bytes()),
            decode_key: DecodingKey::from_secret(secret.as_bytes()),
            enabled,
            near,
            cow,
            symbiosis,
        }
    }

    pub fn attach_descriptor(&self, route: &mut P2pRoute) {
        if !self.enabled || route.execution.is_some() {
            return;
        }
        let Some(provider) = route.route_provider.as_deref() else {
            return;
        };
        // Symbiosis remains discoverable as a quote provider, but production
        // execution requires deployment-owned contract allowlists that are not
        // currently configured.
        if !matches!(provider, "near-intents") && !(provider == "cow-swap" && self.cow.is_some()) {
            return;
        }
        let assets = route
            .route_path
            .iter()
            .filter(|part| part.contains('@'))
            .collect::<Vec<_>>();
        let (Some(from_asset), Some(to_asset), Some(input_amount)) = (
            assets.first(),
            assets.last(),
            route.provider_input_amount.as_ref(),
        ) else {
            return;
        };
        if from_asset == to_asset {
            return;
        }
        let expires_at = Utc::now() + ChronoDuration::minutes(DESCRIPTOR_TTL_MINUTES);
        let claims = ExecutionClaims {
            route_id: route.route_id.clone(),
            provider: provider.to_string(),
            from_asset: (*from_asset).clone(),
            to_asset: (*to_asset).clone(),
            input_amount: input_amount.clone(),
            exp: usize::try_from(expires_at.timestamp()).unwrap_or(usize::MAX),
        };
        let Ok(token) = encode(&Header::default(), &claims, &self.encode_key) else {
            return;
        };
        route.execution = Some(RouteExecutionDescriptor {
            provider: claims.provider,
            from_asset: claims.from_asset,
            to_asset: claims.to_asset,
            input_amount: claims.input_amount,
            expires_at,
            token,
        });
    }

    pub async fn create(
        &self,
        request: CreateRouteExecution,
        idempotency_key: &str,
    ) -> Result<RouteExecutionView, RouteExecutionError> {
        if !self.enabled {
            return Err(RouteExecutionError::Disabled);
        }
        if idempotency_key.trim().is_empty()
            || idempotency_key.len() > 128
            || request.source_address.trim().is_empty()
            || request.recipient.trim().is_empty()
        {
            return Err(RouteExecutionError::InvalidSubmission);
        }
        if let Some(existing) = self
            .find_by_idempotency(request.anonymous_id, idempotency_key)
            .await?
        {
            return Ok(existing);
        }
        let claims = decode::<ExecutionClaims>(
            &request.route_token,
            &self.decode_key,
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| RouteExecutionError::InvalidToken)?
        .claims;
        let input_amount = request.amount.unwrap_or(claims.input_amount.clone());
        positive_amount(&claims.input_amount)?;
        positive_amount(&input_amount)?;
        if canonical_decimal(&input_amount) != canonical_decimal(&claims.input_amount) {
            return Err(RouteExecutionError::InvalidAmount);
        }
        let from =
            Asset::parse(&claims.from_asset).map_err(|_| RouteExecutionError::InvalidToken)?;
        let to = Asset::parse(&claims.to_asset).map_err(|_| RouteExecutionError::InvalidToken)?;
        let amount = Amount::new(input_amount.clone(), from.clone())
            .map_err(|_| RouteExecutionError::InvalidAmount)?;
        let (action, provider_reference, quote_expires_at) = match claims.provider.as_str() {
            "near-intents" => {
                let quote = self
                    .near
                    .executable_quote(NearQuoteRequest {
                        from: from.clone(),
                        to,
                        amount,
                        recipient: request.recipient.clone(),
                        refund_to: request
                            .refund_to
                            .clone()
                            .unwrap_or_else(|| request.source_address.clone()),
                        slippage_bps: request.slippage_bps.unwrap_or(100).min(10_000),
                        dry: false,
                    })
                    .await?;
                let deposit_address = quote
                    .deposit_address
                    .clone()
                    .ok_or(RouteExecutionError::UnsupportedRoute)?;
                let token = self.near.token_for_asset(&from).await?;
                let deposit_memo = first_string(&quote.raw, &["depositMemo", "deposit_memo"]);
                let expires_at = quote
                    .expires_at
                    .unwrap_or_else(|| Utc::now() + ChronoDuration::minutes(3));
                (
                    ExecutionAction::NearDeposit {
                        network: from.location.clone().unwrap_or_default(),
                        asset: from.to_string(),
                        amount: quote.input.value,
                        expected_output: quote.output.value,
                        expected_fee: quote.fee.map(|fee| ExecutionFee {
                            asset: fee.asset.to_string(),
                            amount: fee.value,
                        }),
                        deposit_address: deposit_address.clone(),
                        deposit_memo,
                        asset_id: token.asset_id,
                        decimals: token.decimals,
                        token_contract: token.contract_address,
                    },
                    Some(deposit_address),
                    expires_at,
                )
            }
            "cow-swap" => {
                let fee_asset = from.to_string();
                let quote = self
                    .cow
                    .as_ref()
                    .ok_or(RouteExecutionError::UnsupportedRoute)?
                    .execution_quote(
                        from,
                        to,
                        amount,
                        &request.source_address,
                        &request.recipient,
                    )
                    .await?;
                let expires_at = quote.expires_at;
                (
                    ExecutionAction::CowOrder {
                        chain: quote.chain,
                        chain_id: quote.chain_id,
                        api_url: quote.api_url,
                        sell_token: quote.sell_token,
                        buy_token: quote.buy_token,
                        sell_amount: quote.sell_amount,
                        expected_output: quote.expected_output,
                        expected_fee: quote.expected_fee.map(|amount| ExecutionFee {
                            asset: fee_asset,
                            amount,
                        }),
                        quote: quote.quote,
                    },
                    None,
                    expires_at,
                )
            }
            "symbiosis" => {
                let quote = self
                    .symbiosis
                    .execution_quote(
                        from,
                        to,
                        amount,
                        &request.source_address,
                        &request.recipient,
                    )
                    .await?;
                let expires_at = quote.expires_at;
                (
                    ExecutionAction::SymbiosisTransaction {
                        chain_id: quote.source_chain_id,
                        source_token: quote.source_token,
                        input_amount: quote.input_amount,
                        expected_output: quote.expected_output,
                        expected_fee: None,
                        approval_spender: quote.approval_spender,
                        transaction: quote.transaction,
                    },
                    None,
                    expires_at,
                )
            }
            _ => return Err(RouteExecutionError::UnsupportedRoute),
        };
        let id = Uuid::new_v4();
        let action_json = serde_json::to_value(&action)?;
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
INSERT INTO route_executions
    (id, anonymous_id, idempotency_key, route_id, provider, status,
     from_asset, to_asset, input_amount, source_address, recipient,
     action, provider_reference, quote_expires_at)
VALUES ($1, $2, $3, $4, $5, 'awaiting_signature', $6, $7, $8, $9, $10, $11, $12, $13)
"#,
                &[
                    &id,
                    &request.anonymous_id,
                    &idempotency_key,
                    &claims.route_id,
                    &claims.provider,
                    &claims.from_asset,
                    &claims.to_asset,
                    &input_amount,
                    &request.source_address,
                    &request.recipient,
                    &action_json,
                    &provider_reference,
                    &quote_expires_at,
                ],
            )
            .await?;
        self.get(id, request.anonymous_id).await
    }

    pub async fn submit(
        &self,
        id: Uuid,
        request: SubmitRouteExecution,
    ) -> Result<RouteExecutionView, RouteExecutionError> {
        let reference = request.reference.trim();
        if reference.is_empty()
            || reference.len() > 160
            || !reference.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(RouteExecutionError::InvalidSubmission);
        }
        let current = self.get_stored(id, request.anonymous_id).await?;
        let valid_kind = matches!(
            (current.provider.as_str(), request.kind),
            ("cow-swap", SubmissionKind::OrderUid)
                | (
                    "near-intents" | "symbiosis",
                    SubmissionKind::TransactionHash
                )
        );
        if !valid_kind || current.status != "awaiting_signature" {
            return Err(RouteExecutionError::InvalidSubmission);
        }
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
UPDATE route_executions
SET status = 'submitted', submitted_reference = $3, updated_at = now()
WHERE id = $1 AND anonymous_id = $2
"#,
                &[&id, &request.anonymous_id, &reference],
            )
            .await?;
        if let ExecutionAction::NearDeposit {
            deposit_address, ..
        } = &current.action
        {
            if let Err(error) = self
                .near
                .submit_deposit_tx(reference, deposit_address)
                .await
            {
                tracing::warn!(
                    execution_id = %id,
                    error = %error,
                    "NEAR Intents deposit notification failed"
                );
            }
        }
        self.get(id, request.anonymous_id).await
    }

    pub async fn get(
        &self,
        id: Uuid,
        anonymous_id: Uuid,
    ) -> Result<RouteExecutionView, RouteExecutionError> {
        let mut view = self.get_stored(id, anonymous_id).await?;
        if view.status != "submitted" {
            return Ok(view);
        }
        let Some(reference) = view.submitted_reference.as_deref() else {
            return Ok(view);
        };
        let provider_status: anyhow::Result<Value> = match (&view.action, view.provider.as_str()) {
            (
                ExecutionAction::NearDeposit {
                    deposit_address,
                    deposit_memo,
                    ..
                },
                "near-intents",
            ) => self
                .near
                .status_with_memo(deposit_address, deposit_memo.as_deref())
                .await
                .and_then(|status| serde_json::to_value(status).map_err(Into::into)),
            (ExecutionAction::CowOrder { chain, .. }, "cow-swap") => {
                self.cow
                    .as_ref()
                    .ok_or(RouteExecutionError::UnsupportedRoute)?
                    .order_status(chain, reference)
                    .await
            }
            (ExecutionAction::SymbiosisTransaction { chain_id, .. }, "symbiosis") => {
                self.symbiosis
                    .transaction_status(*chain_id, reference)
                    .await
            }
            _ => return Err(RouteExecutionError::UnsupportedRoute),
        };
        let provider_status = match provider_status {
            Ok(status) => status,
            Err(error) => {
                tracing::warn!(
                    execution_id = %id,
                    provider = %view.provider,
                    error = %error,
                    "provider execution status poll failed"
                );
                return Ok(view);
            }
        };
        let status = normalized_status(&view.provider, &provider_status);
        let client = self.pool.get().await?;
        client
            .execute(
                r#"
UPDATE route_executions
SET status = $3, provider_status = $4, updated_at = now()
WHERE id = $1 AND anonymous_id = $2
"#,
                &[&id, &anonymous_id, &status, &provider_status],
            )
            .await?;
        view.status = status.into();
        view.provider_status = Some(provider_status);
        view.updated_at = Utc::now();
        Ok(view)
    }

    async fn find_by_idempotency(
        &self,
        anonymous_id: Uuid,
        key: &str,
    ) -> Result<Option<RouteExecutionView>, RouteExecutionError> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT id FROM route_executions WHERE anonymous_id = $1 AND idempotency_key = $2",
                &[&anonymous_id, &key],
            )
            .await?;
        match row {
            Some(row) => Ok(Some(self.get_stored(row.get(0), anonymous_id).await?)),
            None => Ok(None),
        }
    }

    async fn get_stored(
        &self,
        id: Uuid,
        anonymous_id: Uuid,
    ) -> Result<RouteExecutionView, RouteExecutionError> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                r#"
SELECT route_id, provider, status, from_asset, to_asset, input_amount,
       source_address, recipient, action, provider_reference,
       submitted_reference, provider_status, quote_expires_at, updated_at
FROM route_executions
WHERE id = $1 AND anonymous_id = $2
"#,
                &[&id, &anonymous_id],
            )
            .await?
            .ok_or(RouteExecutionError::NotFound)?;
        let action: ExecutionAction = serde_json::from_value(row.get(8))?;
        let expected_output = match &action {
            ExecutionAction::NearDeposit {
                expected_output, ..
            }
            | ExecutionAction::CowOrder {
                expected_output, ..
            }
            | ExecutionAction::SymbiosisTransaction {
                expected_output, ..
            } => expected_output.clone(),
        };
        let expected_fee = match &action {
            ExecutionAction::NearDeposit { expected_fee, .. }
            | ExecutionAction::CowOrder { expected_fee, .. }
            | ExecutionAction::SymbiosisTransaction { expected_fee, .. } => expected_fee.clone(),
        };
        Ok(RouteExecutionView {
            id,
            route_id: row.get(0),
            provider: row.get(1),
            status: row.get(2),
            from_asset: row.get(3),
            to_asset: row.get(4),
            input_amount: row.get(5),
            expected_output,
            expected_fee,
            source_address: row.get(6),
            recipient: row.get(7),
            action,
            provider_reference: row.get(9),
            submitted_reference: row.get(10),
            provider_status: row.get(11),
            quote_expires_at: row.get(12),
            updated_at: row.get(13),
        })
    }
}

fn positive_amount(value: &str) -> Result<f64, RouteExecutionError> {
    value
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite() && *amount > 0.0)
        .ok_or(RouteExecutionError::InvalidAmount)
}

fn canonical_decimal(value: &str) -> Option<String> {
    let value = value.trim();
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    let fraction = fraction.trim_end_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    Some(if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .or_else(|| value.get("quote").and_then(|quote| quote.get(*key)))
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn normalized_status(provider: &str, value: &Value) -> &'static str {
    let raw = value
        .get("status")
        .or_else(|| value.get("state"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match provider {
        "cow-swap" if matches!(raw.as_str(), "fulfilled" | "presignaturepending") => {
            if raw == "fulfilled" {
                "completed"
            } else {
                "submitted"
            }
        }
        "near-intents" if matches!(raw.as_str(), "success" | "completed" | "processed") => {
            "completed"
        }
        "near-intents" if raw == "incomplete_deposit" => "failed",
        "symbiosis" if matches!(raw.as_str(), "success" | "completed") => "completed",
        _ if matches!(raw.as_str(), "refunded" | "reverted") => "refunded",
        _ if matches!(raw.as_str(), "failed" | "cancelled" | "expired" | "stuck") => {
            raw_status(&raw)
        }
        _ => "submitted",
    }
}

fn raw_status(status: &str) -> &'static str {
    match status {
        "cancelled" => "cancelled",
        "expired" => "expired",
        "stuck" => "stuck",
        _ => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_statuses_map_to_terminal_execution_states() {
        let cases = [
            ("cow-swap", json!({"status":"fulfilled"}), "completed"),
            ("near-intents", json!({"status":"SUCCESS"}), "completed"),
            (
                "near-intents",
                json!({"status":"INCOMPLETE_DEPOSIT"}),
                "failed",
            ),
            ("symbiosis", json!({"status":"Stuck"}), "stuck"),
            ("symbiosis", json!({"status":"Pending"}), "submitted"),
        ];
        for (provider, status, expected) in cases {
            assert_eq!(
                normalized_status(provider, &status),
                expected,
                "{provider}: {status}"
            );
        }
    }

    #[test]
    fn amount_validation_rejects_zero_negative_and_non_finite_values() {
        for value in ["0", "-1", "NaN", "inf", "not-a-number"] {
            assert!(positive_amount(value).is_err(), "value: {value}");
        }
        assert_eq!(positive_amount("1.25").unwrap(), 1.25);
    }

    #[test]
    fn decimal_comparison_is_exact_and_ignores_only_formatting() {
        assert_eq!(canonical_decimal("001.2500"), canonical_decimal("1.25"));
        assert_ne!(
            canonical_decimal("1.2500000000000001"),
            canonical_decimal("1.25")
        );
        assert_eq!(canonical_decimal("1e3"), None);
    }
}

//! B2B crypto routing API v1.
//!
//! A business asks Pay3Flow to move value from A to B. The router quotes only
//! executable sources (NEAR Intents in v1), the business funds a per-order
//! deposit address, and the platform keeps a capped margin embedded in
//! `appFees` instead of charging a visible commission.
//!
//! The service deliberately stores money as text: provider quotes, business
//! amounts and the fee ledger are exact decimal strings.

pub mod key_coverage;
pub mod keys;
pub mod model;
pub mod page;
pub mod qr;
pub mod webhook;
mod worker;

use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use deadpool_postgres::GenericClient;
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::crypto::SecretBox;
use crate::db::DbPool;
use crate::route_engine::{Amount, AppFee, Asset, NearIntentsProvider, NearQuoteRequest};

use self::model::{
    AnalyticsView, AssetAmount, CheckoutStatusView, CheckoutView, CreateCheckoutRequest,
    CreateTransferRequest, FailureReason, QuoteRequest, QuoteView, TransferEventView, TransferView,
};

/// Every webhook kind v1 can emit to a merchant endpoint.
pub const WEBHOOK_EVENTS: &[&str] = &[
    "order.quoted",
    "order.awaiting_deposit",
    "order.processing",
    "order.paid",
    "order.underpaid",
    "order.overpaid",
    "order.expired",
    "order.failed",
    "order.refunded",
    "order.cancelled",
];

const DEFAULT_MARGIN_CAP_BPS: u32 = 70;
const MAX_QUOTE_TTL_SECS: u64 = 60 * 60;
const MAX_WEBHOOK_SECRET_BYTES: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("transfer not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<tokio_postgres::Error> for Error {
    fn from(error: tokio_postgres::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<deadpool_postgres::PoolError> for Error {
    fn from(error: deadpool_postgres::PoolError) -> Self {
        Self::Internal(error.into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Exact {
    Input,
    Output,
}

impl Exact {
    fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
}

#[derive(Clone)]
pub struct Service {
    pool: DbPool,
    near: NearIntentsProvider,
    secrets: SecretBox,
    fee_recipient: Option<String>,
    margin_bps: u32,
    margin_cap_bps: u32,
    quote_ttl: Duration,
    public_origin: String,
}

struct PreparedQuote {
    exact: Exact,
    input: Amount,
    output: Amount,
    best_output: String,
    captured_bps: i32,
    app_fee_bps: i32,
    deposit_address: String,
    refund_address: String,
    provider_quote_id: String,
    expires_at: DateTime<Utc>,
    raw: Value,
}

struct QuoteTarget {
    from: Asset,
    to: Asset,
    exact: Exact,
    amount: Amount,
    recipient: String,
    refund_address: String,
}

impl Service {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: DbPool,
        near: NearIntentsProvider,
        secrets_key: &str,
        fee_recipient: Option<String>,
        margin_bps: u32,
        margin_cap_bps: u32,
        quote_ttl: Duration,
        public_origin: String,
    ) -> Self {
        let margin_cap_bps = margin_cap_bps.min(DEFAULT_MARGIN_CAP_BPS);
        Self {
            pool,
            near,
            secrets: SecretBox::new(secrets_key),
            fee_recipient: fee_recipient
                .map(|recipient| recipient.trim().to_string())
                .filter(|recipient| !recipient.is_empty()),
            margin_bps: margin_bps.min(margin_cap_bps),
            margin_cap_bps,
            quote_ttl: Duration::from_secs(quote_ttl.as_secs().clamp(60, MAX_QUOTE_TTL_SECS)),
            public_origin: public_origin.trim_end_matches('/').to_string(),
        }
    }

    /// Effective quote TTL. The provider may return a longer deadline than the
    /// request asked for, so the router always caps it locally.
    pub fn quote_ttl(&self) -> Duration {
        self.quote_ttl
    }

    pub fn margin_bps(&self) -> u32 {
        if self.fee_recipient.is_some() {
            self.margin_bps.min(self.margin_cap_bps)
        } else {
            0
        }
    }

    pub fn fee_recipient(&self) -> Option<&str> {
        self.fee_recipient.as_deref()
    }

    async fn client(&self) -> Result<deadpool_postgres::Client> {
        Ok(self.pool.get().await?)
    }

    /// Resolve the caller's merchant account, creating the v1 internal
    /// merchant on first use. API keys are deferred, so the user account is
    /// the merchant identity for now.
    async fn merchant(&self, user_id: Uuid) -> Result<Uuid> {
        let client = self.client().await?;
        if let Some(row) = client
            .query_opt(
                "SELECT id FROM b2b_merchants WHERE user_id = $1 AND status <> 'suspended' \
                 ORDER BY created_at LIMIT 1",
                &[&user_id],
            )
            .await?
        {
            return Ok(row.get(0));
        }
        let row = client
            .query_opt(
                "INSERT INTO b2b_merchants (user_id, name, status) VALUES ($1, $2, 'active') \
                 ON CONFLICT (user_id) DO NOTHING RETURNING id",
                &[&user_id, &format!("merchant-{user_id}")],
            )
            .await?;
        if let Some(row) = row {
            return Ok(row.get(0));
        }
        let row = client
            .query_one(
                "SELECT id, status FROM b2b_merchants WHERE user_id = $1",
                &[&user_id],
            )
            .await?;
        let status: String = row.get(1);
        if status == "suspended" {
            return Err(Error::Unavailable("merchant account is suspended".into()));
        }
        Ok(row.get(0))
    }

    fn env_value(env: Option<&str>) -> Result<String> {
        match env.unwrap_or("live").trim().to_ascii_lowercase().as_str() {
            "live" => Ok("live".into()),
            "test" => Ok("test".into()),
            other => Err(Error::Invalid(format!(
                "env must be `test` or `live`, got `{other}`"
            ))),
        }
    }

    fn validated_target(request: &QuoteRequest) -> Result<QuoteTarget> {
        if request.recipient.trim().is_empty() || request.recipient.len() > 256 {
            return Err(Error::Invalid("invalid recipient".into()));
        }
        if request.refund_address.trim().is_empty() || request.refund_address.len() > 256 {
            return Err(Error::Invalid("invalid refund_address".into()));
        }
        let from = Asset::new(&request.input.asset, Some(&request.input.network))
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let to = Asset::new(&request.output.asset, Some(&request.output.network))
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let input_amount = request.input.amount.as_deref().map(str::trim);
        let output_amount = request.output.amount.as_deref().map(str::trim);
        let (exact, value) = match (input_amount, output_amount) {
            (Some(value), None) if !value.is_empty() => (Exact::Input, value),
            (None, Some(value)) if !value.is_empty() => (Exact::Output, value),
            (Some(_), Some(_)) => {
                return Err(Error::Invalid(
                    "provide an amount on exactly one side of the quote".into(),
                ))
            }
            _ => {
                return Err(Error::Invalid(
                    "an input or output amount is required".into(),
                ))
            }
        };
        let asset = if exact == Exact::Input {
            from.clone()
        } else {
            to.clone()
        };
        let amount =
            Amount::new(value, asset).map_err(|error| Error::Invalid(error.to_string()))?;
        if amount.value.parse::<f64>().is_ok_and(|value| value <= 0.0) {
            return Err(Error::Invalid("amount must be greater than zero".into()));
        }
        Ok(QuoteTarget {
            from,
            to,
            exact,
            amount,
            recipient: request.recipient.trim().to_string(),
            refund_address: request.refund_address.trim().to_string(),
        })
    }

    fn request_hash(request: &QuoteRequest) -> Result<String> {
        let canonical = json!({
            "input_asset": request.input.asset.trim().to_ascii_uppercase(),
            "input_network": request.input.network.trim().to_ascii_lowercase(),
            "input_amount": request.input.amount.as_deref().map(str::trim),
            "output_asset": request.output.asset.trim().to_ascii_uppercase(),
            "output_network": request.output.network.trim().to_ascii_lowercase(),
            "output_amount": request.output.amount.as_deref().map(str::trim),
            "recipient": request.recipient.trim(),
            "refund_address": request.refund_address.trim(),
            "external_id": request.external_id,
            "metadata": request.metadata,
        });
        Ok(to_hex(&Sha256::digest(serde_json::to_vec(&canonical)?)))
    }

    async fn request_provider_quote(&self, target: &QuoteTarget) -> Result<PreparedQuote> {
        let margin_bps = self.margin_bps();
        let app_fees = match (&self.fee_recipient, margin_bps) {
            (Some(recipient), bps) if bps > 0 => vec![AppFee {
                recipient: recipient.clone(),
                fee_bps: bps,
            }],
            _ => Vec::new(),
        };
        // A fee-free reference quote tells us what the provider would have
        // delivered without our margin, which is what the ledger records as
        // `best_output`. Without a configured margin the executable quote is
        // itself the best route.
        let reference = if app_fees.is_empty() {
            None
        } else {
            Some(
                self.near
                    .dry_executable_quote(NearQuoteRequest {
                        from: target.from.clone(),
                        to: target.to.clone(),
                        origin_asset_id: None,
                        destination_asset_id: None,
                        amount: target.amount.clone(),
                        recipient: target.recipient.clone(),
                        refund_to: target.refund_address.clone(),
                        slippage_bps: 100,
                        app_fees: Vec::new(),
                        dry: true,
                    })
                    .await
                    .context("NEAR Intents reference quote failed")?,
            )
        };
        let quote = self
            .near
            .executable_quote(NearQuoteRequest {
                from: target.from.clone(),
                to: target.to.clone(),
                origin_asset_id: None,
                destination_asset_id: None,
                amount: target.amount.clone(),
                recipient: target.recipient.clone(),
                refund_to: target.refund_address.clone(),
                slippage_bps: 100,
                app_fees: app_fees.clone(),
                dry: false,
            })
            .await
            .context("NEAR Intents executable quote failed")?;
        let deposit_address = quote
            .deposit_address
            .clone()
            .filter(|address| !address.trim().is_empty())
            .ok_or_else(|| Error::Unavailable("provider returned no deposit address".into()))?;
        let best_output = match target.exact {
            Exact::Input => reference
                .as_ref()
                .map(|quote| quote.output.value.clone())
                .unwrap_or_else(|| quote.output.value.clone()),
            // Exact-output fixes the destination amount; the margin is taken
            // from the input side.
            Exact::Output => quote.output.value.clone(),
        };
        let expires_at = quote_expiry(
            Utc::now() + ChronoDuration::seconds(self.quote_ttl.as_secs() as i64),
            quote.expires_at,
        )?;
        Ok(PreparedQuote {
            exact: target.exact,
            input: quote.input.clone(),
            output: quote.output.clone(),
            best_output,
            captured_bps: margin_bps as i32,
            app_fee_bps: margin_bps as i32,
            deposit_address,
            refund_address: target.refund_address.clone(),
            provider_quote_id: quote.quote_id.clone(),
            expires_at,
            raw: quote.raw.clone(),
        })
    }

    async fn insert_quote(
        &self,
        client: &impl GenericClient,
        order_id: Uuid,
        prepared: &PreparedQuote,
    ) -> Result<Uuid> {
        let raw = serde_json::to_value(&prepared.raw)?;
        let row = client
            .query_one(
                r#"
INSERT INTO b2b_quotes
    (order_id, provider, provider_quote_id, exact, input_amount, output_amount,
     best_output, captured_bps, app_fee_bps, deposit_address, refund_address,
     expires_at, raw)
VALUES ($1, 'near-intents', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
RETURNING id
"#,
                &[
                    &order_id,
                    &prepared.provider_quote_id,
                    &prepared.exact.as_str(),
                    &prepared.input.value,
                    &prepared.output.value,
                    &prepared.best_output,
                    &prepared.captured_bps,
                    &prepared.app_fee_bps,
                    &prepared.deposit_address,
                    &prepared.refund_address,
                    &prepared.expires_at,
                    &raw,
                ],
            )
            .await?;
        Ok(row.get(0))
    }

    /// Create a quote (and its `quoted` order) for the caller.
    pub async fn create_quote(
        &self,
        user_id: Uuid,
        request: &QuoteRequest,
        idempotency_key: Option<&str>,
    ) -> Result<QuoteView> {
        let merchant_id = self.merchant(user_id).await?;
        let env = Self::env_value(request.env.as_deref())?;
        let target = Self::validated_target(request)?;
        let request_hash = Self::request_hash(request)?;
        let key = idempotency_key
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string);
        if let Some(key) = &key {
            if let Some(existing) = self
                .existing_quote_order(merchant_id, &env, key, &request_hash)
                .await?
            {
                return Ok(existing);
            }
        }
        let prepared = self.request_provider_quote(&target).await?;
        let client = self.client().await?;
        let metadata = json!({
            "request_hash": request_hash,
            "request": request,
        });
        let external_id = request.external_id.as_deref();
        let row = client
            .query_one(
                r#"
INSERT INTO b2b_orders
    (merchant_id, env, kind, external_id, input_asset, input_network,
     output_asset, output_network, recipient, refund_address, status,
     metadata, idempotency_key)
VALUES ($1, $2, 'transfer', $3, $4, $5, $6, $7, $8, $9, 'quoted', $10, $11)
RETURNING id
"#,
                &[
                    &merchant_id,
                    &env,
                    &external_id,
                    &target.from.symbol,
                    &target.from.location,
                    &target.to.symbol,
                    &target.to.location,
                    &request.recipient.trim(),
                    &request.refund_address.trim(),
                    &metadata,
                    &key,
                ],
            )
            .await?;
        let order_id: Uuid = row.get(0);
        let quote_id = self.insert_quote(&client, order_id, &prepared).await?;
        self.load_quote(&client, quote_id, merchant_id).await
    }

    async fn existing_quote_order(
        &self,
        merchant_id: Uuid,
        env: &str,
        key: &str,
        request_hash: &str,
    ) -> Result<Option<QuoteView>> {
        let client = self.client().await?;
        let Some(row) = client
            .query_opt(
                "SELECT id, metadata->>'request_hash', status FROM b2b_orders \
                 WHERE merchant_id = $1 AND env = $2 AND idempotency_key = $3",
                &[&merchant_id, &env, &key],
            )
            .await?
        else {
            return Ok(None);
        };
        let stored: Option<String> = row.get(1);
        if stored.as_deref() != Some(request_hash) {
            return Err(Error::Conflict(
                "Idempotency-Key was already used with a different quote request".into(),
            ));
        }
        let order_id: Uuid = row.get(0);
        let Some(row) = client
            .query_opt(
                "SELECT id FROM b2b_quotes WHERE order_id = $1 ORDER BY created_at DESC LIMIT 1",
                &[&order_id],
            )
            .await?
        else {
            return Err(Error::Conflict(
                "existing quote request has no quote".into(),
            ));
        };
        Ok(Some(
            self.load_quote(&client, row.get(0), merchant_id).await?,
        ))
    }

    /// Accept a quote and move the order to `awaiting_deposit`.
    pub async fn create_transfer(
        &self,
        user_id: Uuid,
        request: &CreateTransferRequest,
        idempotency_key: &str,
    ) -> Result<TransferView> {
        let merchant_id = self.merchant(user_id).await?;
        let key = idempotency_key.trim();
        if key.is_empty() || key.len() > 128 {
            return Err(Error::Invalid(
                "Idempotency-Key header is required (max 128 chars)".into(),
            ));
        }
        let client = self.client().await?;
        let Some(row) = client
            .query_opt(
                r#"
SELECT o.id, o.status, o.env, o.accepted_quote_id, o.transfer_key, q.expires_at
FROM b2b_orders o
JOIN b2b_quotes q ON q.order_id = o.id
WHERE q.id = $1 AND o.merchant_id = $2
"#,
                &[&request.quote_id, &merchant_id],
            )
            .await?
        else {
            return Err(Error::NotFound);
        };
        let order_id: Uuid = row.get(0);
        let status: String = row.get(1);
        let env: String = row.get(2);
        let accepted_quote: Option<Uuid> = row.get(3);
        let expires_at: DateTime<Utc> = row.get(5);
        if let Some(existing_env) = request.env.as_deref() {
            if Self::env_value(Some(existing_env))? != env {
                return Err(Error::Conflict("quote belongs to a different env".into()));
            }
        }
        if accepted_quote == Some(request.quote_id) {
            return self.load_transfer(merchant_id, order_id).await;
        }
        if status != "quoted" {
            return Err(Error::Conflict(format!(
                "transfer is `{status}` and can no longer accept a quote"
            )));
        }
        if expires_at <= Utc::now() {
            return Err(Error::Conflict("quote has expired".into()));
        }
        if let Some(row) = client
            .query_opt(
                "SELECT id, accepted_quote_id FROM b2b_orders \
                 WHERE merchant_id = $1 AND env = $2 AND transfer_key = $3",
                &[&merchant_id, &env, &key],
            )
            .await?
        {
            let existing_order: Uuid = row.get(0);
            let accepted: Option<Uuid> = row.get(1);
            if accepted == Some(request.quote_id) {
                return self.load_transfer(merchant_id, existing_order).await;
            }
            return Err(Error::Conflict(
                "Idempotency-Key was already used for a different transfer".into(),
            ));
        }
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        let updated = tx
            .execute(
                r#"
UPDATE b2b_orders
SET status = 'awaiting_deposit', accepted_quote_id = $2, transfer_key = $3,
    external_id = COALESCE($4, external_id), updated_at = now()
WHERE id = $1 AND merchant_id = $5 AND status = 'quoted'
"#,
                &[
                    &order_id,
                    &request.quote_id,
                    &key,
                    &request.external_id,
                    &merchant_id,
                ],
            )
            .await?;
        if updated == 0 {
            return Err(Error::Conflict(
                "transfer status changed concurrently".into(),
            ));
        }
        for kind in ["order.quoted", "order.awaiting_deposit"] {
            let body = self.transfer_body(&tx, merchant_id, order_id).await?;
            webhook::queue_event(&tx, order_id, kind, &body).await?;
        }
        tx.commit().await?;
        self.load_transfer(merchant_id, order_id).await
    }

    pub async fn cancel_transfer(&self, user_id: Uuid, order_id: Uuid) -> Result<TransferView> {
        let merchant_id = self.merchant(user_id).await?;
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        let updated = tx
            .execute(
                "UPDATE b2b_orders SET status = 'cancelled', updated_at = now() \
                 WHERE id = $1 AND merchant_id = $2 AND status IN ('quoted','awaiting_deposit')",
                &[&order_id, &merchant_id],
            )
            .await?;
        if updated == 0 {
            let exists = tx
                .query_opt(
                    "SELECT status FROM b2b_orders WHERE id = $1 AND merchant_id = $2",
                    &[&order_id, &merchant_id],
                )
                .await?;
            return match exists {
                Some(row) => {
                    let status: String = row.get(0);
                    Err(Error::Conflict(format!(
                        "transfer is `{status}` and cannot be cancelled"
                    )))
                }
                None => Err(Error::NotFound),
            };
        }
        let body = self.transfer_body(&tx, merchant_id, order_id).await?;
        webhook::queue_event(&tx, order_id, "order.cancelled", &body).await?;
        tx.commit().await?;
        self.load_transfer(merchant_id, order_id).await
    }

    pub async fn transfer(&self, user_id: Uuid, order_id: Uuid) -> Result<TransferView> {
        let merchant_id = self.merchant(user_id).await?;
        self.load_transfer(merchant_id, order_id).await
    }

    pub async fn transfers(&self, user_id: Uuid, limit: i64) -> Result<Vec<TransferView>> {
        let merchant_id = self.merchant(user_id).await?;
        let client = self.client().await?;
        let limit = limit.clamp(1, 100);
        let rows = client
            .query(
                "SELECT id FROM b2b_orders WHERE merchant_id = $1 \
                 ORDER BY created_at DESC LIMIT $2",
                &[&merchant_id, &limit],
            )
            .await?;
        let mut transfers = Vec::with_capacity(rows.len());
        for row in rows {
            transfers.push(self.load_transfer(merchant_id, row.get(0)).await?);
        }
        Ok(transfers)
    }

    pub async fn load_transfer(&self, merchant_id: Uuid, order_id: Uuid) -> Result<TransferView> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                r#"
SELECT id, merchant_id, env, kind, external_id, status, input_asset,
       input_network, output_asset, output_network, recipient, refund_address,
       accepted_quote_id, created_at, updated_at
FROM b2b_orders WHERE id = $1 AND merchant_id = $2
"#,
                &[&order_id, &merchant_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let quote_id: Option<Uuid> = row.get(12);
        let quote = match quote_id {
            Some(quote_id) => Some(self.load_quote(&client, quote_id, merchant_id).await?),
            None => None,
        };
        let events = client
            .query(
                "SELECT id, kind, body, created_at FROM b2b_events \
                 WHERE order_id = $1 ORDER BY id LIMIT 200",
                &[&order_id],
            )
            .await?
            .into_iter()
            .map(|event| TransferEventView {
                id: event.get(0),
                kind: event.get(1),
                body: event.get(2),
                created_at: event.get(3),
            })
            .collect();
        Ok(TransferView {
            id: row.get(0),
            merchant_id: row.get(1),
            env: row.get(2),
            kind: row.get(3),
            external_id: row.get(4),
            status: row.get(5),
            input_asset: row.get(6),
            input_network: row.get(7),
            output_asset: row.get(8),
            output_network: row.get(9),
            recipient: row.get(10),
            refund_address: row.get(11),
            quote,
            events,
            created_at: row.get(13),
            updated_at: row.get(14),
        })
    }

    async fn transfer_body(
        &self,
        client: &impl GenericClient,
        merchant_id: Uuid,
        order_id: Uuid,
    ) -> Result<Value> {
        let row = client
            .query_opt(
                r#"
SELECT id, env, kind, external_id, status, input_asset, input_network,
       output_asset, output_network, recipient, refund_address, updated_at
FROM b2b_orders WHERE id = $1 AND merchant_id = $2
"#,
                &[&order_id, &merchant_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let quote_id: Option<Uuid> = client
            .query_opt(
                "SELECT accepted_quote_id FROM b2b_orders WHERE id = $1",
                &[&order_id],
            )
            .await?
            .and_then(|row| row.get(0));
        let quote = match quote_id {
            Some(quote_id) => Some(self.load_quote(client, quote_id, merchant_id).await?),
            None => None,
        };
        Ok(json!({
            "id": row.get::<_, Uuid>(0),
            "env": row.get::<_, String>(1),
            "kind": row.get::<_, String>(2),
            "external_id": row.get::<_, Option<String>>(3),
            "status": row.get::<_, String>(4),
            "input_asset": row.get::<_, String>(5),
            "input_network": row.get::<_, String>(6),
            "output_asset": row.get::<_, String>(7),
            "output_network": row.get::<_, String>(8),
            "recipient": row.get::<_, String>(9),
            "refund_address": row.get::<_, Option<String>>(10),
            "quote": quote,
            "updated_at": row.get::<_, DateTime<Utc>>(11),
        }))
    }

    async fn load_quote(
        &self,
        client: &impl GenericClient,
        quote_id: Uuid,
        merchant_id: Uuid,
    ) -> Result<QuoteView> {
        let row = client
            .query_opt(
                r#"
SELECT q.id, q.order_id, q.provider, q.exact, q.input_amount, q.output_amount,
       q.best_output, q.captured_bps, q.app_fee_bps, q.deposit_address,
       q.refund_address, q.expires_at, q.created_at, q.raw,
       o.status, o.input_asset, o.input_network, o.output_asset, o.output_network,
       o.recipient
FROM b2b_quotes q
JOIN b2b_orders o ON o.id = q.order_id
WHERE q.id = $1 AND o.merchant_id = $2
"#,
                &[&quote_id, &merchant_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let raw: Value = row.get(13);
        Ok(QuoteView {
            quote_id: row.get(0),
            order_id: row.get(1),
            provider: row.get(2),
            status: row.get(14),
            exact: row.get(3),
            input_amount: row.get(4),
            output_amount: row.get(5),
            best_output: row.get(6),
            captured_bps: row.get(7),
            app_fee_bps: row.get(8),
            deposit_address: row.get(9),
            refund_address: row.get(10),
            deposit_memo: deposit_memo(&raw),
            expires_at: row.get(11),
            created_at: row.get(12),
            input_asset: row.get(15),
            input_network: row.get(16),
            output_asset: row.get(17),
            output_network: row.get(18),
            recipient: row.get(19),
        })
    }

    /// `POST /api/v1/payments`: quote plus a hosted page token. The order is
    /// accepted immediately, so the payer can fund it from the page.
    pub async fn create_checkout(
        &self,
        user_id: Uuid,
        request: &CreateCheckoutRequest,
        idempotency_key: Option<&str>,
    ) -> Result<CheckoutView> {
        if let Some(url) = request.success_url.as_deref() {
            validate_http_url(url)?;
        }
        let merchant_id = self.merchant(user_id).await?;
        let env = Self::env_value(request.quote.env.as_deref())?;
        let target = Self::validated_target(&request.quote)?;
        let request_hash = Self::request_hash(&request.quote)?;
        let key = idempotency_key
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string);
        if let Some(key) = &key {
            if let Some(existing) = self
                .existing_checkout(
                    merchant_id,
                    &env,
                    key,
                    &request_hash,
                    request.success_url.as_deref(),
                )
                .await?
            {
                return Ok(existing);
            }
        }
        let prepared = self.request_provider_quote(&target).await?;
        let mut client = self.client().await?;
        let metadata = json!({
            "request_hash": request_hash,
            "request": request.quote,
            "success_url": request.success_url,
        });
        let tx = client.transaction().await?;
        let row = tx
            .query_one(
                r#"
INSERT INTO b2b_orders
    (merchant_id, env, kind, external_id, input_asset, input_network,
     output_asset, output_network, recipient, refund_address, status,
     metadata, idempotency_key)
VALUES ($1, $2, 'checkout', $3, $4, $5, $6, $7, $8, $9, 'awaiting_deposit', $10, $11)
RETURNING id
"#,
                &[
                    &merchant_id,
                    &env,
                    &request.quote.external_id,
                    &target.from.symbol,
                    &target.from.location,
                    &target.to.symbol,
                    &target.to.location,
                    &request.quote.recipient.trim(),
                    &request.quote.refund_address.trim(),
                    &metadata,
                    &key,
                ],
            )
            .await?;
        let order_id: Uuid = row.get(0);
        let quote_id = self.insert_quote(&tx, order_id, &prepared).await?;
        tx.execute(
            "UPDATE b2b_orders SET accepted_quote_id = $2 WHERE id = $1",
            &[&order_id, &quote_id],
        )
        .await?;
        let token = random_token();
        tx.execute(
            "INSERT INTO b2b_checkout_sessions (token, order_id, success_url, expires_at) \
             VALUES ($1, $2, $3, $4)",
            &[
                &token,
                &order_id,
                &request.success_url,
                &prepared.expires_at,
            ],
        )
        .await?;
        for kind in ["order.quoted", "order.awaiting_deposit"] {
            let body = self.transfer_body(&tx, merchant_id, order_id).await?;
            webhook::queue_event(&tx, order_id, kind, &body).await?;
        }
        tx.commit().await?;
        let quote = self.load_quote(&client, quote_id, merchant_id).await?;
        Ok(CheckoutView {
            order_id,
            url: format!("{}/pay/{token}", self.public_origin),
            token,
            status: "awaiting_deposit".into(),
            success_url: request.success_url.clone(),
            quote,
        })
    }

    async fn existing_checkout(
        &self,
        merchant_id: Uuid,
        env: &str,
        key: &str,
        request_hash: &str,
        _success_url: Option<&str>,
    ) -> Result<Option<CheckoutView>> {
        let client = self.client().await?;
        let Some(row) = client
            .query_opt(
                "SELECT id, metadata->>'request_hash', status FROM b2b_orders \
                 WHERE merchant_id = $1 AND env = $2 AND idempotency_key = $3",
                &[&merchant_id, &env, &key],
            )
            .await?
        else {
            return Ok(None);
        };
        let stored: Option<String> = row.get(1);
        if stored.as_deref() != Some(request_hash) {
            return Err(Error::Conflict(
                "Idempotency-Key was already used with a different checkout".into(),
            ));
        }
        let order_id: Uuid = row.get(0);
        let stored_status: String = row.get(2);
        let row = client
            .query_one(
                "SELECT token, success_url FROM b2b_checkout_sessions WHERE order_id = $1 \
                 ORDER BY created_at DESC LIMIT 1",
                &[&order_id],
            )
            .await
            .map_err(|_| Error::Conflict("existing checkout has no page token".into()))?;
        let token: String = row.get(0);
        let stored_success_url: Option<String> = row.get(1);
        let quote = self
            .load_quote(
                &client,
                client
                    .query_one(
                        "SELECT accepted_quote_id FROM b2b_orders WHERE id = $1",
                        &[&order_id],
                    )
                    .await?
                    .get(0),
                merchant_id,
            )
            .await?;
        Ok(Some(CheckoutView {
            order_id,
            url: format!("{}/pay/{token}", self.public_origin),
            token,
            status: stored_status,
            success_url: stored_success_url,
            quote,
        }))
    }

    async fn checkout_session(&self, token: &str) -> Result<(Uuid, Option<String>)> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT order_id, success_url FROM b2b_checkout_sessions WHERE token = $1",
                &[&token],
            )
            .await?
            .ok_or(Error::NotFound)?;
        Ok((row.get(0), row.get(1)))
    }

    async fn checkout_merchant(&self, order_id: Uuid) -> Result<Uuid> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT merchant_id FROM b2b_orders WHERE id = $1",
                &[&order_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        Ok(row.get(0))
    }

    pub async fn checkout(&self, token: &str) -> Result<CheckoutStatusView> {
        let (order_id, success_url) = self.checkout_session(token).await?;
        let merchant_id = self.checkout_merchant(order_id).await?;
        let view = self.load_transfer(merchant_id, order_id).await?;
        let quote = view
            .quote
            .ok_or_else(|| Error::Unavailable("checkout has no quote".into()))?;
        Ok(CheckoutStatusView {
            order_id,
            status: view.status,
            success_url,
            quote,
        })
    }

    /// Re-quote an expired checkout page with a new deposit address.
    pub async fn requote_checkout(&self, token: &str) -> Result<CheckoutStatusView> {
        let (order_id, success_url) = self.checkout_session(token).await?;
        let merchant_id = self.checkout_merchant(order_id).await?;
        let mut client = self.client().await?;
        let order = client
            .query_opt(
                "SELECT status, input_asset, input_network, output_asset, output_network, \
                        recipient, refund_address, external_id, accepted_quote_id FROM b2b_orders \
                 WHERE id = $1 AND merchant_id = $2",
                &[&order_id, &merchant_id],
            )
            .await?
            .ok_or(Error::NotFound)?;
        let status: String = order.get(0);
        let issued_address = order.get::<_, Option<Uuid>>(8).is_some();
        if !checkout_can_requote(&status, issued_address) {
            return Err(Error::Conflict(format!(
                "checkout is `{status}` or already has an issued deposit address; ask the merchant for a new checkout"
            )));
        }
        let latest = client
            .query_one(
                "SELECT exact, input_amount, output_amount FROM b2b_quotes \
                 WHERE order_id = $1 ORDER BY created_at DESC LIMIT 1",
                &[&order_id],
            )
            .await?;
        let exact: String = latest.get(0);
        let (input_amount, output_amount) = if exact == "input" {
            (Some(latest.get::<_, String>(1)), None)
        } else {
            (None, Some(latest.get::<_, String>(2)))
        };
        let request = QuoteRequest {
            input: model::QuoteSide {
                asset: order.get(1),
                network: order.get(2),
                amount: input_amount,
            },
            output: model::QuoteSide {
                asset: order.get(3),
                network: order.get(4),
                amount: output_amount,
            },
            recipient: order.get(5),
            refund_address: {
                let address: Option<String> = order.get(6);
                address.unwrap_or_default()
            },
            env: Some("live".into()),
            external_id: order.get(7),
            metadata: None,
        };
        let target = Self::validated_target(&request)?;
        let prepared = self.request_provider_quote(&target).await?;
        let tx = client.transaction().await?;
        let quote_id = self.insert_quote(&tx, order_id, &prepared).await?;
        let updated = tx
            .execute(
                "UPDATE b2b_orders SET status = 'awaiting_deposit', accepted_quote_id = $2, \
             updated_at = now() WHERE id = $1 AND status = $3",
                &[&order_id, &quote_id, &status],
            )
            .await?;
        if updated == 0 {
            return Err(Error::Conflict(
                "checkout changed while being re-quoted".into(),
            ));
        }
        tx.execute(
            "UPDATE b2b_checkout_sessions SET expires_at = $2 WHERE token = $1",
            &[&token, &prepared.expires_at],
        )
        .await?;
        for kind in ["order.quoted", "order.awaiting_deposit"] {
            let body = self.transfer_body(&tx, merchant_id, order_id).await?;
            webhook::queue_event(&tx, order_id, kind, &body).await?;
        }
        tx.commit().await?;
        let quote = self.load_quote(&client, quote_id, merchant_id).await?;
        Ok(CheckoutStatusView {
            order_id,
            status: "awaiting_deposit".into(),
            success_url,
            quote,
        })
    }

    pub async fn analytics(&self, user_id: Uuid) -> Result<AnalyticsView> {
        let merchant_id = self.merchant(user_id).await?;
        let client = self.client().await?;
        let totals = client
            .query_one(
                r#"
SELECT COUNT(*)::BIGINT,
       COUNT(*) FILTER (WHERE status IN ('paid','overpaid'))::BIGINT,
       COALESCE(ROUND(AVG(captured_bps) FILTER (
           WHERE status IN ('paid','overpaid'))), 0)::INT
FROM b2b_orders o
LEFT JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1
"#,
                &[&merchant_id],
            )
            .await?;
        let orders: i64 = totals.get(0);
        let paid_orders: i64 = totals.get(1);
        let margin_bps: i32 = totals.get(2);
        let average_ticket = client
            .query(
                r#"
SELECT input_asset, ROUND(COALESCE(AVG(input_amount::NUMERIC), 0), 8)::TEXT
FROM b2b_orders o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1
GROUP BY input_asset
ORDER BY input_asset
"#,
                &[&merchant_id],
            )
            .await?
            .into_iter()
            .map(|row| AssetAmount {
                asset: row.get(0),
                amount: row.get(1),
            })
            .collect();
        let volume = client
            .query(
                r#"
SELECT input_asset, COALESCE(SUM(input_amount::NUMERIC), 0)::TEXT
FROM b2b_orders o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
WHERE merchant_id = $1 AND o.status IN ('paid','overpaid')
GROUP BY input_asset
ORDER BY input_asset
"#,
                &[&merchant_id],
            )
            .await?
            .into_iter()
            .map(|row| AssetAmount {
                asset: row.get(0),
                amount: row.get(1),
            })
            .collect();
        let captured = client
            .query(
                "SELECT asset, COALESCE(SUM(amount::NUMERIC), 0)::TEXT FROM b2b_fee_ledger \
                 WHERE merchant_id = $1 GROUP BY asset ORDER BY asset",
                &[&merchant_id],
            )
            .await?
            .into_iter()
            .map(|row| AssetAmount {
                asset: row.get(0),
                amount: row.get(1),
            })
            .collect();
        let failure_reasons = client
            .query(
                r#"
SELECT status, COUNT(*)::BIGINT FROM b2b_orders
WHERE merchant_id = $1
  AND status IN ('failed','expired','underpaid','cancelled')
GROUP BY status ORDER BY status
"#,
                &[&merchant_id],
            )
            .await?
            .into_iter()
            .map(|row| FailureReason {
                reason: row.get(0),
                orders: row.get(1),
            })
            .collect();
        Ok(AnalyticsView {
            orders,
            paid_orders,
            conversion: if orders == 0 {
                0.0
            } else {
                paid_orders as f64 / orders as f64
            },
            average_ticket,
            volume,
            captured,
            margin_bps,
            failure_reasons,
        })
    }

    /// Shared by the worker: load everything needed to poll an accepted order.
    async fn accepted_orders(&self) -> Result<Vec<AcceptedOrder>> {
        let client = self.client().await?;
        let rows = client
            .query(
                r#"
WITH selected AS (
    SELECT id FROM b2b_orders
    WHERE accepted_quote_id IS NOT NULL
      AND status NOT IN ('paid','overpaid','refunded')
    ORDER BY last_polled_at ASC NULLS FIRST, created_at ASC, id ASC
    LIMIT 50
    FOR UPDATE SKIP LOCKED
), polled AS (
    UPDATE b2b_orders o SET last_polled_at = now()
    FROM selected s WHERE o.id = s.id RETURNING o.*
)
SELECT o.id, o.merchant_id, o.status, o.input_network, o.output_network,
       q.id, q.input_amount, q.output_amount, q.captured_bps, q.deposit_address,
       q.expires_at, q.raw
FROM polled o
JOIN b2b_quotes q ON q.id = o.accepted_quote_id
"#,
                &[],
            )
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let raw: Value = row.get(11);
                AcceptedOrder {
                    order_id: row.get(0),
                    merchant_id: row.get(1),
                    status: row.get(2),
                    input_network: row.get(3),
                    output_network: row.get(4),
                    quote_id: row.get(5),
                    input_amount: row.get(6),
                    output_amount: row.get(7),
                    captured_bps: row.get(8),
                    deposit_address: row.get(9),
                    expires_at: row.get(10),
                    deposit_memo: deposit_memo(&raw),
                }
            })
            .collect())
    }

    async fn expire_unaccepted(&self) -> Result<()> {
        let client = self.client().await?;
        client
            .execute(
                r#"
UPDATE b2b_orders o
SET status = 'expired', updated_at = now()
FROM b2b_quotes q
WHERE o.accepted_quote_id IS NULL AND o.status = 'quoted'
  AND q.order_id = o.id AND q.expires_at < now()
  AND q.created_at = (SELECT max(created_at) FROM b2b_quotes WHERE order_id = o.id)
"#,
                &[],
            )
            .await?;
        Ok(())
    }
}

struct AcceptedOrder {
    order_id: Uuid,
    merchant_id: Uuid,
    status: String,
    input_network: String,
    output_network: String,
    quote_id: Uuid,
    input_amount: String,
    output_amount: String,
    captured_bps: i32,
    deposit_address: String,
    expires_at: DateTime<Utc>,
    deposit_memo: Option<String>,
}

fn validate_http_url(url: &str) -> Result<()> {
    if url.len() > 2048 {
        return Err(Error::Invalid("success_url is too long".into()));
    }
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| Error::Invalid("success_url must be an absolute URL".into()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Error::Invalid("success_url must use http or https".into()));
    }
    Ok(())
}

fn deposit_memo(raw: &Value) -> Option<String> {
    raw.get("quote")
        .and_then(|quote| quote.get("depositMemo"))
        .or_else(|| raw.get("depositMemo"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn random_token() -> String {
    let mut bytes = [0u8; MAX_WEBHOOK_SECRET_BYTES];
    OsRng.fill_bytes(&mut bytes);
    to_hex(&bytes)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Exact `value * bps / 10000` on a decimal string, using an 18-decimal fixed
/// point representation. Money never touches `f64`.
fn bps_amount(value: &str, bps: i32) -> Option<String> {
    let scaled = parse_scaled(value)?;
    let scaled = scaled.checked_mul(u128::try_from(bps.max(0)).ok()?)? / 10_000;
    format_scaled(scaled)
}

fn parse_scaled(value: &str) -> Option<u128> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 18
    {
        return None;
    }
    let whole: u128 = whole.parse().ok()?;
    let fraction: u128 = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<18}").parse().ok()?
    };
    whole.checked_mul(10u128.pow(18))?.checked_add(fraction)
}

fn format_scaled(value: u128) -> Option<String> {
    let whole = value / 10u128.pow(18);
    let fraction = value % 10u128.pow(18);
    if fraction == 0 {
        return Some(whole.to_string());
    }
    let fraction = format!("{fraction:018}");
    let fraction = fraction.trim_end_matches('0');
    Some(format!("{whole}.{fraction}"))
}

fn decimal_greater(left: &str, right: &str) -> bool {
    match (parse_scaled(left), parse_scaled(right)) {
        (Some(left), Some(right)) => left > right,
        _ => false,
    }
}

fn checkout_can_requote(status: &str, issued_address: bool) -> bool {
    !issued_address && matches!(status, "quoted" | "expired")
}

fn quote_expiry(local: DateTime<Utc>, provider: Option<DateTime<Utc>>) -> Result<DateTime<Utc>> {
    let deadline =
        provider.ok_or_else(|| Error::Unavailable("provider quote has no expiry".into()))?;
    let expiry = local.min(deadline);
    if expiry <= Utc::now() {
        return Err(Error::Unavailable("provider quote has expired".into()));
    }
    Ok(expiry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_regression_cancelled_checkout_stays_cancelled() {
        assert!(!checkout_can_requote("cancelled", false));
        assert!(!checkout_can_requote("failed", false));
        assert!(checkout_can_requote("expired", false));
        assert!(!checkout_can_requote("expired", true));
        assert!(!checkout_can_requote("quoted", true));
    }

    #[test]
    fn release_regression_missing_and_expired_deadlines_are_refused() {
        let local = Utc::now() + ChronoDuration::minutes(15);
        assert!(quote_expiry(local, None).is_err());
        assert!(quote_expiry(local, Some(Utc::now() - ChronoDuration::seconds(1))).is_err());
        let provider = Utc::now() + ChronoDuration::minutes(5);
        assert_eq!(quote_expiry(local, Some(provider)).unwrap(), provider);
    }

    #[test]
    fn margin_capture_uses_exact_decimal_arithmetic() {
        assert_eq!(bps_amount("1000", 50).as_deref(), Some("5"));
        assert_eq!(bps_amount("1000.00", 50).as_deref(), Some("5"));
        assert_eq!(bps_amount("0.000001", 50).as_deref(), Some("0.000000005"));
        assert_eq!(bps_amount("99.5", 70).as_deref(), Some("0.6965"));
        assert_eq!(bps_amount("-1", 50), None);
    }

    #[test]
    fn decimal_comparison_ignores_formatting() {
        assert!(decimal_greater("100.10", "100.1") == false);
        assert!(decimal_greater("100.11", "100.1"));
        assert!(!decimal_greater("99.9", "100"));
    }

    #[test]
    fn exact_side_validation_rejects_two_amounts() {
        let request = QuoteRequest {
            input: model::QuoteSide {
                asset: "USDT".into(),
                network: "ethereum".into(),
                amount: Some("10".into()),
            },
            output: model::QuoteSide {
                asset: "USDT".into(),
                network: "tron".into(),
                amount: Some("10".into()),
            },
            recipient: "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb".into(),
            refund_address: "0x8ba1f109551bD432803012645Ac136ddd64DBA72".into(),
            env: None,
            external_id: None,
            metadata: None,
        };
        assert!(matches!(
            Service::validated_target(&request),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn quote_ttl_is_capped_at_one_hour() {
        let service = test_service(Duration::from_secs(3 * 60 * 60));
        assert_eq!(service.quote_ttl(), Duration::from_secs(3600));
    }

    #[test]
    fn missing_fee_recipient_means_zero_margin() {
        let service = Service {
            pool: test_pool(),
            near: NearIntentsProvider::new("https://example.test", None).unwrap(),
            secrets: SecretBox::new("test"),
            fee_recipient: None,
            margin_bps: 50,
            margin_cap_bps: 70,
            quote_ttl: Duration::from_secs(900),
            public_origin: "https://pay3flow.test".into(),
        };
        assert_eq!(service.margin_bps(), 0);
    }

    fn test_service(ttl: Duration) -> Service {
        Service {
            pool: test_pool(),
            near: NearIntentsProvider::new("https://example.test", None).unwrap(),
            secrets: SecretBox::new("test"),
            fee_recipient: Some("fees.near".into()),
            margin_bps: 50,
            margin_cap_bps: 70,
            quote_ttl: Duration::from_secs(ttl.as_secs().clamp(60, MAX_QUOTE_TTL_SECS)),
            public_origin: "https://pay3flow.test".into(),
        }
    }

    fn test_pool() -> DbPool {
        crate::db::build_pool_for_tests()
    }
}

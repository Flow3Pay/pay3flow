use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use uuid::Uuid;

use crate::activitypub::model::AcquirerCandidate;
use crate::activitypub::Service;
use crate::db::DbPool;
use crate::payments::fees;
use crate::payments::model::{
    to_minor, Minor, NewPayment, NewRoute, NewTransaction, Route,
};
use crate::payments::providers::{
    AcquireProvider, AcquireResult, ExecuteRequest, ProviderRegistry,
};
use crate::payments::rates::Rates;
use crate::payments::repo::{self, AcquirerRow};
use crate::payments::status::{RouteStatus, TransactionStatus};
use crate::quotes::Quote;
use crate::routing::{PaymentRequest, RoutePicker};

/// Live status event broadcast to `/ws/payments` subscribers (PLAN #44).
#[derive(Debug, Clone, Serialize)]
pub struct PaymentEvent {
    pub id: Uuid,
    pub status: TransactionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub at: String,
}

/// Domain service for creating and advancing payments (stages 33–40).
///
/// Owns no HTTP surface: handlers build a `PaymentService` inside `AppState`,
/// call `create`, and serialize the resulting `PaymentView`.
#[derive(Clone)]
pub struct PaymentService {
    pool: DbPool,
    ap: Service,
    picker: RoutePicker,
    providers: ProviderRegistry,
    rates: Rates,
    service_fee_percent: f64,
    events: tokio::sync::broadcast::Sender<PaymentEvent>,
}

#[derive(Clone)]
pub struct PaymentConfig {
    /// Our own cut, percent of the gross amount.
    pub service_fee_percent: f64,
}

impl PaymentService {
    pub fn new(
        pool: DbPool,
        ap: Service,
        picker: RoutePicker,
        providers: ProviderRegistry,
        rates: Rates,
        config: PaymentConfig,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(64);
        Self {
            pool,
            ap,
            picker,
            providers,
            rates,
            service_fee_percent: config.service_fee_percent,
            events,
        }
    }

    pub fn rates(&self) -> &Rates {
        &self.rates
    }

    /// Subscribe to live payment status events (PLAN #44).
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PaymentEvent> {
        self.events.subscribe()
    }

    fn publish(&self, id: Uuid, status: TransactionStatus, external_id: Option<String>) {
        let _ = self.events.send(PaymentEvent {
            id,
            status,
            external_id,
            at: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Full lifecycle of a payment: validate → route → execute → persist.
    ///
    /// Returns a view with the transaction and, when routed, its route.
    pub async fn create(&self, user_id: Uuid, payment: NewPayment, idem_key: Option<String>) -> Result<PaymentView> {
        let gross = validate(&payment)?;

        // Idempotency: when the caller supplied a key, a second identical
        // request must return the first result instead of double-charging.
        // The unique partial index on `idempotency_key` is the guard; see the
        // concurrent try-in-insert path below (PLAN #36, #45).
        if let Some(key) = &idem_key {
            if let Some(existing) = repo::transaction_by_idempotency_key(&self.pool, key).await? {
                return self.view(&existing.id).await;
            }
        }

        let fees = fees::compute(
            gross,
            self.service_fee_percent,
            0.0,
            0,
        );
        let inserted = repo::insert_transaction(
            &self.pool,
            &NewTransaction {
                user_id,
                from_amount: gross,
                from_currency: payment.currency.clone(),
                to_currency: payment.to_currency.clone(),
                from_account: payment.from.clone(),
                to_account: payment.to.clone(),
                method: payment.method.clone(),
                fees: fees.total(),
                idempotency_key: idem_key.clone(),
            },
        )
        .await?;

        // Race on the same idempotency key (PLAN #45): the unique partial index
        // lets only one INSERT win; the loser returns the winner's row right
        // away — routing and execution never run twice for one key.
        let tx = match (inserted, &idem_key) {
            (Some(tx), _) => tx,
            (None, Some(key)) => {
                let winner = repo::transaction_by_idempotency_key(&self.pool, key)
                    .await?
                    .context("idempotency race lost but winner row not found")?;
                return self.view(&winner.id).await;
            }
            (None, None) => {
                return Err(anyhow!("transaction insert returned no row"));
            }
        };
        self.publish(tx.id, TransactionStatus::Pending, None);

        // Route: reuse the live quote pipeline (fmatch → fallback). The picked
        // acquirer is the rank-1 candidate.
        let request = PaymentRequest::new(
            payment.amount,
            &payment.currency,
            &payment.from,
            &payment.to,
        )
        .with_to_currency(payment.to_currency.clone().unwrap_or_else(|| payment.currency.clone()))
        .with_geo(payment.to_geo.clone())
        .with_method(payment.method.clone());
        let quote = crate::quotes::compute_quote(&self.ap, &self.picker, request).await;
        // compute_quote already re-ranks fmatch by the real pair fee when the
        // request converts (fallback ranks by effective fee today), so rank-1 is
        // the cheapest managed solver that serves the pair, not fmatch's
        // relevance winner. The final commercial pick is ours (PLAN #22).
        let Some(best) = quote.best.clone() else {
            // No acquirer can serve; park the transaction as failed (route
            // never created). The failure reason is surfaced in the view.
            repo::transition_status(&self.pool, &tx.id, TransactionStatus::Pending, TransactionStatus::Failed)
                .await?;
            return self.view(&tx.id).await;
        };

        // Fall back to the acquirer fee from the local profile when the
        // candidate (`price` is an effective ratio, not our fee terms).
        let acquirer = self.acquirer_for(&best).await?;
        let acquirer_slug = acquirer
            .as_ref()
            .map(|a| a.slug.clone())
            .unwrap_or_else(|| best.short_id.clone());
        // Fee: the exact pair commission when the winner is a managed solver
        // serving `from -> to`, otherwise the profile fee_percent as before.
        let to_currency = payment.to_currency.as_deref().unwrap_or(&payment.currency);
        let pair_fee_percent = crate::fake_acquirers::fake_acquirer_by_name(&best.name)
            .and_then(|a| a.commission_for(&payment.currency, to_currency))
            .and_then(crate::fake_acquirers::commission_pct);
        let fee_percent = pair_fee_percent
            .unwrap_or_else(|| acquirer.as_ref().map(|a| a.fee_percent).unwrap_or(0.0));
        let route = repo::insert_route(
            &self.pool,
            &NewRoute {
                transaction_id: tx.id,
                acquirer_id: acquirer.as_ref().map(|a| a.id),
                acquirer_slug: acquirer_slug.clone(),
                fee_percent,
                exchange_rate: None,
                status: RouteStatus::Pending,
                source: quote.source.as_str().to_string(),
            },
        )
        .await?
        .context("route insert returned no row")?;

        // Pending -> Matched (route chosen).
        repo::transition_status(&self.pool, &tx.id, TransactionStatus::Pending, TransactionStatus::Matched)
            .await?;
        repo::transition_route(&self.pool, &route.id, RouteStatus::Pending, RouteStatus::Matched)
            .await?;
        self.publish(tx.id, TransactionStatus::Matched, None);

        // Fees: ours + the acquirer's, then compute what the recipient gets
        // (gross - fees), optionally converted to the destination currency.
        let totals = fees::compute(
            gross,
            self.service_fee_percent,
            fee_percent,
            acquirer.as_ref().map(|a| a.fee_fixed).unwrap_or(0),
        );
        let net_minor = fees::net_amount(gross, &totals);
        let (to_amount, to_currency) = match (&payment.to_currency, &payment.currency) {
            (Some(target), from) if !target.eq_ignore_ascii_case(from) => {
                let converted = self.rates.convert(net_minor, from, target).await?;
                (Some(converted), Some(target.clone()))
            }
            _ => (Some(net_minor), Some(payment.currency.clone())),
        };
        repo::set_amounts(
            &self.pool,
            &tx.id,
            to_amount,
            to_currency,
            totals.total(),
        )
        .await?;

        // Execute through the matched acquirer. The stub provider drives the
        // smoke-test branches deterministically.
        let (provider_status, external_id) = self.execute(&tx.id, &route, &payment).await?;
        let (final_status, route_status) = match provider_status {
            AcquireResult::Succeeded => (TransactionStatus::Done, RouteStatus::Done),
            AcquireResult::Failed => (TransactionStatus::Failed, RouteStatus::Failed),
            AcquireResult::Pending => (TransactionStatus::Executing, RouteStatus::Executing),
        };
        repo::transition_status(&self.pool, &tx.id, TransactionStatus::Executing, final_status)
            .await?;
        repo::transition_route(&self.pool, &route.id, RouteStatus::Matched, route_status)
            .await?;
        repo::set_provider(
            &self.pool,
            &tx.id,
            &acquirer_slug,
            &external_id,
        )
        .await?;
        self.publish(tx.id, final_status, Some(external_id));

        self.view(&tx.id).await
    }

    /// Fetch a fully-populated payment view (transaction + optional route).
    pub async fn view(&self, tx_id: &Uuid) -> Result<PaymentView> {
        let tx = repo::transaction_by_id(&self.pool, tx_id)
            .await?
            .context("transaction not found")?;
        let route = repo::route_for_transaction(&self.pool, tx_id).await?;
        Ok(PaymentView { transaction: tx, route, quote: None })
    }

    /// List recent payments for a user.
    pub async fn list_for_user(&self, user_id: &Uuid) -> Result<Vec<PaymentView>> {
        let txs = repo::transactions_for_user(&self.pool, user_id).await?;
        let mut views = Vec::with_capacity(txs.len());
        for tx in txs {
            let route = repo::route_for_transaction(&self.pool, &tx.id).await?;
            views.push(PaymentView { transaction: tx, route, quote: None });
        }
        Ok(views)
    }

    /// Resolve the acquirer a fmatch candidate refers to.
    ///
    /// fmatch identifies candidates by a generic `shortId` (e.g. "c-offer"),
    /// not our own slug, so first try the literal slug, then match the
    /// candidate name (e.g. "Flinger Pay (US|EU|Global, 3.4%)") against our
    /// local profile pool and the fake solver catalog. `None` = candidate is a
    /// solver we don't manage; the route is still stored with `acquirer_id = NULL`.
    async fn acquirer_for(&self, best: &AcquirerCandidate) -> Result<Option<AcquirerRow>> {
        if let Some(row) = repo::acquirer_by_slug(&self.pool, &best.short_id).await? {
            return Ok(Some(row));
        }
        if let Some(row) = self.acquirer_by_candidate_name(&best.name).await? {
            return Ok(Some(row));
        }
        Ok(None)
    }

    async fn acquirer_by_candidate_name(&self, name: &str) -> Result<Option<AcquirerRow>> {
        if name.trim().is_empty() {
            return Ok(None);
        }
        // Our own curated real-provider pool.
        let pool = crate::routing::profile::seed_pool();
        if let Some(slug) = pool.iter().find(|p| name.starts_with(&p.name)).map(|p| p.slug.clone()) {
            return repo::acquirer_by_slug(&self.pool, &slug).await;
        }
        // The fictional solvers fmatch actually routes to.
        if let Some(fake) = crate::fake_acquirers::FAKE_ACQUIRERS
            .iter()
            .find(|a| name.starts_with(a.name))
        {
            return repo::acquirer_by_slug(&self.pool, fake.slug).await;
        }
        Ok(None)
    }

    async fn execute(
        &self,
        tx_id: &Uuid,
        route: &Route,
        payment: &NewPayment,
    ) -> Result<(AcquireResult, String)> {
        let provider = match self.providers.get(&route.acquirer_slug) {
            Some(p) => p,
            // The matched acquirer has no live adapter: fall back to the stub so
            // smoke tests and development still complete end-to-end.
            None => self
                .providers
                .get("stub")
                .ok_or_else(|| anyhow!("no provider for slug {}", route.acquirer_slug))?,
        };
        self.run_provider(provider, tx_id, payment).await
    }

    async fn run_provider(
        &self,
        provider: &dyn AcquireProvider,
        tx_id: &Uuid,
        payment: &NewPayment,
    ) -> Result<(AcquireResult, String)> {
        let provider_ref = tx_id.to_string();
        // Matched -> Executing, held until the provider settles.
        repo::transition_status(&self.pool, tx_id, TransactionStatus::Matched, TransactionStatus::Executing)
            .await?;
        let request = ExecuteRequest {
            amount: to_minor(payment.amount),
            currency: payment.currency.clone(),
            from_account: payment.from.clone(),
            to_account: payment.to.clone(),
            method: payment.method.clone(),
            provider_ref,
        };
        let outcome = provider.execute(&request).await?;
        Ok((outcome.status, outcome.external_id))
    }
}

/// Result of a payment creation, what the API serializes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaymentView {
    pub transaction: crate::payments::model::Transaction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
}

/// Validate a payment request (PLAN #34). Returns gross amount in minor units.
fn validate(payment: &NewPayment) -> Result<Minor> {
    if !(payment.amount.is_finite()) || payment.amount <= 0.0 {
        return Err(anyhow!("amount must be positive"));
    }
    if payment.currency.trim().is_empty() {
        return Err(anyhow!("currency is required"));
    }
    if payment.from.trim().is_empty() {
        return Err(anyhow!("from (source account) is required"));
    }
    if payment.to.trim().is_empty() {
        return Err(anyhow!("to (destination account) is required"));
    }
    Ok(to_minor(payment.amount))
}

trait RequestExt {
    fn with_geo(self, geo: Option<String>) -> Self;
    fn with_method(self, method: Option<String>) -> Self;
}

impl RequestExt for PaymentRequest {
    fn with_geo(mut self, geo: Option<String>) -> Self {
        self.to_geo = geo;
        self
    }
    fn with_method(mut self, method: Option<String>) -> Self {
        self.method = method;
        self
    }
}
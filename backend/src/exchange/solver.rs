use std::time::Duration as StdDuration;

use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::exchange::model::{
    ExchangeOrder, ExchangeSolver, Minor, NewExchangeQuote, NewExchangeSolver,
};
use crate::exchange::status::{OrderStatus, QuoteStatus, SolverStatus};

pub const FAST_LOW_LIMIT_SLUG: &str = "fast-low-limit";
pub const SLOW_BETTER_RATE_SLUG: &str = "slow-better-rate";
const QUOTE_TTL_MINUTES: i64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FakeSolverBehavior {
    Success,
    Reject,
    Timeout,
}

impl Default for FakeSolverBehavior {
    fn default() -> Self {
        Self::Success
    }
}

#[derive(Debug, Clone)]
pub struct FakeSolver {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub rate_bps: i64,
    pub fee_minor: Minor,
    pub eta_minutes: i32,
    pub min_amount_minor: Option<Minor>,
    pub max_amount_minor: Option<Minor>,
    pub risk_score: i32,
}

impl FakeSolver {
    pub fn as_new_solver(&self) -> NewExchangeSolver {
        NewExchangeSolver {
            slug: self.slug.to_string(),
            actor_id: Some(format!("https://pay3flow.local/solvers/{}", self.slug)),
            handle: Some(self.slug.to_string()),
            display_name: self.display_name.to_string(),
            status: SolverStatus::Active,
            countries: json!(["AM", "RU"]),
            currencies: json!(["AMD", "RUB"]),
            rails: json!(["card", "bank", "bank_card", "wallet"]),
            min_amount_minor: self.min_amount_minor,
            max_amount_minor: self.max_amount_minor,
            fee_model: json!({
                "type": "fake_solver",
                "rate_bps": self.rate_bps,
                "fee_minor": self.fee_minor,
                "eta_minutes": self.eta_minutes,
            }),
            risk_score: self.risk_score,
        }
    }
}

pub fn fake_solvers() -> Vec<FakeSolver> {
    vec![
        FakeSolver {
            slug: FAST_LOW_LIMIT_SLUG,
            display_name: "Fast Low Limit",
            rate_bps: 2_000,
            fee_minor: 300,
            eta_minutes: 5,
            min_amount_minor: Some(1_000),
            max_amount_minor: Some(10_000_000),
            risk_score: 15,
        },
        FakeSolver {
            slug: SLOW_BETTER_RATE_SLUG,
            display_name: "Slow Better Rate",
            rate_bps: 2_150,
            fee_minor: 150,
            eta_minutes: 45,
            min_amount_minor: Some(1_000),
            max_amount_minor: Some(100_000_000),
            risk_score: 25,
        },
    ]
}

#[derive(Debug, Clone)]
pub struct RouteQuoteRequest {
    pub order: ExchangeOrder,
    pub solver: ExchangeSolver,
    pub behavior: FakeSolverBehavior,
}

#[derive(Debug, Clone)]
pub enum RouteQuoteOutcome {
    Success { quote: NewExchangeQuote },
    Rejected { reason: String },
    TimedOut { after_ms: u64 },
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteQuoteSourceHealth {
    pub name: String,
    pub healthy: bool,
}

#[async_trait]
pub trait RouteQuoteSource: Send + Sync {
    fn name(&self) -> &str;
    async fn health(&self) -> RouteQuoteSourceHealth;
    async fn quote(&self, request: RouteQuoteRequest) -> RouteQuoteOutcome;
}

#[derive(Debug, Clone)]
pub struct MockRouteQuoteSource {
    timeout_after: StdDuration,
}

impl Default for MockRouteQuoteSource {
    fn default() -> Self {
        Self {
            timeout_after: StdDuration::from_millis(150),
        }
    }
}

impl MockRouteQuoteSource {
    fn profile_for(&self, slug: &str) -> Option<FakeSolver> {
        fake_solvers()
            .into_iter()
            .find(|solver| solver.slug == slug)
    }
}

#[async_trait]
impl RouteQuoteSource for MockRouteQuoteSource {
    fn name(&self) -> &str {
        "mock"
    }

    async fn health(&self) -> RouteQuoteSourceHealth {
        RouteQuoteSourceHealth {
            name: self.name().to_string(),
            healthy: true,
        }
    }

    async fn quote(&self, request: RouteQuoteRequest) -> RouteQuoteOutcome {
        match request.behavior {
            FakeSolverBehavior::Reject => RouteQuoteOutcome::Rejected {
                reason: "fake solver rejected the order".into(),
            },
            FakeSolverBehavior::Timeout => {
                sleep(self.timeout_after).await;
                RouteQuoteOutcome::TimedOut {
                    after_ms: self.timeout_after.as_millis() as u64,
                }
            }
            FakeSolverBehavior::Success => {
                let Some(profile) = self.profile_for(&request.solver.slug) else {
                    return RouteQuoteOutcome::Rejected {
                        reason: format!("unknown fake solver: {}", request.solver.slug),
                    };
                };
                match build_quote(&request.order, &request.solver, &profile, Utc::now()) {
                    Ok(quote) => RouteQuoteOutcome::Success { quote },
                    Err(err) => RouteQuoteOutcome::Rejected {
                        reason: err.to_string(),
                    },
                }
            }
        }
    }
}

pub fn validate_quote(
    order: &ExchangeOrder,
    solver: &ExchangeSolver,
    quote: &NewExchangeQuote,
    now: DateTime<Utc>,
) -> Result<()> {
    if !solver.status.accepts_quotes() {
        bail!("solver is not active");
    }
    if !matches!(order.status, OrderStatus::Quoting | OrderStatus::Quoted) {
        bail!("exchange order must be quoting before solver quote");
    }
    if quote.order_id != order.id || quote.solver_id != solver.id {
        bail!("quote ids do not match order and solver");
    }
    if quote.source_amount_minor != order.source_amount_minor
        || quote.source_currency != order.source_currency
        || quote.target_currency != order.target_currency
    {
        bail!("quote does not match order amounts or currencies");
    }
    if quote.target_amount_minor <= 0 {
        bail!("quote target amount must be positive");
    }
    if let Some(min) = order.target_amount_min_minor {
        if quote.target_amount_minor < min {
            bail!("quote target amount is below minimum");
        }
    }
    if quote.fee_minor < 0 {
        bail!("quote fee must not be negative");
    }
    if quote.eta_minutes <= 0 {
        bail!("quote eta_minutes must be positive");
    }
    if quote.expires_at <= now {
        bail!("quote expired");
    }
    Ok(())
}

fn build_quote(
    order: &ExchangeOrder,
    solver: &ExchangeSolver,
    profile: &FakeSolver,
    now: DateTime<Utc>,
) -> Result<NewExchangeQuote> {
    if let Some(min) = profile.min_amount_minor {
        if order.source_amount_minor < min {
            bail!("source amount is below fake solver minimum");
        }
    }
    if let Some(max) = profile.max_amount_minor {
        if order.source_amount_minor > max {
            bail!("source amount is above fake solver maximum");
        }
    }

    let gross_target = order.source_amount_minor * profile.rate_bps / 10_000;
    let target_amount_minor = gross_target - profile.fee_minor;
    let quote = NewExchangeQuote {
        order_id: order.id,
        solver_id: solver.id,
        source_amount_minor: order.source_amount_minor,
        target_amount_minor,
        source_currency: order.source_currency.clone(),
        target_currency: order.target_currency.clone(),
        funding_method_type: order.source_method_type.clone(),
        requires_user_funding: true,
        rate: format!("{:.12}", profile.rate_bps as f64 / 10_000.0),
        fee_minor: profile.fee_minor,
        eta_minutes: profile.eta_minutes,
        expires_at: now + Duration::minutes(QUOTE_TTL_MINUTES),
        status: QuoteStatus::Valid,
        settlement_plan: settlement_plan(order, solver, profile),
        risk_score: profile.risk_score,
        score: None,
        raw_response: Some(json!({
            "source": "mock",
            "solver_slug": profile.slug,
            "behavior": "success",
        })),
    };
    validate_quote(order, solver, &quote, now)?;
    Ok(quote)
}

fn settlement_plan(order: &ExchangeOrder, solver: &ExchangeSolver, profile: &FakeSolver) -> Value {
    json!({
        "type": "fake_solver_route",
        "solver_slug": &solver.slug,
        "source_leg": {
            "country": &order.source_country,
            "currency": &order.source_currency,
            "method_type": &order.source_method_type,
            "requires_user_funding": true,
        },
        "token_leg": {
            "asset": "TOKEN",
            "mode": "mock_ledger",
        },
        "money_leg": {
            "country": &order.target_country,
            "currency": &order.target_currency,
            "method_type": &order.target_method_type,
            "eta_minutes": profile.eta_minutes,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::model::ExchangeSolver;
    use crate::exchange::status::FundingInstructionStatus;
    use uuid::Uuid;

    fn order(status: OrderStatus) -> ExchangeOrder {
        ExchangeOrder {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            idempotency_key: "idem".into(),
            source_country: "AM".into(),
            source_currency: "AMD".into(),
            source_amount_minor: 100_000,
            source_method_type: "card".into(),
            source_method_ref: None,
            target_country: "RU".into(),
            target_currency: "RUB".into(),
            target_amount_min_minor: Some(19_000),
            target_method_type: "bank_card".into(),
            target_method_ref: None,
            funding_instruction_id: None,
            funding_status: FundingInstructionStatus::NotStarted,
            status,
            correlation_id: Uuid::new_v4(),
            deadline_at: None,
            selected_quote_id: None,
            failure_code: None,
            failure_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn solver(status: SolverStatus, slug: &str) -> ExchangeSolver {
        ExchangeSolver {
            id: Uuid::new_v4(),
            slug: slug.into(),
            actor_id: None,
            handle: None,
            display_name: slug.into(),
            status,
            countries: json!(["AM", "RU"]),
            currencies: json!(["AMD", "RUB"]),
            rails: json!(["card", "bank_card"]),
            min_amount_minor: Some(1_000),
            max_amount_minor: Some(10_000_000),
            fee_model: json!({}),
            risk_score: 15,
            last_seen_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn mock_quote_source_returns_success_quote() {
        let source = MockRouteQuoteSource::default();
        let order = order(OrderStatus::Quoting);
        let solver = solver(SolverStatus::Active, FAST_LOW_LIMIT_SLUG);
        let outcome = source
            .quote(RouteQuoteRequest {
                order: order.clone(),
                solver: solver.clone(),
                behavior: FakeSolverBehavior::Success,
            })
            .await;

        let RouteQuoteOutcome::Success { quote } = outcome else {
            panic!("expected quote success");
        };
        assert_eq!(quote.order_id, order.id);
        assert_eq!(quote.solver_id, solver.id);
        assert_eq!(quote.target_amount_minor, 19_700);
        assert_eq!(quote.status, QuoteStatus::Valid);
    }

    #[tokio::test]
    async fn mock_quote_source_can_reject_and_timeout() {
        let source = MockRouteQuoteSource::default();
        let order = order(OrderStatus::Quoting);
        let solver = solver(SolverStatus::Active, FAST_LOW_LIMIT_SLUG);

        let rejected = source
            .quote(RouteQuoteRequest {
                order: order.clone(),
                solver: solver.clone(),
                behavior: FakeSolverBehavior::Reject,
            })
            .await;
        assert!(matches!(rejected, RouteQuoteOutcome::Rejected { .. }));

        let timed_out = source
            .quote(RouteQuoteRequest {
                order,
                solver,
                behavior: FakeSolverBehavior::Timeout,
            })
            .await;
        assert!(matches!(timed_out, RouteQuoteOutcome::TimedOut { .. }));
    }

    #[test]
    fn quote_validation_rejects_expired_quote_and_inactive_solver() {
        let now = Utc::now();
        let order = order(OrderStatus::Quoting);
        let active_solver = solver(SolverStatus::Active, FAST_LOW_LIMIT_SLUG);
        let profile = fake_solvers()
            .into_iter()
            .find(|solver| solver.slug == FAST_LOW_LIMIT_SLUG)
            .expect("fast solver exists");
        let mut quote = build_quote(&order, &active_solver, &profile, now).expect("valid quote");

        quote.expires_at = now;
        assert!(validate_quote(&order, &active_solver, &quote, now)
            .unwrap_err()
            .to_string()
            .contains("expired"));

        quote.expires_at = now + Duration::minutes(5);
        let inactive_solver = solver(SolverStatus::Paused, FAST_LOW_LIMIT_SLUG);
        assert!(validate_quote(&order, &inactive_solver, &quote, now)
            .unwrap_err()
            .to_string()
            .contains("not active"));
    }
}

use std::collections::HashMap;
use std::time::Duration as StdDuration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::model::{ExchangeOrder, ExchangeQuote, ExchangeSolver};
use crate::exchange::repo;
use crate::exchange::solver::{
    FakeSolverBehavior, RouteQuoteOutcome, RouteQuoteRequest, RouteQuoteSource,
};
use crate::exchange::status::{OrderStatus, QuoteStatus};

pub const AUCTION_WINDOW: StdDuration = StdDuration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
pub struct AuctionResult {
    pub order: ExchangeOrder,
    pub selected_quote: ExchangeQuote,
    pub scored_quotes: Vec<ScoredQuote>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredQuote {
    pub quote: ExchangeQuote,
    pub score: i64,
}

#[derive(Debug, Clone)]
pub struct AuctionQuoteCandidate {
    pub quote: ExchangeQuote,
    pub solver: ExchangeSolver,
}

pub async fn run_auction_for_order<S>(
    pool: &DbPool,
    order: &ExchangeOrder,
    quote_source: &S,
) -> Result<AuctionResult>
where
    S: RouteQuoteSource,
{
    ensure_auction_can_run(order)?;
    if order.status == OrderStatus::Quoted {
        return existing_selection(pool, order).await;
    }

    sleep(AUCTION_WINDOW).await;

    let solvers = repo::solvers_for_order(pool, order).await?;
    for solver in &solvers {
        if !solver.status.accepts_quotes() {
            continue;
        }
        if let RouteQuoteOutcome::Success { quote } = quote_source
            .quote(RouteQuoteRequest {
                order: order.clone(),
                solver: solver.clone(),
                behavior: FakeSolverBehavior::Success,
            })
            .await
        {
            repo::insert_quote(pool, &quote).await?;
        }
    }

    let quotes = repo::quotes_for_order(pool, &order.id).await?;
    let solver_by_id: HashMap<Uuid, ExchangeSolver> =
        solvers.into_iter().map(|solver| (solver.id, solver)).collect();
    let candidates = quotes
        .into_iter()
        .filter_map(|quote| {
            solver_by_id
                .get(&quote.solver_id)
                .cloned()
                .map(|solver| AuctionQuoteCandidate { quote, solver })
        })
        .collect::<Vec<_>>();

    let now = Utc::now();
    let scored_quotes = scored_candidates(order, &candidates, now);
    let winner = choose_winner_from_scored(&scored_quotes).context("no valid exchange quotes")?;
    let order = repo::select_quote_for_order(pool, order, &winner.quote, winner.score).await?;
    let selected_quote = repo::quote_by_id(pool, &winner.quote.id)
        .await?
        .context("selected exchange quote not found after auction")?;

    Ok(AuctionResult {
        order,
        selected_quote,
        scored_quotes,
    })
}

pub fn scored_candidates(
    order: &ExchangeOrder,
    candidates: &[AuctionQuoteCandidate],
    now: DateTime<Utc>,
) -> Vec<ScoredQuote> {
    let mut scored = candidates
        .iter()
        .filter(|candidate| quote_is_eligible(order, candidate, now))
        .map(|candidate| ScoredQuote {
            quote: candidate.quote.clone(),
            score: score_quote(&candidate.quote),
        })
        .collect::<Vec<_>>();
    scored.sort_by(winner_order);
    scored
}

pub fn choose_winner(candidates: &[ScoredQuote]) -> Option<ScoredQuote> {
    candidates.iter().cloned().min_by(winner_order)
}

pub fn score_quote(quote: &ExchangeQuote) -> i64 {
    let manual_review_penalty = quote
        .settlement_plan
        .get("requires_manual_review")
        .and_then(serde_json::Value::as_bool)
        .map_or(0, |required| if required { 5_000 } else { 0 });

    quote.target_amount_minor
        - quote.fee_minor
        - i64::from(quote.eta_minutes) * 10
        - i64::from(quote.risk_score) * 100
        - manual_review_penalty
}

fn ensure_auction_can_run(order: &ExchangeOrder) -> Result<()> {
    match order.status {
        OrderStatus::Quoting | OrderStatus::Quoted => Ok(()),
        OrderStatus::Locked => bail!("winner cannot change after locked"),
        other => bail!("exchange order cannot run auction from {}", other.as_str()),
    }
}

async fn existing_selection(pool: &DbPool, order: &ExchangeOrder) -> Result<AuctionResult> {
    let quote_id = order
        .selected_quote_id
        .context("quoted exchange order has no selected quote")?;
    let selected_quote = repo::quote_by_id(pool, &quote_id)
        .await?
        .context("selected exchange quote not found")?;
    Ok(AuctionResult {
        order: order.clone(),
        selected_quote,
        scored_quotes: Vec::new(),
    })
}

fn quote_is_eligible(
    order: &ExchangeOrder,
    candidate: &AuctionQuoteCandidate,
    now: DateTime<Utc>,
) -> bool {
    let quote = &candidate.quote;
    let solver = &candidate.solver;
    solver.status.accepts_quotes()
        && matches!(quote.status, QuoteStatus::Received | QuoteStatus::Valid)
        && quote.order_id == order.id
        && quote.solver_id == solver.id
        && quote.source_amount_minor == order.source_amount_minor
        && quote.source_currency == order.source_currency
        && quote.target_currency == order.target_currency
        && quote.expires_at > now
        && quote.target_amount_minor > 0
        && order
            .target_amount_min_minor
            .map_or(true, |min| quote.target_amount_minor >= min)
        && solver
            .min_amount_minor
            .map_or(true, |min| quote.source_amount_minor >= min)
        && solver
            .max_amount_minor
            .map_or(true, |max| quote.source_amount_minor <= max)
}

fn choose_winner_from_scored(candidates: &[ScoredQuote]) -> Option<ScoredQuote> {
    candidates.first().cloned()
}

fn winner_order(left: &ScoredQuote, right: &ScoredQuote) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.quote.risk_score.cmp(&right.quote.risk_score))
        .then_with(|| left.quote.created_at.cmp(&right.quote.created_at))
        .then_with(|| left.quote.id.cmp(&right.quote.id))
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::json;

    use super::*;
    use crate::exchange::status::{FundingInstructionStatus, SolverStatus};

    fn order() -> ExchangeOrder {
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
            status: OrderStatus::Quoting,
            deadline_at: None,
            selected_quote_id: None,
            failure_code: None,
            failure_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn solver(status: SolverStatus, risk_score: i32) -> ExchangeSolver {
        ExchangeSolver {
            id: Uuid::new_v4(),
            slug: format!("solver-{risk_score}"),
            actor_id: None,
            handle: None,
            display_name: "Solver".into(),
            status,
            countries: json!(["AM", "RU"]),
            currencies: json!(["AMD", "RUB"]),
            rails: json!(["card", "bank_card"]),
            min_amount_minor: Some(1_000),
            max_amount_minor: Some(10_000_000),
            fee_model: json!({}),
            risk_score,
            last_seen_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn quote(
        order: &ExchangeOrder,
        solver: &ExchangeSolver,
        target_amount_minor: i64,
        fee_minor: i64,
        eta_minutes: i32,
        risk_score: i32,
        created_at: DateTime<Utc>,
    ) -> ExchangeQuote {
        ExchangeQuote {
            id: Uuid::new_v4(),
            order_id: order.id,
            solver_id: solver.id,
            source_amount_minor: order.source_amount_minor,
            target_amount_minor,
            source_currency: order.source_currency.clone(),
            target_currency: order.target_currency.clone(),
            funding_method_type: order.source_method_type.clone(),
            requires_user_funding: true,
            rate: "0.200000000000".into(),
            fee_minor,
            eta_minutes,
            expires_at: Utc::now() + Duration::minutes(5),
            status: QuoteStatus::Valid,
            settlement_plan: json!({}),
            risk_score,
            score: None,
            raw_response: None,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn scoring_prefers_better_net_quote() {
        let order = order();
        let fast_solver = solver(SolverStatus::Active, 15);
        let slow_solver = solver(SolverStatus::Active, 25);
        let now = Utc::now();
        let fast = quote(&order, &fast_solver, 19_700, 300, 5, 15, now);
        let slow = quote(
            &order,
            &slow_solver,
            21_350,
            150,
            45,
            25,
            now + Duration::seconds(1),
        );
        let scored = scored_candidates(
            &order,
            &[
                AuctionQuoteCandidate {
                    quote: fast,
                    solver: fast_solver,
                },
                AuctionQuoteCandidate {
                    quote: slow.clone(),
                    solver: slow_solver,
                },
            ],
            now,
        );

        let winner = choose_winner(&scored).expect("winner");
        assert_eq!(winner.quote.id, slow.id);
        assert_eq!(winner.score, score_quote(&slow));
    }

    #[test]
    fn filters_expired_invalid_and_inactive_quotes() {
        let order = order();
        let active_solver = solver(SolverStatus::Active, 15);
        let paused_solver = solver(SolverStatus::Paused, 15);
        let now = Utc::now();
        let valid = quote(&order, &active_solver, 20_000, 100, 10, 15, now);
        let mut expired = quote(&order, &active_solver, 21_000, 100, 10, 15, now);
        expired.expires_at = now - Duration::seconds(1);
        let mut invalid = quote(&order, &active_solver, 22_000, 100, 10, 15, now);
        invalid.status = QuoteStatus::Invalid;
        let inactive = quote(&order, &paused_solver, 23_000, 100, 10, 15, now);

        let scored = scored_candidates(
            &order,
            &[
                AuctionQuoteCandidate {
                    quote: expired,
                    solver: active_solver.clone(),
                },
                AuctionQuoteCandidate {
                    quote: invalid,
                    solver: active_solver.clone(),
                },
                AuctionQuoteCandidate {
                    quote: inactive,
                    solver: paused_solver,
                },
                AuctionQuoteCandidate {
                    quote: valid.clone(),
                    solver: active_solver,
                },
            ],
            now,
        );

        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].quote.id, valid.id);
    }

    #[test]
    fn tie_break_prefers_lower_risk_then_earlier_quote() {
        let order = order();
        let lower_risk_solver = solver(SolverStatus::Active, 10);
        let higher_risk_solver = solver(SolverStatus::Active, 20);
        let now = Utc::now();
        let higher_risk = quote(&order, &higher_risk_solver, 21_000, 0, 10, 20, now);
        let lower_risk = quote(&order, &lower_risk_solver, 20_000, 0, 10, 10, now);

        let scored = scored_candidates(
            &order,
            &[
                AuctionQuoteCandidate {
                    quote: higher_risk,
                    solver: higher_risk_solver,
                },
                AuctionQuoteCandidate {
                    quote: lower_risk.clone(),
                    solver: lower_risk_solver,
                },
            ],
            now,
        );
        assert_eq!(choose_winner(&scored).expect("winner").quote.id, lower_risk.id);

        let solver_a = solver(SolverStatus::Active, 10);
        let solver_b = solver(SolverStatus::Active, 10);
        let early = quote(&order, &solver_a, 20_000, 0, 10, 10, now);
        let late = quote(
            &order,
            &solver_b,
            20_000,
            0,
            10,
            10,
            now + Duration::seconds(1),
        );
        let scored = scored_candidates(
            &order,
            &[
                AuctionQuoteCandidate {
                    quote: late,
                    solver: solver_b,
                },
                AuctionQuoteCandidate {
                    quote: early.clone(),
                    solver: solver_a,
                },
            ],
            now,
        );
        assert_eq!(choose_winner(&scored).expect("winner").quote.id, early.id);
    }

    #[test]
    fn locked_order_rejects_auction() {
        let mut order = order();
        order.status = OrderStatus::Locked;
        assert!(ensure_auction_can_run(&order)
            .unwrap_err()
            .to_string()
            .contains("winner cannot change"));
    }
}

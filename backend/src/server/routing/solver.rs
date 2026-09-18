use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::exchange::solver::{
    FakeSolverBehavior, MockRouteQuoteSource, RouteQuoteOutcome, RouteQuoteRequest,
    RouteQuoteSource, FAST_LOW_LIMIT_SLUG,
};
use crate::exchange::{repo, ExchangeOrder, ExchangeQuote};

const DEFAULT_OPEN_ORDER_LIMIT: i64 = 20;
const MAX_OPEN_ORDER_LIMIT: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct OpenOrdersQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitQuoteReq {
    pub solver_slug: Option<String>,
    #[serde(default)]
    pub behavior: FakeSolverBehavior,
}

#[derive(Debug, Serialize)]
pub struct SubmitQuoteRes {
    pub order_id: Uuid,
    pub solver_slug: String,
    #[serde(flatten)]
    pub outcome: SubmitQuoteOutcome,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum SubmitQuoteOutcome {
    Success { quote: ExchangeQuote },
    Rejected { reason: String },
    TimedOut { after_ms: u64 },
}

pub async fn open_orders(
    State(state): State<AppState>,
    Query(query): Query<OpenOrdersQuery>,
) -> Result<Json<Vec<ExchangeOrder>>, AppError> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_OPEN_ORDER_LIMIT)
        .clamp(1, MAX_OPEN_ORDER_LIMIT);
    let orders = repo::open_solver_orders(&state.pool, limit)
        .await
        .map_err(AppError::from)?;
    Ok(Json(orders))
}

pub async fn submit_quote(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(req): Json<SubmitQuoteReq>,
) -> Result<Json<SubmitQuoteRes>, AppError> {
    let order = repo::order_by_id(&state.pool, &order_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange order not found".into()))?;
    let solver_slug = req
        .solver_slug
        .as_deref()
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .unwrap_or(FAST_LOW_LIMIT_SLUG)
        .to_string();
    let solver = repo::solver_by_slug(&state.pool, &solver_slug)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange solver not found".into()))?;

    let quote_source = MockRouteQuoteSource::default();
    let outcome = quote_source
        .quote(RouteQuoteRequest {
            order,
            solver,
            behavior: req.behavior,
        })
        .await;

    let outcome = match outcome {
        RouteQuoteOutcome::Success { quote } => {
            let quote = repo::insert_quote(&state.pool, &quote)
                .await
                .map_err(AppError::from)?;
            SubmitQuoteOutcome::Success { quote }
        }
        RouteQuoteOutcome::Rejected { reason } => SubmitQuoteOutcome::Rejected { reason },
        RouteQuoteOutcome::TimedOut { after_ms } => SubmitQuoteOutcome::TimedOut { after_ms },
    };

    Ok(Json(SubmitQuoteRes {
        order_id,
        solver_slug,
        outcome,
    }))
}

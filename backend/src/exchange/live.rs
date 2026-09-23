use std::time::Duration as StdDuration;

use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::sleep;
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::auction::{score_quote, AuctionQuoteCandidate, ScoredQuote};
use crate::exchange::model::{ExchangeOrder, ExchangeSolver, Minor, NewExchangeQuote};
use crate::exchange::repo;
use crate::exchange::status::{OrderStatus, QuoteStatus};

const QUOTE_TTL_MINUTES: i64 = 5;

#[derive(Debug, Clone)]
struct MockRouteProfile {
    asset: String,
    network: String,
    entry_provider: String,
    exit_provider: String,
    entry_delay_ms: u64,
    exit_delay_ms: u64,
    rate_bps: i64,
    fee_minor: Minor,
    eta_minutes: i32,
    spread_bps: i32,
    risk_score: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveRouteEvent {
    OrderStatus {
        order_id: Uuid,
        status: OrderStatus,
    },
    SearchStarted {
        order_id: Uuid,
    },
    EntryLegFound {
        order_id: Uuid,
        route_id: Uuid,
        status: RouteCandidateStatus,
        source_amount_minor: Minor,
        source_currency: String,
        entry_asset: String,
        entry_network: String,
        spread_bps: i32,
        legs: Vec<RouteLeg>,
    },
    ExitSearchStarted {
        order_id: Uuid,
        route_id: Uuid,
        entry_asset: String,
        entry_network: String,
    },
    RouteCandidateFound {
        order_id: Uuid,
        route_id: Uuid,
        quote_id: Uuid,
        status: RouteCandidateStatus,
        source_amount_minor: Minor,
        source_currency: String,
        entry_asset: String,
        entry_network: String,
        target_amount_minor: Minor,
        target_currency: String,
        spread_bps: i32,
        fee_minor: Minor,
        eta_minutes: i32,
        is_current_best: bool,
        legs: Vec<RouteLeg>,
    },
    BestRouteUpdated {
        order_id: Uuid,
        route_id: Uuid,
        quote_id: Uuid,
        target_amount_minor: Minor,
        target_currency: String,
        spread_bps: i32,
        score: i64,
    },
    RouteRejected {
        order_id: Uuid,
        route_id: Uuid,
        entry_asset: String,
        entry_network: String,
        reason: String,
    },
    SearchFinished {
        order_id: Uuid,
        best_quote_id: Option<Uuid>,
    },
    SearchFailed {
        order_id: Uuid,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteCandidateStatus {
    Partial,
    Complete,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteLeg {
    pub kind: &'static str,
    pub from: String,
    pub to: String,
    pub provider: String,
    pub status: &'static str,
}

pub fn run_mock_live_search(pool: DbPool, order: ExchangeOrder) -> mpsc::Receiver<LiveRouteEvent> {
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        if let Err(err) = live_search_task(pool, order.clone(), tx.clone()).await {
            let _ = tx
                .send(LiveRouteEvent::SearchFailed {
                    order_id: order.id,
                    error: err.to_string(),
                })
                .await;
        }
    });
    rx
}

async fn live_search_task(
    pool: DbPool,
    order: ExchangeOrder,
    tx: mpsc::Sender<LiveRouteEvent>,
) -> Result<()> {
    let order = ensure_search_order_state(&pool, order).await?;
    send(
        &tx,
        LiveRouteEvent::OrderStatus {
            order_id: order.id,
            status: order.status,
        },
    )
    .await?;
    send(&tx, LiveRouteEvent::SearchStarted { order_id: order.id }).await?;

    let solver = select_solver(&pool, &order).await?;
    let profiles = load_mock_profiles(&pool).await?;
    let mut route_rx = spawn_mock_route_tasks(order.clone(), solver, profiles);
    let mut best: Option<ScoredQuote> = None;

    while let Some(result) = route_rx.recv().await {
        let route_id = result.route_id;
        match result.outcome {
            RouteOutcome::EntryFound => {
                send(
                    &tx,
                    LiveRouteEvent::EntryLegFound {
                        order_id: order.id,
                        route_id,
                        status: RouteCandidateStatus::Partial,
                        source_amount_minor: order.source_amount_minor,
                        source_currency: order.source_currency.clone(),
                        entry_asset: result.profile.asset.to_string(),
                        entry_network: result.profile.network.to_string(),
                        spread_bps: result.profile.spread_bps,
                        legs: partial_legs(&order, &result.profile),
                    },
                )
                .await?;
                send(
                    &tx,
                    LiveRouteEvent::ExitSearchStarted {
                        order_id: order.id,
                        route_id,
                        entry_asset: result.profile.asset.to_string(),
                        entry_network: result.profile.network.to_string(),
                    },
                )
                .await?;
            }
            RouteOutcome::Quote { quote, solver } => {
                let quote = repo::insert_quote(&pool, &quote).await?;
                let scored = ScoredQuote {
                    score: score_quote(&quote),
                    quote: quote.clone(),
                };
                let is_current_best = best
                    .as_ref()
                    .map_or(true, |current| scored.score > current.score);

                send(
                    &tx,
                    LiveRouteEvent::RouteCandidateFound {
                        order_id: order.id,
                        route_id,
                        quote_id: quote.id,
                        status: RouteCandidateStatus::Complete,
                        source_amount_minor: quote.source_amount_minor,
                        source_currency: quote.source_currency.clone(),
                        entry_asset: result.profile.asset.to_string(),
                        entry_network: result.profile.network.to_string(),
                        target_amount_minor: quote.target_amount_minor,
                        target_currency: quote.target_currency.clone(),
                        spread_bps: result.profile.spread_bps,
                        fee_minor: quote.fee_minor,
                        eta_minutes: quote.eta_minutes,
                        is_current_best,
                        legs: complete_legs(&order, &result.profile),
                    },
                )
                .await?;

                let candidate = AuctionQuoteCandidate { quote, solver };
                if is_current_best
                    && crate::exchange::auction::scored_candidates(&order, &[candidate], Utc::now())
                        .first()
                        .is_some()
                {
                    let quote = scored.quote.clone();
                    send(
                        &tx,
                        LiveRouteEvent::BestRouteUpdated {
                            order_id: order.id,
                            route_id,
                            quote_id: quote.id,
                            target_amount_minor: quote.target_amount_minor,
                            target_currency: quote.target_currency,
                            spread_bps: result.profile.spread_bps,
                            score: scored.score,
                        },
                    )
                    .await?;
                    best = Some(scored);
                }
            }
            RouteOutcome::Rejected { reason } => {
                send(
                    &tx,
                    LiveRouteEvent::RouteRejected {
                        order_id: order.id,
                        route_id,
                        entry_asset: result.profile.asset.to_string(),
                        entry_network: result.profile.network.to_string(),
                        reason,
                    },
                )
                .await?;
            }
        }
    }

    send(
        &tx,
        LiveRouteEvent::SearchFinished {
            order_id: order.id,
            best_quote_id: best.map(|scored| scored.quote.id),
        },
    )
    .await?;
    Ok(())
}

async fn ensure_search_order_state(pool: &DbPool, order: ExchangeOrder) -> Result<ExchangeOrder> {
    match order.status {
        OrderStatus::Created => {
            repo::transition_order_status(
                pool,
                &order.id,
                OrderStatus::Created,
                OrderStatus::Discovering,
            )
            .await?;
            repo::transition_order_status(
                pool,
                &order.id,
                OrderStatus::Discovering,
                OrderStatus::Quoting,
            )
            .await?;
        }
        OrderStatus::Discovering => {
            repo::transition_order_status(
                pool,
                &order.id,
                OrderStatus::Discovering,
                OrderStatus::Quoting,
            )
            .await?;
        }
        OrderStatus::Quoting | OrderStatus::Quoted => {}
        OrderStatus::Locked => bail!("winner cannot change after locked"),
        status if status.is_terminal() => bail!("exchange order is already terminal"),
        other => bail!(
            "exchange order cannot run live search from {}",
            other.as_str()
        ),
    }

    repo::order_by_id(pool, &order.id)
        .await?
        .context("exchange order not found after live search status update")
}

async fn select_solver(pool: &DbPool, order: &ExchangeOrder) -> Result<ExchangeSolver> {
    repo::solvers_for_order(pool, order)
        .await?
        .into_iter()
        .find(|solver| solver.status.accepts_quotes())
        .context("no active exchange solver available for live search")
}

fn spawn_mock_route_tasks(
    order: ExchangeOrder,
    solver: ExchangeSolver,
    profiles: Vec<MockRouteProfile>,
) -> mpsc::Receiver<RouteSearchResult> {
    let (tx, rx) = mpsc::channel(profiles.len().max(1));
    for profile in profiles {
        let tx = tx.clone();
        let order = order.clone();
        let solver = solver.clone();
        tokio::spawn(async move {
            let route_id = Uuid::new_v4();
            sleep(StdDuration::from_millis(profile.entry_delay_ms)).await;
            let _ = tx
                .send(RouteSearchResult {
                    route_id,
                    profile: profile.clone(),
                    outcome: RouteOutcome::EntryFound,
                })
                .await;
            sleep(StdDuration::from_millis(profile.exit_delay_ms)).await;
            let outcome = build_live_quote(&order, &solver, &profile)
                .map(|quote| RouteOutcome::Quote {
                    quote,
                    solver: solver.clone(),
                })
                .unwrap_or_else(|err| RouteOutcome::Rejected {
                    reason: err.to_string(),
                });
            let _ = tx
                .send(RouteSearchResult {
                    route_id,
                    profile,
                    outcome,
                })
                .await;
        });
    }
    drop(tx);
    rx
}

#[derive(Debug)]
struct RouteSearchResult {
    route_id: Uuid,
    profile: MockRouteProfile,
    outcome: RouteOutcome,
}

#[derive(Debug)]
enum RouteOutcome {
    EntryFound,
    Quote {
        quote: NewExchangeQuote,
        solver: ExchangeSolver,
    },
    Rejected {
        reason: String,
    },
}

fn build_live_quote(
    order: &ExchangeOrder,
    solver: &ExchangeSolver,
    profile: &MockRouteProfile,
) -> Result<NewExchangeQuote> {
    let target_amount_minor =
        order.source_amount_minor * profile.rate_bps / 10_000 - profile.fee_minor;
    if target_amount_minor <= 0 {
        bail!("route target amount is not positive");
    }
    if let Some(min) = order.target_amount_min_minor {
        if target_amount_minor < min {
            bail!("route target amount is below minimum");
        }
    }

    Ok(NewExchangeQuote {
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
        expires_at: Utc::now() + Duration::minutes(QUOTE_TTL_MINUTES),
        status: QuoteStatus::Valid,
        settlement_plan: json!({
            "type": "mock_live_route",
            "solver_slug": &solver.slug,
            "entry": {
                "from": &order.source_currency,
                "to": &profile.asset,
                "network": &profile.network,
                "provider": &profile.entry_provider,
            },
            "exit": {
                "from": &profile.asset,
                "network": &profile.network,
                "to": &order.target_currency,
                "provider": &profile.exit_provider,
            },
            "requires_manual_review": false,
        }),
        risk_score: profile.risk_score,
        score: None,
        raw_response: Some(json!({
            "source": "mock_live_search",
            "asset": &profile.asset,
            "network": &profile.network,
            "entry_provider": &profile.entry_provider,
            "exit_provider": &profile.exit_provider,
            "spread_bps": profile.spread_bps,
        })),
    })
}

async fn load_mock_profiles(pool: &DbPool) -> Result<Vec<MockRouteProfile>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            r#"
SELECT asset, network, entry_provider, exit_provider, entry_delay_ms,
       exit_delay_ms, rate_bps, fee_minor, eta_minutes, spread_bps, risk_score
FROM mock_route_profiles
WHERE status = 'enabled'
ORDER BY slug
"#,
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| MockRouteProfile {
            asset: row.get("asset"),
            network: row.get("network"),
            entry_provider: row.get("entry_provider"),
            exit_provider: row.get("exit_provider"),
            entry_delay_ms: row.get::<_, i64>("entry_delay_ms") as u64,
            exit_delay_ms: row.get::<_, i64>("exit_delay_ms") as u64,
            rate_bps: row.get("rate_bps"),
            fee_minor: row.get("fee_minor"),
            eta_minutes: row.get("eta_minutes"),
            spread_bps: row.get("spread_bps"),
            risk_score: row.get("risk_score"),
        })
        .collect())
}

fn partial_legs(order: &ExchangeOrder, profile: &MockRouteProfile) -> Vec<RouteLeg> {
    vec![
        RouteLeg {
            kind: "entry",
            from: order.source_currency.clone(),
            to: profile.asset.to_string(),
            provider: profile.entry_provider.to_string(),
            status: "found",
        },
        RouteLeg {
            kind: "exit",
            from: profile.asset.to_string(),
            to: order.target_currency.clone(),
            provider: profile.exit_provider.to_string(),
            status: "searching",
        },
    ]
}

fn complete_legs(order: &ExchangeOrder, profile: &MockRouteProfile) -> Vec<RouteLeg> {
    vec![
        RouteLeg {
            kind: "entry",
            from: order.source_currency.clone(),
            to: profile.asset.to_string(),
            provider: profile.entry_provider.to_string(),
            status: "found",
        },
        RouteLeg {
            kind: "exit",
            from: profile.asset.to_string(),
            to: order.target_currency.clone(),
            provider: profile.exit_provider.to_string(),
            status: "found",
        },
    ]
}

async fn send(tx: &mpsc::Sender<LiveRouteEvent>, event: LiveRouteEvent) -> Result<()> {
    tx.send(event).await.context("live route receiver closed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::status::{FundingInstructionStatus, SolverStatus};
    use serde_json::Value;

    fn test_profiles() -> Vec<MockRouteProfile> {
        [
            (
                "USDT",
                "ERC20",
                "am-p2p-mock",
                "ru-buyer-mock",
                120,
                160,
                2_035,
                1_200,
                12,
                8,
                18,
            ),
            (
                "ETH",
                "Ethereum",
                "am-eth-desk-mock",
                "ru-eth-buyer-mock",
                180,
                220,
                2_018,
                1_800,
                18,
                0,
                22,
            ),
            (
                "BTC",
                "Binance",
                "am-btc-p2p-mock",
                "ru-btc-rail-mock",
                90,
                300,
                2_012,
                2_100,
                24,
                -8,
                28,
            ),
            (
                "SOL",
                "Solana",
                "am-sol-liquidity-mock",
                "ru-sol-buyer-mock",
                240,
                130,
                2_026,
                1_500,
                15,
                3,
                20,
            ),
        ]
        .into_iter()
        .map(
            |(
                asset,
                network,
                entry,
                exit,
                entry_delay_ms,
                exit_delay_ms,
                rate_bps,
                fee_minor,
                eta_minutes,
                spread_bps,
                risk_score,
            )| MockRouteProfile {
                asset: asset.into(),
                network: network.into(),
                entry_provider: entry.into(),
                exit_provider: exit.into(),
                entry_delay_ms,
                exit_delay_ms,
                rate_bps,
                fee_minor,
                eta_minutes,
                spread_bps,
                risk_score,
            },
        )
        .collect()
    }

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
            target_amount_min_minor: Some(1),
            target_method_type: "bank_card".into(),
            target_method_ref: None,
            funding_instruction_id: None,
            funding_status: FundingInstructionStatus::NotStarted,
            status: OrderStatus::Quoting,
            correlation_id: Uuid::new_v4(),
            deadline_at: None,
            selected_quote_id: None,
            failure_code: None,
            failure_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn solver() -> ExchangeSolver {
        ExchangeSolver {
            id: Uuid::new_v4(),
            slug: "fast-low-limit".into(),
            actor_id: None,
            handle: None,
            display_name: "Fast Low Limit".into(),
            status: SolverStatus::Active,
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

    #[test]
    fn mock_profiles_cover_plan2_assets() {
        let assets = test_profiles()
            .into_iter()
            .map(|profile| (profile.asset, profile.network))
            .collect::<Vec<_>>();

        assert_eq!(
            assets,
            vec![
                ("USDT".to_string(), "ERC20".to_string()),
                ("ETH".to_string(), "Ethereum".to_string()),
                ("BTC".to_string(), "Binance".to_string()),
                ("SOL".to_string(), "Solana".to_string()),
            ]
        );
    }

    #[test]
    fn live_quote_contains_route_plan_and_positive_target() {
        let order = order();
        let solver = solver();
        let profile = test_profiles().remove(0);

        let quote = build_live_quote(&order, &solver, &profile).expect("quote is valid");
        assert_eq!(quote.source_currency, "AMD");
        assert_eq!(quote.target_currency, "RUB");
        assert!(quote.target_amount_minor > 0);
        assert_eq!(quote.status, QuoteStatus::Valid);
        assert_eq!(
            quote.settlement_plan["entry"]["to"],
            Value::String("USDT".into())
        );
        assert_eq!(
            quote.settlement_plan["exit"]["to"],
            Value::String("RUB".into())
        );
    }

    #[test]
    fn partial_and_complete_legs_expose_expected_statuses() {
        let order = order();
        let profile = test_profiles().remove(0);

        let partial = partial_legs(&order, &profile);
        assert_eq!(partial[0].status, "found");
        assert_eq!(partial[1].status, "searching");

        let complete = complete_legs(&order, &profile);
        assert_eq!(complete[0].status, "found");
        assert_eq!(complete[1].status, "found");
    }
}

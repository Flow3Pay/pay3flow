use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::AppError;
use crate::core::state::AppState;
use crate::exchange::{
    auction, discovery, ledger, live, repo, solver, AuditEvent, ExchangeOrder, ExchangeProof,
    ExchangeQuote, ExchangeSettlement, FundingInstruction, Minor, NewExchangeOrder, OrderStatus,
    TokenLedgerOperation,
};
use crate::server::routing::payments::auth_user_id;

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
const DEFAULT_LIST_LIMIT: i64 = 20;
const MAX_LIST_LIMIT: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct CreateOrderReq {
    pub source_country: String,
    pub source_currency: String,
    pub source_amount_minor: Minor,
    pub source_method_type: String,
    pub source_method_ref: Option<String>,
    pub target_country: String,
    pub target_currency: String,
    pub target_amount_min_minor: Option<Minor>,
    pub target_method_type: String,
    pub target_method_ref: Option<String>,
    pub deadline_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListOrdersQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LiveRoutesQuery {
    pub access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmOrderReq {
    pub quote_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ConfirmOrderRes {
    pub order: ExchangeOrder,
    pub funding_instruction: FundingInstruction,
    pub settlement: ExchangeSettlement,
}

#[derive(Debug, Serialize)]
pub struct FundingConfirmRes {
    pub order: ExchangeOrder,
    pub funding_instruction: FundingInstruction,
    pub settlement: ExchangeSettlement,
}

#[derive(Debug, Deserialize)]
pub struct SubmitProofReq {
    pub proof_type: String,
    pub proof_payload: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct SubmitProofRes {
    pub order: ExchangeOrder,
    pub settlement: ExchangeSettlement,
    pub proof: ExchangeProof,
}

pub async fn create_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateOrderReq>,
) -> Result<Json<ExchangeOrder>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let idempotency_key = idempotency_key(&headers)?;
    validate_create_order(&req)?;

    let order = NewExchangeOrder {
        user_id,
        idempotency_key,
        source_country: normalized_code(req.source_country),
        source_currency: normalized_code(req.source_currency),
        source_amount_minor: req.source_amount_minor,
        source_method_type: req.source_method_type.trim().to_string(),
        source_method_ref: clean_optional(req.source_method_ref),
        target_country: normalized_code(req.target_country),
        target_currency: normalized_code(req.target_currency),
        target_amount_min_minor: req.target_amount_min_minor,
        target_method_type: req.target_method_type.trim().to_string(),
        target_method_ref: clean_optional(req.target_method_ref),
        deadline_at: req.deadline_at,
    };

    let order = repo::create_order_idempotent(&state.pool, &order)
        .await
        .map_err(map_exchange_err)?;
    tracing::info!(
        order_id = %order.id,
        correlation_id = %order.correlation_id,
        status = %order.status.as_str(),
        "exchange.order.created"
    );
    Ok(Json(order))
}

pub async fn get_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ExchangeOrder>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    Ok(Json(order))
}

pub async fn list_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListOrdersQuery>,
) -> Result<Json<Vec<ExchangeOrder>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let orders = repo::orders_for_user(&state.pool, &user_id, limit)
        .await
        .map_err(AppError::from)?;
    Ok(Json(orders))
}

pub async fn get_quotes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ExchangeQuote>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let quotes = repo::quotes_for_order(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(quotes))
}

pub async fn live_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LiveRoutesQuery>,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let user_id = match query
        .access_token
        .as_deref()
        .filter(|token| !token.is_empty())
    {
        Some(token) => user_id_from_token(&state, token)?,
        None => auth_user_id(&state, &headers)?,
    };
    let order = owned_order(&state, &user_id, &id).await?;
    Ok(ws.on_upgrade(move |socket| live_routes_socket(state, order, socket)))
}

async fn live_routes_socket(state: AppState, order: ExchangeOrder, mut socket: WebSocket) {
    let mut events = live::run_mock_live_search(state.pool.clone(), order);
    while let Some(event) = events.recv().await {
        let payload = match serde_json::to_string(&event) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::error!(error = %err, "exchange.live.serialize_failed");
                break;
            }
        };
        if socket.send(Message::Text(payload)).await.is_err() {
            break;
        }
    }
    let _ = socket.close().await;
}

pub async fn discover_solvers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<discovery::SolverDiscoveryResult>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let result =
        discovery::discover_solvers_for_order(&state.pool, &state.ap, state.redis.as_ref(), &order)
            .await
            .map_err(map_exchange_err)?;
    tracing::info!(
        order_id = %order.id,
        correlation_id = %order.correlation_id,
        candidates = result.candidates.len(),
        source = %result.source.as_str(),
        "exchange.discovery.completed"
    );
    Ok(Json(result))
}

pub async fn run_auction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<auction::AuctionResult>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let quote_source = solver::MockRouteQuoteSource::default();
    let result = auction::run_auction_for_order(&state.pool, &order, &quote_source)
        .await
        .map_err(map_exchange_err)?;
    tracing::info!(
        order_id = %result.order.id,
        correlation_id = %result.order.correlation_id,
        quote_id = %result.selected_quote.id,
        status = %result.order.status.as_str(),
        "exchange.auction.completed"
    );
    Ok(Json(result))
}

pub async fn cancel_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ExchangeOrder>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    if order.status.is_terminal() {
        return Err(AppError::Conflict(
            "exchange order is already terminal".into(),
        ));
    }
    if !order.status.can_transition(OrderStatus::Cancelled) {
        return Err(AppError::Conflict(format!(
            "exchange order cannot be cancelled from {}",
            order.status.as_str()
        )));
    }

    let changed = repo::cancel_order(&state.pool, &order.id, order.status)
        .await
        .map_err(map_exchange_err)?;
    if !changed {
        return Err(AppError::Conflict(
            "exchange order status changed; retry with latest state".into(),
        ));
    }
    let order = repo::order_by_id(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange order not found".into()))?;
    Ok(Json(order))
}

pub async fn confirm_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ConfirmOrderReq>,
) -> Result<Json<ConfirmOrderRes>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;

    if let Some(instruction_id) = order.funding_instruction_id {
        let instruction = repo::funding_instruction_by_id(&state.pool, &instruction_id)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("funding instruction not found".into()))?;
        let quote = repo::quote_by_id(&state.pool, &instruction.quote_id)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::NotFound("exchange quote not found".into()))?;
        let settlement = repo::ensure_settlement_for_order(&state.pool, &order, &quote)
            .await
            .map_err(map_exchange_err)?;
        return Ok(Json(ConfirmOrderRes {
            order,
            funding_instruction: instruction,
            settlement,
        }));
    }

    let quote_id = req
        .quote_id
        .or(order.selected_quote_id)
        .ok_or_else(|| AppError::BadRequest("quote_id is required".into()))?;
    let quote = repo::quote_by_id(&state.pool, &quote_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange quote not found".into()))?;
    if quote.order_id != order.id {
        return Err(AppError::NotFound("exchange quote not found".into()));
    }

    let (order, funding_instruction) =
        repo::create_funding_instruction_for_quote(&state.pool, &order, &quote)
            .await
            .map_err(map_exchange_err)?;
    let settlement = repo::settlement_by_order_id(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange settlement not found".into()))?;
    tracing::info!(
        order_id = %order.id,
        correlation_id = %order.correlation_id,
        funding_instruction_id = %funding_instruction.id,
        settlement_id = %settlement.id,
        status = %order.status.as_str(),
        "exchange.order.confirmed"
    );
    Ok(Json(ConfirmOrderRes {
        order,
        funding_instruction,
        settlement,
    }))
}

pub async fn confirm_funding(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<FundingConfirmRes>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let instruction_id = order
        .funding_instruction_id
        .ok_or_else(|| AppError::BadRequest("funding instruction is not created".into()))?;
    let instruction = repo::funding_instruction_by_id(&state.pool, &instruction_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("funding instruction not found".into()))?;

    let (order, funding_instruction) =
        repo::confirm_funding_instruction(&state.pool, &order, &instruction)
            .await
            .map_err(map_exchange_err)?;
    let (order, funding_instruction, settlement) =
        repo::execute_settlement_after_user_funding(&state.pool, &order, &funding_instruction)
            .await
            .map_err(map_exchange_err)?;
    tracing::info!(
        order_id = %order.id,
        correlation_id = %order.correlation_id,
        funding_instruction_id = %funding_instruction.id,
        settlement_id = %settlement.id,
        status = %order.status.as_str(),
        "exchange.funding.confirmed"
    );
    Ok(Json(FundingConfirmRes {
        order,
        funding_instruction,
        settlement,
    }))
}

pub async fn get_settlement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ExchangeSettlement>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let settlement = repo::settlement_by_order_id(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange settlement not found".into()))?;
    Ok(Json(settlement))
}

pub async fn get_proofs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ExchangeProof>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let settlement = repo::settlement_by_order_id(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange settlement not found".into()))?;
    let proofs = repo::proofs_for_settlement(&state.pool, &settlement.id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(proofs))
}

pub async fn get_ledger_operations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TokenLedgerOperation>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let operations = ledger::operations_for_order(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(operations))
}

pub async fn get_audit_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AuditEvent>>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let events = repo::audit_events_for_entity(&state.pool, "exchange_order", &order.id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(events))
}

pub async fn submit_proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<SubmitProofReq>,
) -> Result<Json<SubmitProofRes>, AppError> {
    let user_id = auth_user_id(&state, &headers)?;
    let order = owned_order(&state, &user_id, &id).await?;
    let settlement = repo::settlement_by_order_id(&state.pool, &order.id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange settlement not found".into()))?;
    let (order, settlement, proof) = repo::submit_and_verify_proof(
        &state.pool,
        &order,
        &settlement,
        &req.proof_type,
        req.proof_payload,
    )
    .await
    .map_err(map_exchange_err)?;
    tracing::info!(
        order_id = %order.id,
        correlation_id = %order.correlation_id,
        settlement_id = %settlement.id,
        proof_id = %proof.id,
        proof_status = %proof.verification_status.as_str(),
        status = %order.status.as_str(),
        "exchange.proof.submitted"
    );
    Ok(Json(SubmitProofRes {
        order,
        settlement,
        proof,
    }))
}

async fn owned_order(
    state: &AppState,
    user_id: &Uuid,
    id: &Uuid,
) -> Result<ExchangeOrder, AppError> {
    let order = repo::order_by_id(&state.pool, id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound("exchange order not found".into()))?;
    ensure_order_owner(&order, user_id)?;
    Ok(order)
}

fn ensure_order_owner(order: &ExchangeOrder, user_id: &Uuid) -> Result<(), AppError> {
    if order.user_id != *user_id {
        return Err(AppError::NotFound("exchange order not found".into()));
    }
    Ok(())
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, AppError> {
    headers
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest("Idempotency-Key header is required".into()))
}

fn validate_create_order(req: &CreateOrderReq) -> Result<(), AppError> {
    if req.source_amount_minor <= 0 {
        return Err(AppError::BadRequest(
            "source_amount_minor must be greater than 0".into(),
        ));
    }
    if matches!(req.target_amount_min_minor, Some(amount) if amount < 0) {
        return Err(AppError::BadRequest(
            "target_amount_min_minor must not be negative".into(),
        ));
    }
    validate_country("source_country", &req.source_country)?;
    validate_country("target_country", &req.target_country)?;
    validate_currency("source_currency", &req.source_currency)?;
    validate_currency("target_currency", &req.target_currency)?;
    validate_non_empty("source_method_type", &req.source_method_type)?;
    validate_non_empty("target_method_type", &req.target_method_type)?;
    validate_deadline(req.deadline_at)?;
    Ok(())
}

fn validate_country(name: &str, value: &str) -> Result<(), AppError> {
    validate_code(name, value, 2)
}

fn validate_currency(name: &str, value: &str) -> Result<(), AppError> {
    validate_code(name, value, 3)
}

fn validate_code(name: &str, value: &str, len: usize) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.len() != len || !trimmed.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(AppError::BadRequest(format!(
            "{name} must be an ISO-like {len}-letter code"
        )));
    }
    Ok(())
}

fn validate_non_empty(name: &str, value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::BadRequest(format!("{name} must not be empty")));
    }
    Ok(())
}

fn validate_deadline(deadline_at: Option<DateTime<Utc>>) -> Result<(), AppError> {
    let Some(deadline_at) = deadline_at else {
        return Ok(());
    };
    let now = Utc::now();
    if deadline_at <= now {
        return Err(AppError::BadRequest(
            "deadline_at must be in the future".into(),
        ));
    }
    if deadline_at > now + Duration::days(30) {
        return Err(AppError::BadRequest(
            "deadline_at must be within 30 days".into(),
        ));
    }
    Ok(())
}

fn normalized_code(value: String) -> String {
    value.trim().to_ascii_uppercase()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn user_id_from_token(state: &AppState, token: &str) -> Result<Uuid, AppError> {
    let sub = state
        .jwt
        .verify(token)
        .map_err(|_| AppError::Unauthorized("invalid or expired token".into()))?;
    Uuid::parse_str(&sub).map_err(|_| AppError::Unauthorized("malformed token subject".into()))
}

fn map_exchange_err(err: anyhow::Error) -> AppError {
    let msg = err.to_string();
    if msg.contains("corridor")
        || msg.contains("amount")
        || msg.contains("quote")
        || msg.contains("funding instruction")
        || msg.contains("transition")
        || msg.contains("discover")
        || msg.contains("solver")
        || msg.contains("winner")
        || msg.contains("auction")
        || msg.contains("ledger")
        || msg.contains("settlement")
        || msg.contains("proof")
        || msg.contains("release")
        || msg.contains("rollback")
        || msg.contains("no valid")
        || msg.contains("expired")
        || msg.contains("must be")
        || msg.contains("already")
    {
        AppError::BadRequest(msg)
    } else {
        AppError::Internal(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::FundingInstructionStatus;
    use axum::http::HeaderValue;

    fn valid_req() -> CreateOrderReq {
        CreateOrderReq {
            source_country: "am".into(),
            source_currency: "amd".into(),
            source_amount_minor: 1000,
            source_method_type: "card".into(),
            source_method_ref: None,
            target_country: "ru".into(),
            target_currency: "rub".into(),
            target_amount_min_minor: Some(1),
            target_method_type: "card".into(),
            target_method_ref: None,
            deadline_at: Some(Utc::now() + Duration::minutes(10)),
        }
    }

    fn order_for(user_id: Uuid) -> ExchangeOrder {
        ExchangeOrder {
            id: Uuid::new_v4(),
            user_id,
            idempotency_key: "idem-1".into(),
            source_country: "AM".into(),
            source_currency: "AMD".into(),
            source_amount_minor: 1000,
            source_method_type: "card".into(),
            source_method_ref: None,
            target_country: "RU".into(),
            target_currency: "RUB".into(),
            target_amount_min_minor: Some(1),
            target_method_type: "card".into(),
            target_method_ref: None,
            funding_instruction_id: None,
            funding_status: FundingInstructionStatus::NotStarted,
            status: OrderStatus::Created,
            correlation_id: Uuid::new_v4(),
            deadline_at: None,
            selected_quote_id: None,
            failure_code: None,
            failure_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn create_order_validation_accepts_iso_like_codes_and_positive_amount() {
        assert!(validate_create_order(&valid_req()).is_ok());
    }

    #[test]
    fn create_order_validation_rejects_bad_amount_and_codes() {
        let mut req = valid_req();
        req.source_amount_minor = 0;
        assert!(validate_create_order(&req).is_err());

        let mut req = valid_req();
        req.source_country = "ARM".into();
        assert!(validate_create_order(&req).is_err());

        let mut req = valid_req();
        req.target_currency = "".into();
        assert!(validate_create_order(&req).is_err());
    }

    #[test]
    fn create_order_validation_rejects_unsane_deadlines() {
        let mut req = valid_req();
        req.deadline_at = Some(Utc::now() - Duration::seconds(1));
        assert!(validate_create_order(&req).is_err());

        let mut req = valid_req();
        req.deadline_at = Some(Utc::now() + Duration::days(31));
        assert!(validate_create_order(&req).is_err());
    }

    #[test]
    fn idempotency_header_is_required_and_trimmed() {
        let mut headers = HeaderMap::new();
        assert!(idempotency_key(&headers).is_err());

        headers.insert(IDEMPOTENCY_HEADER, HeaderValue::from_static(" idem-1 "));
        assert_eq!(idempotency_key(&headers).unwrap(), "idem-1");
    }

    #[test]
    fn ownership_guard_hides_other_users_orders() {
        let user_id = Uuid::new_v4();
        let order = order_for(user_id);
        assert!(ensure_order_owner(&order, &user_id).is_ok());

        let err = ensure_order_owner(&order, &Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }
}

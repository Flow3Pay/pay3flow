use super::{
    booking::{QuoteRequest, RfqRequest},
    error::{invalid, Error, Result},
    operations::IncidentAction,
    transfers::{PrepareRequest, SubmitRequest},
    wallet::{ChallengeRequest, LinkRequest, ProofRequest},
};
use crate::core::state::AppState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/otc/config", get(config))
        .route("/api/otc/session", get(session_info).post(link))
        .route("/api/otc/session/challenge", post(login_challenge))
        .route("/api/otc/session/proof", post(login_prove))
        .route("/api/otc/session/revoke", post(revoke))
        .route("/api/otc/wallet/challenge", post(challenge))
        .route("/api/otc/wallet/proof", post(prove))
        .route("/api/otc/listings", get(listings))
        .route("/api/otc/rfqs", post(rfq))
        .route("/api/otc/trades", get(trades))
        .route("/api/otc/trades/:id", get(details))
        .route("/api/otc/trades/:id/apply", post(apply))
        .route("/api/otc/trades/:id/prepare", post(prepare))
        .route("/api/otc/trades/:id/escrow", get(escrow_details))
        .route("/api/otc/trades/:id/escrow/consent", post(escrow_consent))
        .route("/api/otc/trades/:id/escrow/proposals", post(escrow_propose))
        .route("/api/otc/trades/:id/escrow/approvals", post(escrow_approve))
        .route("/api/otc/trades/:id/escrow/prepare", post(escrow_prepare))
        .route(
            "/api/otc/trades/:id/escrow/reference",
            post(escrow_reference),
        )
        .route("/api/otc/attempts/:id/reference", post(submit))
        .route("/api/otc/attempts/:id/retry", post(retry))
        .route("/api/otc/desk/listings", post(publish))
        .route("/api/otc/desk/quotes", post(quote))
        .route("/api/otc/desk/trades/:id/decision", post(decide))
        .route("/api/otc/desk/control", post(control))
        .route("/api/otc/desk/accounting", post(accounting))
        .route("/api/otc/desk/dashboard", get(dashboard))
        .route("/api/otc/desk/metrics", get(metrics))
        .route("/api/otc/desk/incidents/:key", post(incident))
        .layer(axum::middleware::from_fn(no_store))
}
fn bearer(headers: &HeaderMap) -> Result<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(Error::Unauthorized)
}
fn key(headers: &HeaderMap) -> Result<&str> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| invalid("Idempotency-Key required (1–128 characters)"))
}
async fn config(State(state): State<AppState>) -> Json<Value> {
    let cfg = &state.otc.cfg;
    let available = state.otc.ready().await.is_ok();
    let wallet_login_chains: Vec<_> = [
        (cfg.ethereum_rpc.len() >= 2).then_some("ethereum"),
        (cfg.everscale_graphql.len() >= 2 && !cfg.everwallet_code_hashes.is_empty())
            .then_some("everscale"),
    ]
    .into_iter()
    .flatten()
    .collect();
    Json(
        json!({"enabled":cfg.enabled,"available":available,"wallet_login_available":state.otc.wallet_login_available(),"wallet_login_chains":wallet_login_chains,"escrow":cfg.escrow,"escrow_lifetime_seconds":14400,"escrow_max_ever":"100000","demo":cfg.demo,"desk":cfg.desk_actor,"desk_name":cfg.desk_name,"ever_resource":cfg.ever_resource,"usdt_resource":cfg.usdt_resource,"support_owner":cfg.support_owner,"ever_network":"Everscale","usdt_network":"Ethereum","usdt_contract":super::config::USDT,"profile":super::protocol::profile(),"fee":{"payer":"desk","rate":"0.0025","floor":"5","currency":"USDT"},"counterparty_risk":if cfg.escrow.is_some(){"USDT funds a 2-of-3 Safe escrow before native EVER delivery. Expiry permits a USDT refund; it cannot reverse EVER delivery."}else{"Two separate transfers. The desk must deliver or refund; there is no atomic swap or automatic rollback."}}),
    )
}
async fn link(
    State(state): State<AppState>,
    Json(request): Json<LinkRequest>,
) -> Result<Json<Value>> {
    Ok(Json(state.otc.link(request).await?))
}
async fn session_info(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    Ok(Json(state.otc.session_info(bearer(&headers)?).await?))
}
async fn login_challenge(
    State(state): State<AppState>,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<Value>> {
    Ok(Json(state.otc.login_challenge(request).await?))
}
async fn login_prove(
    State(state): State<AppState>,
    Json(request): Json<ProofRequest>,
) -> Result<Json<Value>> {
    Ok(Json(state.otc.login_prove(request).await?))
}
async fn revoke(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (session, _) = state.otc.session(bearer(&headers)?).await?;
    let client = state.pool.get().await?;
    client
        .execute(
            "UPDATE otc_sessions SET revoked=TRUE WHERE id=$1",
            &[&session],
        )
        .await?;
    Ok(Json(json!({"revoked":true})))
}
async fn challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChallengeRequest>,
) -> Result<Json<Value>> {
    let (id, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.challenge(id, &actor, request).await?))
}
async fn prove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProofRequest>,
) -> Result<Json<Value>> {
    let (id, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.prove(id, &actor, request).await?))
}
async fn listings(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(state.otc.listings().await?))
}
async fn rfq(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RfqRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.rfq(&actor, key(&headers)?, request).await?))
}
async fn trades(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.trades(&actor).await?))
}
async fn details(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.details(&actor, id).await?))
}
async fn apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.apply(&actor, id).await?))
}
async fn prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<PrepareRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .prepare(&actor, id, key(&headers)?, &request.kind)
            .await?,
    ))
}
async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<SubmitRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.submit(&actor, id, request).await?))
}
async fn publish(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.publish_listings(&actor).await?))
}
async fn quote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<QuoteRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.quote(&actor, request).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    accept: bool,
}
async fn decide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<Decision>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.decide(&actor, id, request.accept).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    paused: bool,
}
async fn control(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Control>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.control(&actor, request.paused).await?))
}
async fn dashboard(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.dashboard(&actor).await?))
}
async fn incident(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(request): Json<IncidentAction>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state.otc.incident_action(&actor, &key, request).await?,
    ))
}

async fn accounting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<super::accounting::EntryRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.accounting_entry(&actor, request).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retry {
    evidence: Value,
}
async fn retry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<Retry>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .authorize_retry(&actor, id, &request.evidence)
            .await?,
    ))
}

async fn metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok((
        [("Content-Type", "text/plain; version=0.0.4")],
        state.otc.metrics(&actor).await?,
    ))
}

async fn no_store(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}

async fn escrow_details(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.escrow_details(&actor, id).await?))
}
async fn escrow_consent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::escrow::ConsentRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .escrow_consent(&actor, id, key(&headers)?, request)
            .await?,
    ))
}
async fn escrow_propose(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::escrow::ProposalRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .escrow_propose(&actor, id, key(&headers)?, request)
            .await?,
    ))
}
async fn escrow_approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::escrow::ApprovalRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    key(&headers)?;
    Ok(Json(state.otc.escrow_approve(&actor, id, request).await?))
}
async fn escrow_prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::escrow::CallRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .escrow_prepare(&actor, id, key(&headers)?, request)
            .await?,
    ))
}
async fn escrow_reference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::escrow::ReferenceRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    key(&headers)?;
    Ok(Json(state.otc.escrow_reference(&actor, id, request).await?))
}

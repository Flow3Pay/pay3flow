use super::{
    booking::{QuoteRequest, RfqRequest},
    error::{invalid, Error, Result},
    operations::IncidentAction,
    p2p::{AcceptOfferRequest, DealPrepareRequest, DealSubmitRequest},
    transfers::{PrepareRequest, SubmitRequest},
    wallet::{ChallengeRequest, LinkRequest, ProofRequest},
};
use crate::core::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
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
        .route("/api/otc/history", get(history))
        .route(
            "/api/otc/offers",
            get(public_offers).post(post_public_offer),
        )
        .route("/api/otc/offers/mine", get(my_public_offers))
        .route("/api/otc/offers/history", get(public_offer_history))
        .route("/api/otc/offers/:id/cancel", post(cancel_public_offer))
        .route("/api/otc/offers/:id/accept", post(accept_offer))
        .route("/api/otc/volumes", get(volumes))
        .route("/api/otc/p2p/trades", get(p2p_trades))
        .route("/api/otc/p2p/assets", get(p2p_assets))
        .route("/api/otc/p2p/trades/:id", get(p2p_deal))
        .route("/api/otc/p2p/trades/:id/prepare", post(p2p_prepare))
        .route("/api/otc/p2p/trades/:id/confirm", post(p2p_confirm))
        .route("/api/otc/p2p/trades/:id/cancel", post(p2p_cancel))
        .route("/api/otc/p2p/attempts/:id/reference", post(p2p_submit))
        .route("/api/otc/rfqs", post(rfq))
        .route("/api/otc/trades", get(trades))
        .route("/api/otc/balances", get(balances))
        .route("/api/otc/multisig/review", post(multisig_review))
        .route("/api/otc/multisig/prepare", post(multisig_prepare))
        .route("/api/otc/multisig/calls/:id", get(multisig_call))
        .route(
            "/api/otc/multisig/calls/:id/reference",
            post(multisig_reference),
        )
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
        state
            .otc
            .wallet_login_chain_available("ethereum")
            .then_some("ethereum"),
        state
            .otc
            .wallet_login_chain_available("everscale")
            .then_some("everscale"),
    ]
    .into_iter()
    .flatten()
    .collect();
    Json(
        json!({"enabled":cfg.enabled,"available":available,"public_offers_available":state.otc.public_offers_available(),"public_offer_directions":["buy","sell"],"p2p_available":state.otc.p2p_available(),"ethereum_chain_id":cfg.ethereum_chain_id,"usdt_contract":cfg.usdt_contract,"usdc_contract":super::config::USDC,"everscale_multisig_available":state.otc.multisig_available(),"wallet_login_available":state.otc.wallet_login_available(),"wallet_login_chains":wallet_login_chains,"wallet_verification_days":super::repo::WALLET_VERIFICATION_DAYS,"escrow":cfg.escrow,"escrow_lifetime_seconds":14400,"escrow_max_ever":"100000","demo":cfg.demo,"desk":cfg.desk_actor,"desk_name":cfg.desk_name,"ever_resource":cfg.ever_resource,"usdt_resource":cfg.usdt_resource,"support_owner":cfg.support_owner,"ever_network":"Everscale","usdt_network":"Ethereum","p2p_ethereum_chain_id":cfg.p2p_chain(),"p2p_usdt_contract":cfg.p2p_usdt(),"p2p_usdc_contract":super::config::USDC,"p2p_usdt_network":"Ethereum","profile":super::protocol::profile(),"fee":{"payer":"desk","rate":"0.0025","floor":"5","currency":"USDT"},"counterparty_risk":if cfg.escrow.is_some(){"USDT funds a 2-of-3 Safe escrow before native EVER delivery. Expiry permits a USDT refund; it cannot reverse EVER delivery."}else{"Two separate transfers. The payer of each leg is responsible for it; there is no atomic swap or automatic rollback."}}),
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
async fn public_offers(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(state.otc.public_offers(None).await?))
}
async fn public_offer_history(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(state.otc.public_offer_history().await?))
}
/// Settled EVER/USDT volume for the last 24 hours, 7 days and all time.
async fn volumes(State(state): State<AppState>) -> Result<Json<Value>> {
    Ok(Json(state.otc.volumes().await?))
}
/// Time-bucketed chart data: candles and book levels from the desk's own
/// records, real exchange OHLC for the selected range, plus the indicative
/// series the converter is priced from.
async fn history(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>> {
    let market = params
        .get("market")
        .map(String::as_str)
        .unwrap_or("EVER-USDT");
    if market != "EVER-USDT" {
        return Err(invalid("unsupported market"));
    }
    let window = super::history::Window::parse(
        params.get("range").map(String::as_str).unwrap_or("1m"),
        Utc::now(),
    )?;
    let (candles, levels) = super::history::aggregate(&state.otc.history(&window).await?, &window);
    let (period, span, bucket_seconds) = window.spot_layout();
    let (points, stale) = state.market_prices.spot_history("EVER", period, span).await;
    let (exchange, exchange_stale) = state
        .market_prices
        .exchange_candles("EVER", window.market_interval, window.market_limit())
        .await;
    let market_candles = super::history::aggregate_market(&exchange, &window);
    Ok(Json(json!({
        "market": market,
        "range": window.range,
        "bucket_seconds": window.bucket_seconds,
        "from": window.from,
        "to": window.to,
        "market_candles": {
            "source": "Gate.io",
            "stale": exchange_stale,
            "interval": window.market_interval,
            "candles": market_candles,
        },
        "spot": {
            "source": "Gate.io",
            "stale": stale,
            "bucket_seconds": bucket_seconds,
            "points": points,
        },
        "candles": candles,
        "levels": levels,
    })))
}
async fn my_public_offers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.public_offers(Some(&actor)).await?))
}
async fn post_public_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(draft): Json<super::public_offers::PublicOfferDraft>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .post_public_offer(&actor, key(&headers)?, draft)
            .await?,
    ))
}
async fn cancel_public_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.cancel_public_offer(&actor, id).await?))
}
async fn accept_offer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    destination: Option<Json<AcceptOfferRequest>>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .accept_offer_with_destination(
                &actor,
                id,
                key(&headers)?,
                destination.map(|Json(request)| request).unwrap_or_default(),
            )
            .await?,
    ))
}
async fn p2p_trades(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.p2p_trades(&actor).await?))
}
async fn p2p_assets(State(state): State<AppState>) -> Json<Value> {
    let Some(provider) = state.otc.near_intents.as_ref() else {
        return Json(json!({"assets":[],"destination_assets":[],"buy_destination_assets":[]}));
    };
    let tokens = provider.supported_tokens().await;
    let mut candidates = Vec::new();
    for token in tokens {
        let symbol = token.symbol.to_ascii_uppercase();
        let network = crate::route_engine::canonical_network_id(&token.blockchain);
        if !matches!(symbol.as_str(), "USDT" | "USDC")
            || token.decimals.is_none()
            || token
                .contract_address
                .as_deref()
                .is_none_or(|address| address.trim().is_empty())
            || !crate::route_execution::supports_otc_wallet_network(
                &crate::route_engine::canonical_network_id(&token.blockchain),
            )
        {
            continue;
        }
        if network == "ethereum" {
            let expected = if symbol == "USDT" {
                super::config::USDT
            } else {
                super::config::USDC
            };
            if token.decimals != Some(6)
                || token
                    .contract_address
                    .as_deref()
                    .is_none_or(|address| !address.eq_ignore_ascii_case(expected))
            {
                continue;
            }
        }
        let Ok(asset) = token.asset() else {
            continue;
        };
        if provider
            .token_for_asset_id(&asset, &token.asset_id)
            .await
            .is_err()
        {
            continue;
        }
        candidates.push((asset, token));
    }

    let mut assets = Vec::new();
    let mut destination_assets = Vec::new();
    let mut buy_destination_assets = Vec::new();
    for (destination, token) in &candidates {
        if token.decimals != Some(6) {
            continue;
        }
        let symbol = token.symbol.to_ascii_uppercase();
        let network = destination.location.as_deref().unwrap_or_default();
        let Some(fee_recipient) = state.otc.cfg.fee_recipients.get(network) else {
            continue;
        };
        if crate::route_execution::validate_otc_wallet_address(network, fee_recipient).is_err() {
            continue;
        }
        let Ok(fee_asset) = crate::route_engine::Asset::new("USDT", Some(network)) else {
            continue;
        };
        let Ok(fee_token) = provider.unique_token_for_asset(&fee_asset).await else {
            continue;
        };
        if fee_token.decimals != Some(6)
            || fee_token
                .contract_address
                .as_deref()
                .is_none_or(|address| address.trim().is_empty())
        {
            continue;
        }
        let direct_network = state.otc.p2p_direct_network_available(network);
        let is_ethereum = network == "ethereum";
        if is_ethereum {
            let expected = if symbol == "USDT" {
                super::config::USDT
            } else {
                super::config::USDC
            };
            if token
                .contract_address
                .as_deref()
                .is_none_or(|address| !address.eq_ignore_ascii_case(expected))
            {
                continue;
            }
        }
        let output = json!({
            "asset": destination,
            "symbol": symbol,
            "network": destination.location,
            "asset_id": token.asset_id,
            "decimals": token.decimals,
            "contract_address": token.contract_address,
        });
        let mut any_route = false;
        for (source, source_token) in &candidates {
            let payment_route = (direct_network
                && source == destination
                && source_token.asset_id == token.asset_id)
                || (source != destination && provider.route_available(source, destination).await);
            if !payment_route {
                continue;
            }
            let fee_route = if source == &fee_asset && source_token.asset_id == fee_token.asset_id {
                direct_network
            } else {
                provider.route_available(source, &fee_asset).await
            };
            if fee_route {
                any_route = true;
                break;
            }
        }
        if any_route {
            destination_assets.push(output.clone());
        }
        let buy_source = match crate::route_engine::Asset::new("USDT", Some("ethereum")) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let Ok(buy_source_token) = provider.unique_token_for_asset(&buy_source).await else {
            continue;
        };
        let buy_payment_route = (direct_network
            && &buy_source == destination
            && symbol == "USDT"
            && buy_source_token.asset_id == token.asset_id)
            || (&buy_source != destination
                && provider.route_available(&buy_source, destination).await);
        let buy_fee_route =
            if buy_source == fee_asset && buy_source_token.asset_id == fee_token.asset_id {
                direct_network
            } else {
                provider.route_available(&buy_source, &fee_asset).await
            };
        let buy_route = buy_payment_route && buy_fee_route;
        if buy_route {
            buy_destination_assets.push(output);
        }
    }

    for (source, token) in &candidates {
        let mut destination_asset_ids = Vec::new();
        for (destination, destination_token) in &candidates {
            if destination_token.decimals != Some(6) {
                continue;
            }
            let network = destination.location.as_deref().unwrap_or_default();
            let Some(fee_recipient) = state.otc.cfg.fee_recipients.get(network) else {
                continue;
            };
            if crate::route_execution::validate_otc_wallet_address(network, fee_recipient).is_err()
            {
                continue;
            }
            let Ok(fee_asset) = crate::route_engine::Asset::new("USDT", Some(network)) else {
                continue;
            };
            let Ok(fee_token) = provider.unique_token_for_asset(&fee_asset).await else {
                continue;
            };
            if fee_token.decimals != Some(6)
                || fee_token
                    .contract_address
                    .as_deref()
                    .is_none_or(|address| address.trim().is_empty())
            {
                continue;
            }
            let direct_network = state.otc.p2p_direct_network_available(network);
            let is_exact_direct = direct_network
                && source == destination
                && token.asset_id == destination_token.asset_id;
            let payment_route = is_exact_direct
                || (source != destination && provider.route_available(source, destination).await);
            let fee_route = if source == &fee_asset && token.asset_id == fee_token.asset_id {
                direct_network
            } else {
                provider.route_available(source, &fee_asset).await
            };
            if payment_route && fee_route {
                destination_asset_ids.push(destination_token.asset_id.clone());
            }
        }
        if destination_asset_ids.is_empty() {
            continue;
        }
        assets.push(json!({
            "asset": source,
            "symbol": token.symbol.to_ascii_uppercase(),
            "network": source.location,
            "asset_id": token.asset_id,
            "decimals": token.decimals,
            "contract_address": token.contract_address,
            "destination_asset_ids": destination_asset_ids,
        }));
    }
    let sort_assets = |items: &mut Vec<Value>| {
        items.sort_by(|left, right| {
            left["symbol"]
                .as_str()
                .cmp(&right["symbol"].as_str())
                .then_with(|| left["network"].as_str().cmp(&right["network"].as_str()))
                .then_with(|| left["asset_id"].as_str().cmp(&right["asset_id"].as_str()))
        })
    };
    sort_assets(&mut assets);
    sort_assets(&mut destination_assets);
    sort_assets(&mut buy_destination_assets);
    Json(json!({
        "assets":assets,
        "destination_assets":destination_assets,
        "buy_destination_assets":buy_destination_assets
    }))
}
async fn p2p_deal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.p2p_deal(&actor, id).await?))
}
async fn p2p_prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<DealPrepareRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .p2p_prepare(&actor, id, key(&headers)?, request)
            .await?,
    ))
}
async fn p2p_cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.p2p_cancel(&actor, id).await?))
}
async fn p2p_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.p2p_confirm(&actor, id).await?))
}
async fn p2p_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<DealSubmitRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state.otc.p2p_submit(&actor, id, &request.reference).await?,
    ))
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
async fn balances(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.balances(&actor).await))
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

async fn multisig_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<super::multisig::ReviewRequest>,
) -> Result<Json<Value>> {
    state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.multisig_review(request).await?))
}
async fn multisig_prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<super::multisig::PrepareRequest>,
) -> Result<Json<Value>> {
    let (_, actor) = state.otc.session(bearer(&headers)?).await?;
    Ok(Json(
        state
            .otc
            .multisig_prepare(&actor, key(&headers)?, request)
            .await?,
    ))
}
async fn multisig_call(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.multisig_call(id).await?))
}
async fn multisig_reference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<super::multisig::ReferenceRequest>,
) -> Result<Json<Value>> {
    state.otc.session(bearer(&headers)?).await?;
    Ok(Json(state.otc.multisig_reference(id, request).await?))
}

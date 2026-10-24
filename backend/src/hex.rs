//! Read-only HEX GNK market transport. Wallets sign and publish on the client.
use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

pub const CONTRACT: &str = "orderbook.fi.tg";
pub const GNK: &str = "nep245:v2_1.omni.hot.tg:4444119_wyixUKCL";
pub const USDT: &str = "nep141:usdt.tether-token.near";
const RPC: &str = "https://rpc.mainnet.near.org";
const INDEXER: &str = "https://hex-api.app/orderbook/v1/orderbook.fi.tg/transactions";
type Error = (StatusCode, Json<Value>);
type Result<T> = std::result::Result<T, Error>;

fn bad(message: &str) -> Error {
    (StatusCode::BAD_REQUEST, Json(json!({"error": message})))
}
fn unavailable(message: &str) -> Error {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error": message})),
    )
}
fn pair() -> Value {
    json!({"base": GNK, "quote": USDT})
}

#[derive(Clone)]
struct Service {
    http: reqwest::Client,
    snapshot: Arc<Mutex<Option<(Instant, Value)>>>,
}
impl Service {
    async fn view(&self, contract: &str, method: &str, args: Value) -> Result<Value> {
        use base64::Engine;
        let args = base64::engine::general_purpose::STANDARD.encode(args.to_string());
        let response: Value = self
            .http
            .post(RPC)
            .json(&json!({
                "jsonrpc":"2.0", "id":"pay3flow-hex", "method":"query",
                "params":{"request_type":"call_function", "finality":"final", "account_id":contract,
                    "method_name":method, "args_base64":args}
            }))
            .send()
            .await
            .map_err(|_| unavailable("HEX chain connection unavailable"))?
            .error_for_status()
            .map_err(|_| unavailable("HEX chain request failed"))?
            .json()
            .await
            .map_err(|_| unavailable("Invalid HEX chain response"))?;
        let bytes: Vec<u8> = serde_json::from_value(response["result"]["result"].clone())
            .map_err(|_| unavailable("HEX contract request failed"))?;
        let mut value = serde_json::from_slice(&bytes)
            .map_err(|_| unavailable("Invalid HEX contract response"))?;
        lossless(&mut value);
        Ok(value)
    }
}

// Contract timestamps and integer balances can exceed JavaScript's exact range.
fn lossless(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(lossless),
        Value::Object(values) => values.values_mut().for_each(lossless),
        Value::Number(n) if n.as_u64().is_some_and(|v| v > 9_007_199_254_740_991) => {
            *value = json!(n.to_string())
        }
        _ => {}
    }
}

pub fn router() -> Router<crate::core::state::AppState> {
    let service = Service {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("HEX HTTP client"),
        snapshot: Arc::new(Mutex::new(None)),
    };
    Router::new()
        .route("/api/hex/config", get(config))
        .route("/api/hex/snapshot", get(snapshot))
        .route("/api/hex/account", get(account))
        .route("/api/hex/history", get(history))
        .route("/api/hex/quote", post(quote))
        .layer(Extension(service))
}

async fn config() -> Json<Value> {
    Json(
        json!({"market":"GNK-USDT", "contract":CONTRACT, "base_token":GNK, "quote_token":USDT,
        "base_decimals":9, "quote_decimals":6,
        "execution_enabled":std::env::var("HEX_TRADING_ENABLED").is_ok_and(|s| s == "true")}),
    )
}
async fn snapshot(Extension(service): Extension<Service>) -> Result<Json<Value>> {
    let mut cache = service.snapshot.lock().await;
    if let Some((at, value)) = &*cache {
        if at.elapsed() < Duration::from_secs(2) {
            return Ok(Json(value.clone()));
        }
    }
    let (book, fees) = tokio::try_join!(
        service.view(
            CONTRACT,
            "get_orderbook",
            json!({"token_pair":pair(), "depth":100, "with_extra":true})
        ),
        service.view(CONTRACT, "get_protocol_fee", json!({"token_pair":pair()}))
    )?;
    let value = json!({"book":book, "fees":fees, "fetched_at":chrono::Utc::now().to_rfc3339()});
    *cache = Some((Instant::now(), value.clone()));
    Ok(Json(value))
}

#[derive(Deserialize, Default)]
struct AccountQuery {
    account: Option<String>,
    skip: Option<u32>,
    limit: Option<u32>,
}
fn owner(query: &AccountQuery) -> Result<&str> {
    let account = query
        .account
        .as_deref()
        .ok_or_else(|| bad("Account required"))?;
    if !(2..=128).contains(&account.len())
        || !account
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(bad("Invalid Omni account"));
    }
    Ok(account)
}
async fn account(
    Extension(service): Extension<Service>,
    Query(query): Query<AccountQuery>,
) -> Result<Json<Value>> {
    let owner = owner(&query)?;
    let (orders, pending, balances) = tokio::try_join!(
        service.view(CONTRACT, "get_orders_by_owner", json!({"token_pair":pair(), "owner_id":owner,
            "skip":query.skip.unwrap_or(0).min(100_000), "limit":query.limit.unwrap_or(100).clamp(1,100)})),
        service.view(CONTRACT, "get_user_balance", json!({"account_id":owner})),
        service.view("intents.near", "mt_batch_balance_of", json!({"account_id":owner, "token_ids":[GNK,USDT]}))
    )?;
    Ok(Json(
        json!({"account":owner, "orders":orders, "pending":pending, "balances":balances}),
    ))
}
async fn history(
    Extension(service): Extension<Service>,
    Query(query): Query<AccountQuery>,
) -> Result<Json<Value>> {
    let mut request = service
        .http
        .get(INDEXER)
        .query(&[("base_token", GNK), ("quote_token", USDT)]);
    if query.account.is_some() {
        request = request.query(&[("account_id", owner(&query)?)]);
    }
    let mut value: Value = request
        .send()
        .await
        .map_err(|_| unavailable("HEX history unavailable"))?
        .error_for_status()
        .map_err(|_| unavailable("HEX history unavailable"))?
        .json()
        .await
        .map_err(|_| unavailable("Invalid HEX history response"))?;
    let records = value
        .get_mut("transactions")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| unavailable("Invalid HEX history response"))?;
    // Bound downstream payloads even if an upstream ignores pagination.
    let skip = query.skip.unwrap_or(0).min(100_000) as usize;
    let limit = query.limit.unwrap_or(100).clamp(1, 100) as usize;
    let mut result =
        json!({"transactions": records.iter().skip(skip).take(limit).cloned().collect::<Vec<_>>()});
    lossless(&mut result);
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quote {
    side: String,
    order_type: String,
    amount: String,
    price: Option<String>,
}
fn quote_args(quote: &Quote) -> Result<Value> {
    if !matches!(quote.side.as_str(), "buy" | "sell")
        || !matches!(quote.order_type.as_str(), "market" | "limit")
    {
        return Err(bad("Invalid HEX order"));
    }
    if quote.amount.is_empty()
        || quote.amount.len() > 39
        || !quote.amount.bytes().all(|b| b.is_ascii_digit())
        || quote
            .amount
            .parse::<u128>()
            .ok()
            .filter(|v| *v > 0)
            .is_none()
    {
        return Err(bad("Invalid atomic amount"));
    }
    let mut action = json!({"token_pair":pair(), "order_type":quote.order_type});
    if quote.order_type == "limit" {
        let price = quote
            .price
            .as_deref()
            .ok_or_else(|| bad("Limit price required"))?;
        let parts: Vec<_> = price.split('.').collect();
        if price.len() > 64
            || parts.len() > 2
            || parts[0].is_empty()
            || parts
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
            || parts.get(1).is_some_and(|p| p.len() > 24)
            || !price.bytes().any(|b| matches!(b, b'1'..=b'9'))
        {
            return Err(bad("Invalid contract price"));
        }
        action["price"] = json!(price);
    } else {
        action["min_amount_out"] = json!("0");
    }
    Ok(json!({"ext":{"refs":[]}, "amount":quote.amount,
        "token_id":if quote.side == "buy" {USDT} else {GNK}, "order_placement_action":action}))
}
async fn quote(
    Extension(service): Extension<Service>,
    Json(quote): Json<Quote>,
) -> Result<Json<Value>> {
    let result = service
        .view(CONTRACT, "get_quote_pub", quote_args(&quote)?)
        .await?;
    Ok(Json(
        json!({"result":result, "expires_at":(chrono::Utc::now()+chrono::Duration::seconds(15)).to_rfc3339()}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_mapping_and_validation() {
        let mut q = Quote {
            side: "buy".into(),
            order_type: "market".into(),
            amount: "1000000".into(),
            price: None,
        };
        let args = quote_args(&q).unwrap();
        assert_eq!(args["token_id"], USDT);
        assert_eq!(args["ext"]["refs"], json!([]));
        q.side = "sell".into();
        q.order_type = "limit".into();
        q.price = Some("0.0002".into());
        assert_eq!(quote_args(&q).unwrap()["token_id"], GNK);
        for price in ["0", "-1", "1e-3", ".1", "1.", "1.0000000000000000000000001"] {
            q.price = Some(price.into());
            assert!(quote_args(&q).is_err());
        }
        q.price = Some("0.0002".into());
        q.amount = "340282366920938463463374607431768211456".into();
        assert!(quote_args(&q).is_err());
    }
    #[test]
    fn exact_timestamps_and_accounts() {
        let mut v = json!({"created_ts":1778870350271264686u64,"fee":10000});
        lossless(&mut v);
        assert_eq!(v["created_ts"], "1778870350271264686");
        assert_eq!(v["fee"], 10000);
        for account in ["0x1234", "alice.near", "user-hot.tg"] {
            assert!(owner(&AccountQuery {
                account: Some(account.into()),
                ..Default::default()
            })
            .is_ok());
        }
        assert!(owner(&AccountQuery {
            account: Some("../other?".into()),
            ..Default::default()
        })
        .is_err());
    }
}

use super::*;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode as HttpStatus};
use axum::response::IntoResponse;
use axum::routing::get;
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};

const PAIRS: &str = r#"{"status":true,"data":{
"1":{"from":"trx","to":"usdt_trx","min_from":1,"max_from":10000,"min_to":0.1,"max_to":null,"rate":9999,"rate_up":99999},
"2":{"from":"usdt","to":"btc","min_from":1,"max_from":10000,"min_to":0,"max_to":null},
"3":{"from":"eth","to":"eth","min_from":1,"max_from":10000,"min_to":0,"max_to":null}
}}"#;
const RATE: &str = r#"{"status":true,"data":{"swap_rate_id":15390,"currency_from":"trx","currency_to":"usdt_trx","amount":5000,"rate":0.3476539,"result_amount":1737.2695,"network_fee":1,"commission_info":{"total":2.9338975}}}"#;

struct Mock {
    catalog_calls: AtomicUsize,
    quote_calls: AtomicUsize,
    catalog_status: AtomicU16,
    quote_status: AtomicU16,
    pair_response: std::sync::Mutex<String>,
    rate: std::sync::Mutex<String>,
    queries: std::sync::Mutex<Vec<HashMap<String, String>>>,
}

async fn pairs(State(state): State<Arc<Mock>>, headers: HeaderMap) -> impl IntoResponse {
    assert_eq!(headers.get(AUTHORIZATION).unwrap(), "External test-token");
    assert_eq!(headers.get(ACCEPT).unwrap(), "application/json");
    state.catalog_calls.fetch_add(1, Ordering::Relaxed);
    (
        HttpStatus::from_u16(state.catalog_status.load(Ordering::Relaxed)).unwrap(),
        [("content-type", "application/json")],
        state.pair_response.lock().unwrap().clone(),
    )
}
async fn rate(
    State(state): State<Arc<Mock>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    assert_eq!(headers.get(AUTHORIZATION).unwrap(), "External test-token");
    state.quote_calls.fetch_add(1, Ordering::Relaxed);
    state.queries.lock().unwrap().push(query);
    (
        HttpStatus::from_u16(state.quote_status.load(Ordering::Relaxed)).unwrap(),
        [("content-type", "application/json")],
        state.rate.lock().unwrap().clone(),
    )
}
async fn mock() -> (
    CryptoOfficeRouteProvider,
    Arc<Mock>,
    tokio::task::JoinHandle<()>,
) {
    let state = Arc::new(Mock {
        catalog_calls: AtomicUsize::new(0),
        quote_calls: AtomicUsize::new(0),
        catalog_status: AtomicU16::new(200),
        quote_status: AtomicU16::new(200),
        pair_response: std::sync::Mutex::new(PAIRS.into()),
        rate: std::sync::Mutex::new(RATE.into()),
        queries: std::sync::Mutex::new(Vec::new()),
    });
    let app = axum::Router::new()
        .route("/api/v1/swap/list-available-pairs", get(pairs))
        .route("/api/v1/swap/get-rate-info", get(rate))
        .with_state(Arc::clone(&state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider = CryptoOfficeRouteProvider::with_endpoint(
        &format!("http://{}/api/v1/", listener.local_addr().unwrap()),
        "test-token",
    )
    .unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (provider, state, task)
}
fn direction() -> (Asset, Asset, Amount) {
    let from = Asset::parse("TRX@tron").unwrap();
    let to = Asset::parse("USDT@tron").unwrap();
    let amount = Amount::new("5000", from.clone()).unwrap();
    (from, to, amount)
}

#[test]
fn decimal_json_keeps_precision_and_supports_exponents() {
    for (json, expected) in [
        ("0.123456789012345678", "0.123456789012345678"),
        ("1e-8", "0.00000001"),
        ("1.5e2", "150"),
        ("\"0.000000000000000001\"", "0.000000000000000001"),
        ("1.0000000000000000000", "1"),
    ] {
        let decimal: Decimal = serde_json::from_str(json).unwrap();
        assert_eq!(decimal.text().unwrap(), expected);
    }
    for json in [
        "-1", "1e-19", "1e100", "true", "null", "\"NaN\"", "\"1..2\"",
    ] {
        assert!(serde_json::from_str::<Decimal>(json).is_err(), "{json}");
    }
}

#[test]
fn provider_identifiers_keep_chains_distinct_and_auth_is_redacted() {
    assert_eq!(
        provider_asset("usdt_eth").unwrap().to_string(),
        "USDT@ethereum"
    );
    assert_eq!(
        provider_asset("usdt_bnb").unwrap().to_string(),
        "USDT@bnb-smart-chain"
    );
    assert_eq!(provider_asset("usdt_trx").unwrap().to_string(), "USDT@tron");
    assert_eq!(
        provider_asset("usdc_eth").unwrap().to_string(),
        "USDC@ethereum"
    );
    assert_eq!(provider_asset("btc").unwrap().to_string(), "BTC@bitcoin");
    assert_eq!(
        provider_asset("usdt_sol").unwrap().to_string(),
        "USDT@solana"
    );
    assert!(provider_asset("usdt").is_none());
    assert!(provider_asset("usdt_unknown").is_none());
    let provider = CryptoOfficeRouteProvider::with_endpoint(API_URL, "test-token").unwrap();
    let Authorization::Token(header) = provider.authorization else {
        panic!("expected token")
    };
    assert!(!format!("{header:?}").contains("test-token"));
    assert!(CryptoOfficeRouteProvider::with_endpoint(API_URL, "bad\ntoken").is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_only_catalog_directions_and_deducts_the_amount_specific_fee_once() {
    let (provider, state, server) = mock().await;
    let assets = provider.supported_assets().await;
    assert_eq!(
        assets.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["TRX@tron", "USDT@tron"]
    );
    let (from, to, amount) = direction();
    assert!(provider.supports_pair(&from, &to));
    assert!(!provider.supports_pair(&to, &from));
    assert!(!provider.supports_pair(&from, &Asset::parse("USDT@ethereum").unwrap()));
    let quote = provider
        .quote(from.clone(), to.clone(), amount)
        .await
        .unwrap();
    assert_eq!(quote.output.value, "1736.2695");
    assert_eq!(quote.output.asset, to);
    assert_eq!(quote.fees[0].value, "1");
    assert_eq!(quote.fees[0].asset, to);
    assert_eq!(quote.quote_id.as_deref(), Some("15390"));
    assert_eq!(quote.path, [from, to]);
    assert!(quote.expires_at.unwrap() > chrono::Utc::now());
    assert_eq!(state.catalog_calls.load(Ordering::Relaxed), 1);
    assert_eq!(state.quote_calls.load(Ordering::Relaxed), 1);
    let queries = state.queries.lock().unwrap();
    assert_eq!(queries[0].get("currency_from").unwrap(), "trx");
    assert_eq!(queries[0].get("currency_to").unwrap(), "usdt_trx");
    assert_eq!(queries[0].get("amount").unwrap(), "5000");
    assert_eq!(queries[0].len(), 3);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_is_shared_and_failed_catalogs_do_not_become_cached_answers() {
    let (provider, state, server) = mock().await;
    state.catalog_status.store(401, Ordering::Relaxed);
    assert!(provider
        .catalog()
        .await
        .err()
        .unwrap()
        .to_string()
        .contains("authentication"));
    assert!(!provider.supports_pair(&direction().0, &direction().1));
    state.catalog_status.store(200, Ordering::Relaxed);
    let (a, b) = tokio::join!(provider.supported_assets(), provider.supported_assets());
    assert_eq!(a.len(), 2);
    assert_eq!(b.len(), 2);
    assert_eq!(state.catalog_calls.load(Ordering::Relaxed), 2);
    provider
        .catalog
        .write()
        .unwrap()
        .as_mut()
        .unwrap()
        .loaded_at = Instant::now() - CATALOG_TTL;
    assert_eq!(provider.supported_assets().await.len(), 2);
    assert_eq!(state.catalog_calls.load(Ordering::Relaxed), 3);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn respects_input_limits_and_classifies_unavailable_amounts_without_hiding_failures() {
    let (provider, state, server) = mock().await;
    let (from, to, amount) = direction();
    for value in ["0", "0.5", "10001"] {
        let error = provider
            .quote(
                from.clone(),
                to.clone(),
                Amount::new(value, from.clone()).unwrap(),
            )
            .await
            .unwrap_err();
        assert!(error.downcast_ref::<QuoteUnavailable>().is_some());
    }
    assert_eq!(state.quote_calls.load(Ordering::Relaxed), 0);
    state.quote_status.store(404, Ordering::Relaxed);
    assert!(provider
        .quote(from.clone(), to.clone(), amount.clone())
        .await
        .unwrap_err()
        .downcast_ref::<QuoteUnavailable>()
        .is_some());
    for status in [401, 429, 500] {
        state.quote_status.store(status, Ordering::Relaxed);
        let error = provider
            .quote(from.clone(), to.clone(), amount.clone())
            .await
            .unwrap_err();
        assert!(error.downcast_ref::<QuoteUnavailable>().is_none());
        assert!(!error.to_string().contains("test-token"));
    }
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_wrong_direction_amount_sentinels_and_non_positive_payouts() {
    let (provider, state, server) = mock().await;
    for body in [
        RATE.replace("\"amount\":5000", "\"amount\":5001"),
        RATE.replace(
            "\"currency_to\":\"usdt_trx\"",
            "\"currency_to\":\"usdt_eth\"",
        ),
        RATE.replace("\"swap_rate_id\":15390", "\"swap_rate_id\":-1"),
        RATE.replace("\"network_fee\":1", "\"network_fee\":1737.2695"),
        RATE.replace("\"network_fee\":1", "\"network_fee\":2000"),
        RATE.replace("\"result_amount\":1737.2695", "\"result_amount\":-1"),
        RATE.replace("\"status\":true", "\"status\":false"),
        "not json".into(),
    ] {
        *state.rate.lock().unwrap() = body;
        let (from, to, amount) = direction();
        assert!(provider.quote(from, to, amount).await.is_err());
    }
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_office_quotes_are_composed_into_public_routes() {
    let (provider, _, server) = mock().await;
    let service = crate::p2p::P2pSearchService::with_sources(Vec::new(), Duration::from_secs(2))
        .with_route_providers(vec![Arc::new(provider)]);
    let request = serde_json::from_str(
        r#"{
        "source_fiat":"TRX", "target_fiat":"USDT", "source_amount":5000,
        "source_network":"tron", "target_network":"tron", "sources":"crypto-office",
        "exchange_mode":"exchanger", "limit":5
    }"#,
    )
    .unwrap();
    let response = service.search_routes(request).await.unwrap();
    assert_eq!(response.routes.len(), 1);
    let route = &response.routes[0];
    assert_eq!(route.route_provider.as_deref(), Some("crypto-office"));
    assert_eq!(
        Decimal::parse(&route.target_amount).unwrap(),
        Decimal::parse("1736.2695").unwrap()
    );
    assert_eq!(route.route_path, ["TRX@tron", "USDT@tron"]);
    assert!(route.execution.is_none());
    assert_eq!(route.provider_quote_id.as_deref(), Some("15390"));
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quote_subtraction_preserves_eighteen_decimal_places() {
    let (provider, state, server) = mock().await;
    *state.rate.lock().unwrap() = RATE
        .replace(
            "\"result_amount\":1737.2695",
            "\"result_amount\":1737.269500000000000001",
        )
        .replace("\"network_fee\":1", "\"network_fee\":1e-18");
    let (from, to, amount) = direction();
    let quote = provider.quote(from, to, amount).await.unwrap();
    assert_eq!(quote.output.value, "1737.2695");
    assert_eq!(quote.fees[0].value, "0.000000000000000001");
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_limits_are_checked_against_the_live_quote() {
    let (provider, state, server) = mock().await;
    for response in [
        PAIRS.replace("\"min_to\":0.1", "\"min_to\":2000"),
        PAIRS.replace("\"max_to\":null", "\"max_to\":1500"),
    ] {
        *state.pair_response.lock().unwrap() = response;
        *provider.catalog.write().unwrap() = None;
        let (from, to, amount) = direction();
        let error = provider.quote(from, to, amount).await.unwrap_err();
        assert!(error.downcast_ref::<QuoteUnavailable>().is_some());
        assert!(error.to_string().contains("output amount"));
    }
    server.abort();
}

#[test]
fn signing_matches_official_php_libsodium_fixture() {
    let key: [u8; 32] = Blake2b::<U32>::digest(b"fixture-secret-key").into();
    let nonce: [u8; 24] = std::array::from_fn(|index| index as u8);
    let header =
        signed_header("PUBLIC-FIXTURE", &key, "fixture phrase", 1700000000, &nonce).unwrap();
    assert!(header.is_sensitive());
    assert_eq!(header.to_str().unwrap(), "External PUBLIC-FIXTURE|AAECAwQFBgcICQoLDA0ODxAREhMUFRYXrqrjfF0F0wFCsiWzGqEu5DbTG8Q/4wEP/A==");
}

#[test]
fn key_configuration_requires_both_keys_and_prefers_keys_over_static_tokens() {
    assert!(Authorization::configured(None, None, None)
        .unwrap()
        .is_none());
    assert!(Authorization::configured(Some("public"), None, Some("token")).is_err());
    assert!(Authorization::configured(None, Some("secret"), None).is_err());
    assert!(Authorization::configured(Some("bad|public"), Some("secret"), None).is_err());
    assert!(matches!(
        Authorization::configured(Some("public"), Some("secret"), Some("bad\ntoken")).unwrap(),
        Some(Authorization::Keys { .. })
    ));
    assert!(matches!(
        Authorization::configured(None, None, Some("token")).unwrap(),
        Some(Authorization::Token(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key_auth_fetches_a_phrase_and_signs_fresh_tokens_for_each_request() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
    let captured = Arc::clone(&seen);
    let app = axum::Router::new()
        .route("/api/get-phrase", get(|headers: HeaderMap| async move {
            assert!(headers.get(AUTHORIZATION).is_none());
            axum::Json(serde_json::json!({"status":true,"data":{"ip":"127.0.0.1","phrase":"mock IP-bound phrase"}}))
        }))
        .route("/api/v1/swap/list-available-pairs", get(move |headers: HeaderMap| {
            let captured = Arc::clone(&captured);
            async move {
                let encoded = headers.get(AUTHORIZATION).unwrap().to_str().unwrap()
                    .strip_prefix("External public|").unwrap();
                let payload = STANDARD.decode(encoded).unwrap();
                captured.lock().unwrap().push(payload[..24].to_vec());
                let key: [u8; 32] = Blake2b::<U32>::digest(b"secret").into();
                let mut plaintext = payload[24..].to_vec();
                XSalsa20::new((&key).into(), payload[..24].into()).apply_keystream(&mut plaintext);
                let plaintext = String::from_utf8(plaintext).unwrap();
                let (phrase, timestamp) = plaintext.rsplit_once('|').unwrap();
                assert_eq!(phrase, "mock IP-bound phrase");
                assert!((chrono::Utc::now().timestamp() - timestamp.parse::<i64>().unwrap()).abs() < 5);
                ([("content-type", "application/json")], PAIRS)
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider = CryptoOfficeRouteProvider::with_authorization(
        &format!("http://{}/api/v1/", listener.local_addr().unwrap()),
        Authorization::configured(Some("public"), Some("secret"), None)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for _ in 0..2 {
        let _: HashMap<String, Pair> = provider
            .get("swap/list-available-pairs", &[])
            .await
            .unwrap();
    }
    let nonces = seen.lock().unwrap();
    assert_eq!(nonces.len(), 2);
    assert_ne!(nonces[0], nonces[1]);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phrase_failure_prevents_authenticated_requests_and_does_not_expose_response() {
    let app = axum::Router::new().route(
        "/api/get-phrase",
        get(|| async { (HttpStatus::FORBIDDEN, "private-phrase-never-log") }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider = CryptoOfficeRouteProvider::with_authorization(
        &format!("http://{}/api/v1/", listener.local_addr().unwrap()),
        Authorization::configured(Some("public"), Some("secret"), None)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let error = provider.catalog().await.err().unwrap().to_string();
    assert!(error.contains("403"));
    assert!(!error.contains("private-phrase"));
    server.abort();
}

use super::*;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

fn office() -> Office {
    serde_json::from_value(json!({"id":"batumi", "nameEn":"Batumi", "addressEn":"27 Gamsakhurdia St", "workingHoursEn":"Mon-Sat", "isActive":true, "comingSoon":false})).unwrap()
}

fn catalog_json() -> Value {
    json!({"detailed":[
        {"code":"GEL","symbol":"GEL","type":"FIAT"},
        {"code":"USD","symbol":"USD","type":"FIAT"},
        {"code":"USDT_TRC20","symbol":"USDT","type":"CRYPTO","network":"TRC20"},
        {"code":"USDC_ERC20","symbol":"USDC","type":"CRYPTO","network":"ERC20"},
        {"code":"USDT_ERC20","symbol":"USDT","type":"CRYPTO","network":"ERC20"},
        {"code":"FAKE","symbol":"FAKE","type":"CRYPTO","network":"TRC20"}
    ],"pairs":[
        {"base":"GEL","quote":"USDT_TRC20"}, {"base":"USDT_TRC20","quote":"GEL"},
        {"base":"GEL","quote":"USD"}, {"base":"USD","quote":"GEL"},
        {"base":"USDT_TRC20","quote":"USDC_ERC20"}, {"base":"USDC_ERC20","quote":"USDT_ERC20"}
    ]})
}

#[test]
fn catalog_networks_directions_and_cash_eligibility() {
    let provider = SolunexProvider::new(NetworkCatalog::test_default()).unwrap();
    let catalog = provider.parse_catalog(office(), &catalog_json()).unwrap();
    assert!(!catalog.codes.contains_key("FAKE"));
    let gel = Asset::parse("GEL").unwrap();
    assert!(provider.supports_payment_method(&gel, Some("Cash")));
    for method in [None, Some("TBC Bank"), Some("Bank Transfer")] {
        assert!(!provider.supports_payment_method(&gel, method));
    }
    assert!(!provider.allows_coefficient_quotes());
    *provider.catalog.write().unwrap() = Some(Catalog {
        loaded: Instant::now(),
        offices: vec![catalog],
    });
    assert!(provider.supports_pair(&gel, &Asset::parse("USDT@tron").unwrap()));
    assert!(provider.cached_catalog().is_some());
    assert!(!provider.supports_pair(&gel, &Asset::parse("USDT@ethereum").unwrap()));
    assert!(!provider.supports_pair(
        &Asset::parse("USD").unwrap(),
        &Asset::parse("USDT@tron").unwrap()
    ));
    provider.catalog.write().unwrap().as_mut().unwrap().loaded = Instant::now() - CATALOG_TTL;
    assert!(provider.cached_catalog().is_none());
}

#[test]
fn net_output_is_not_charged_twice_and_invalid_quotes_are_rejected() {
    let from = Asset::parse("GEL").unwrap();
    let to = Asset::parse("USDT@tron").unwrap();
    let amount = Amount::new("1000", from.clone()).unwrap();
    let raw = json!({"giveAmount":"1000", "receiveAmount":"365.80730693", "commission":"15", "error":null,
        "quoteExpiresAt":(Utc::now()+chrono::Duration::minutes(10)).to_rfc3339(), "quoteToken":"private-token"});
    let quote =
        SolunexProvider::parse_quote(&raw, &office(), from.clone(), to.clone(), amount.clone())
            .unwrap();
    assert_eq!(quote.output.value, "365.80730693");
    assert_eq!(quote.quote_id.as_deref(), Some("batumi"));
    assert!(quote.fees.is_empty());
    assert!(!format!("{quote:?}").contains("private-token"));
    for (key, value) in [
        ("error", json!("Minimum amount")),
        ("giveAmount", json!("999")),
        ("receiveAmount", json!("0")),
        ("receiveAmount", json!("NaN")),
        ("quoteExpiresAt", json!("2020-01-01T00:00:00Z")),
    ] {
        let mut invalid = raw.clone();
        invalid[key] = value;
        assert!(
            SolunexProvider::parse_quote(
                &invalid,
                &office(),
                from.clone(),
                to.clone(),
                amount.clone()
            )
            .is_err(),
            "{key}"
        );
    }
}

type Calls = Arc<Mutex<Vec<Value>>>;
async fn calculator(
    State(calls): State<Calls>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    assert_eq!(headers.get("origin").unwrap(), "https://solunex.ge");
    assert_eq!(headers.get("referer").unwrap(), HOME);
    let amount = body["giveAmount"].as_str().unwrap().parse::<f64>().unwrap();
    calls.lock().await.push(body.clone());
    let output = if body["giveCurrency"] == "GEL" && body["receiveCurrency"] == "USDT_TRC20" {
        amount / 2.5 - 15.0
    } else if body["giveCurrency"] == "USDT_TRC20" && body["receiveCurrency"] == "GEL" {
        (amount - 15.0) * 2.5
    } else if body["giveCurrency"] == "GEL" {
        amount / 2.5
    } else {
        amount - 15.0
    };
    Json(
        json!({"giveAmount":amount.to_string(), "receiveAmount":output.to_string(), "error":null,
        "quoteExpiresAt":(Utc::now()+chrono::Duration::minutes(10)).to_rfc3339(), "commission":"15"}),
    )
}

async fn fixture() -> (SolunexProvider, Calls, tokio::task::JoinHandle<()>) {
    let calls = Calls::default();
    let offices = json!({"offices":[
        {"id":"batumi","nameEn":"Batumi","addressEn":"27 Gamsakhurdia St","isActive":true,"comingSoon":false},
        {"id":"unopened","nameEn":"Tbilisi","addressEn":"37 Chavchavadze Ave","isActive":true,"comingSoon":true}
    ]});
    let router = Router::new()
        .route("/api/offices", get(move || async { Json(offices) }))
        .route(
            "/api/currencies",
            get(
                |axum::extract::Query(q): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| async move {
                    assert_eq!(q["officeId"], "batumi");
                    Json(catalog_json())
                },
            ),
        )
        .route("/api/rates/calculate", post(calculator))
        .with_state(calls.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut provider = SolunexProvider::new(NetworkCatalog::test_default()).unwrap();
    provider.base_url = format!("http://{address}");
    (provider, calls, task)
}

#[tokio::test]
async fn routes_use_exact_amounts_cash_only_and_exclude_unopened_offices() {
    let (provider, calls, server) = fixture().await;
    let service = crate::p2p::P2pSearchService::with_sources(vec![], Duration::from_secs(2))
        .with_route_providers(vec![Arc::new(provider)]);
    for (amount, expected) in [(1000, 385.0), (2000, 785.0)] {
        let query = serde_json::from_value(json!({"source_fiat":"GEL","target_fiat":"USDT","source_amount":amount,
            "target_network":"tron","source_payment_method":"Cash","sources":"solunex","exchange_mode":"exchanger"})).unwrap();
        let result = service.search_routes(query).await.unwrap();
        let route = result
            .routes
            .iter()
            .find(|r| r.route_provider.as_deref() == Some("solunex"))
            .expect("live Solunex route");
        assert_eq!(route.target_amount.parse::<f64>().unwrap(), expected);
        assert!(route.warnings.iter().any(|s| s.contains("Batumi")));
        assert!(route.quote_expires_at.is_some());
        assert!(route.execution.is_none());
    }
    let before = calls.lock().await.len();
    let bank_query = serde_json::from_value(json!({"source_fiat":"GEL","target_fiat":"USDT","source_amount":1000,
        "target_network":"tron","source_payment_method":"TBC Bank","sources":"solunex","exchange_mode":"exchanger"})).unwrap();
    assert!(service
        .search_routes(bank_query)
        .await
        .unwrap()
        .routes
        .is_empty());
    assert_eq!(
        calls.lock().await.len(),
        before,
        "bank selection must not request cash quotes"
    );
    for (source, target, network_field, method_field, amount, expected, kind) in [
        (
            "USDT",
            "GEL",
            "source_network",
            "target_payment_method",
            400,
            962.5,
            "crypto_to_fiat",
        ),
        (
            "GEL",
            "USD",
            "target_network",
            "source_payment_method",
            1000,
            400.0,
            "fiat_to_fiat",
        ),
    ] {
        let mut query = json!({"source_fiat":source, "target_fiat":target, "source_amount":amount, "sources":"solunex", "exchange_mode":"exchanger"});
        query[method_field] = json!("Cash");
        if source == "USDT" {
            query[network_field] = json!("tron");
        } else {
            query["target_payment_method"] = json!("Cash");
        }
        let result = service
            .search_routes(serde_json::from_value(query).unwrap())
            .await
            .unwrap();
        assert!(result
            .routes
            .iter()
            .any(|r| r.route_kind == kind && r.target_amount.parse::<f64>().unwrap() == expected));
    }
    let small = serde_json::from_value(json!({"source_fiat":"GEL","target_fiat":"USDT","source_amount":1,
        "target_network":"tron","source_payment_method":"Cash","sources":"solunex","exchange_mode":"exchanger"})).unwrap();
    assert!(service
        .search_routes(small)
        .await
        .unwrap()
        .routes
        .is_empty());
    assert!(calls.lock().await.iter().all(|q| q["officeId"] == "batumi"));
    server.abort();
}

#[tokio::test]
async fn composed_crypto_route_requotes_the_actual_intermediate_amount() {
    let (provider, calls, server) = fixture().await;
    let service = crate::p2p::P2pSearchService::with_sources(vec![], Duration::from_secs(2))
        .with_route_providers(vec![Arc::new(provider)]);
    let query = serde_json::from_value(json!({"source_fiat":"USDT","target_fiat":"USDT","source_amount":100,
        "source_network":"tron","target_network":"ethereum","allow_cross_venue":true,"sources":"solunex","exchange_mode":"exchanger"})).unwrap();
    let result = service.search_routes(query).await.unwrap();
    assert!(result
        .routes
        .iter()
        .any(|r| r.target_amount.parse::<f64>().unwrap() == 70.0));
    assert!(calls
        .lock()
        .await
        .iter()
        .any(|q| q["giveCurrency"] == "USDC_ERC20" && q["giveAmount"] == "85"));
    server.abort();
}

#[tokio::test]
#[ignore = "read-only live Solunex calculator check"]
async fn live_catalog_and_quotes() {
    let provider = SolunexProvider::new(NetworkCatalog::test_default()).unwrap();
    assert!(provider
        .supported_assets()
        .await
        .contains(&Asset::parse("GEL").unwrap()));
    let from = Asset::parse("GEL").unwrap();
    let quote = provider
        .quote(
            from.clone(),
            Asset::parse("USDT@tron").unwrap(),
            Amount::new("1000", from).unwrap(),
        )
        .await
        .unwrap();
    assert!(positive(&quote.output.value).is_some());
    assert!(quote.expires_at.unwrap() > Utc::now());
}

#[tokio::test]
async fn catalog_failure_backs_off_before_another_request() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let app = axum::Router::new().route("/api/offices", axum::routing::get(move || {
        let count = count.clone();
        async move { count.fetch_add(1, Ordering::SeqCst); axum::http::StatusCode::SERVICE_UNAVAILABLE }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
    let mut provider = SolunexProvider::new(NetworkCatalog::test_default()).unwrap();
    provider.base_url = format!("http://{addr}");
    assert!(provider.supported_assets().await.is_empty());
    assert!(provider.supported_assets().await.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.abort();
}

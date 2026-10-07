use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::p2p::{P2pRouteSearchQuery, P2pSearchService};
use crate::route_engine::{
    Amount, Asset, CowRouteProvider, NearIntentsProvider, PublicRouteProvider,
    SymbiosisRouteProvider,
};
use crate::route_execution::RouteExecutionService;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

const USDT: &str = "0x0000000000000000000000000000000000000010";
const USDC: &str = "0x0000000000000000000000000000000000000020";
const DYNAMIC: &str = "0x0000000000000000000000000000000000000040";
const ONLY_COW: &str = "0x0000000000000000000000000000000000000060";
const BSC_USDT: &str = "0x0000000000000000000000000000000000000030";
const OWNER: &str = "0x0000000000000000000000000000000000000001";

#[derive(Clone, Default)]
struct Requests {
    bodies: Arc<Mutex<Vec<Value>>>,
    reject_usdt_bridge: bool,
    reject_all_bridges: bool,
}

struct Fixture {
    cow: CowRouteProvider,
    requests: Requests,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture(reject_usdt_bridge: bool, reject_all_bridges: bool) -> Fixture {
    let requests = Requests {
        reject_usdt_bridge,
        reject_all_bridges,
        ..Requests::default()
    };
    let app = Router::new()
        .route("/api/v1/quote", post(|State(requests): State<Requests>, Json(body): Json<Value>| async move {
            requests.bodies.lock().unwrap().push(body.clone());
            let buy_amount = body["sellAmountBeforeFee"].as_str().unwrap().parse::<u128>().unwrap() - 1_250_000;
            Json(json!({"quote": {"buyAmount": buy_amount.to_string(), "feeAmount": "1250000"}}))
        }))
        .route("/tokens.json", get(|| async { Json(json!({"tokens":[
            {"chainId":1, "symbol":"USDT", "address":USDT, "decimals":6},
            {"chainId":1, "symbol":"USDC", "address":USDC, "decimals":6},
            {"chainId":1, "symbol":"UNLISTED", "address":DYNAMIC, "decimals":8},
            {"chainId":1, "symbol":"ONLYCOW", "address":ONLY_COW, "decimals":8},
            {"chainId":56, "symbol":"USDT", "address":BSC_USDT, "decimals":18}
        ]})) }))
        .route("/v0/tokens", get(|| async { Json(json!([
            {"assetId":"usdt-eth", "blockchain":"eth", "symbol":"USDT", "decimals":6, "contractAddress":USDT},
            {"assetId":"usdc-eth", "blockchain":"eth", "symbol":"USDC", "decimals":6, "contractAddress":USDC},
            {"assetId":"sol-native", "blockchain":"sol", "symbol":"SOL", "decimals":9},
            {"assetId":"usdc-sol", "blockchain":"sol", "symbol":"USDC", "decimals":6, "contractAddress":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"},
            {"assetId":"usdt-tron", "blockchain":"tron", "symbol":"USDT", "decimals":6},
            {"assetId":"unlisted-eth", "blockchain":"eth", "symbol":"UNLISTED", "decimals":8, "contractAddress":DYNAMIC},
            {"assetId":"usdt-bsc", "blockchain":"bsc", "symbol":"USDT", "decimals":18, "contractAddress":BSC_USDT},
            {"assetId":"usdc-base", "blockchain":"base", "symbol":"USDC", "decimals":6, "contractAddress":USDC},
            {"assetId":"newcoin-base", "blockchain":"base", "symbol":"NEWCOIN", "decimals":8, "contractAddress":DYNAMIC},
            {"assetId":"nep141:btc.omft.near", "blockchain":"btc", "symbol":"BTC", "decimals":8},
            {"assetId":"1cs_v1:btc:native:coin", "blockchain":"btc", "symbol":"BTC(OMNI)", "decimals":8, "contractAddress":"coin"}
        ])) }))
        .route("/v0/quote", post(|State(requests): State<Requests>, Json(body): Json<Value>| async move {
            requests.bodies.lock().unwrap().push(body.clone());
            if requests.reject_all_bridges || (requests.reject_usdt_bridge && body["originAsset"] == "usdt-eth") {
                return (StatusCode::BAD_REQUEST, Json(json!({"message":"No liquidity available"})));
            }
            assert_eq!(body["dry"], true);
            let output = match body["destinationAsset"].as_str().unwrap() {
                "sol-native" => "25000000000",
                "usdc-base" => "995000000",
                "newcoin-base" => "12345678",
                "1cs_v1:btc:native:coin" => "2000000",
                other => panic!("unexpected bridge destination {other}"),
            };
            (StatusCode::OK, Json(json!({"quote": {"amountOut":output, "networkFee":"100000", "deadline":body["deadline"]}})))
        }))
        .with_state(requests.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let bridge = NearIntentsProvider::new(format!("http://{address}"), None)
        .unwrap()
        .with_quote_addresses("So11111111111111111111111111111111111111112", OWNER)
        .unwrap()
        .with_quote_network_addresses(
            &[
                "solana=So11111111111111111111111111111111111111112".into(),
                "bitcoin=bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".into(),
            ],
            &["solana=So11111111111111111111111111111111111111112".into()],
        )
        .unwrap();
    let cow = CowRouteProvider::from_config(
        &[
            format!("ethereum=http://{address}"),
            format!("bnb-smart-chain=http://{address}"),
            format!("solana=http://{address}"),
        ],
        &[],
        Some(OWNER),
    )
    .unwrap()
    .unwrap()
    .with_token_catalog(format!("http://{address}/tokens.json"))
    .with_bridge(bridge);
    Fixture {
        cow,
        requests,
        server,
    }
}

fn asset(value: &str) -> Asset {
    Asset::parse(value).unwrap()
}

#[tokio::test]
async fn cow_selected_alone_finds_usdt_erc20_to_native_sol() {
    let fixture = fixture(false, false).await;
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(5))
        .with_route_providers(vec![Arc::new(fixture.cow.clone())]);
    let query: P2pRouteSearchQuery = serde_json::from_value(json!({
        "source_fiat":"USDT", "target_fiat":"SOL", "source_amount":1000,
        "source_network":"ethereum", "target_network":"solana", "sources":"cow-swap"
    }))
    .unwrap();
    let response = service.search_routes(query).await.unwrap();
    let route = response
        .routes
        .first()
        .expect("CoW must discover the Solana route");
    assert_eq!(route.route_provider.as_deref(), Some("cow-swap"));
    assert_eq!(route.target_amount, "25");
    assert_eq!(route.route_path, ["USDT@ethereum", "SOL@solana"]);
    assert_eq!(route.route_fees[0].amount, "1.25");
    assert_eq!(route.route_fees[1].amount, "0.1");
    assert_eq!(route.route_provider_url.as_deref(), Some(format!(
        "https://swap.cow.fi/#/1/swap/{USDT}/11111111111111111111111111111111?targetChainId=1000000001"
    ).as_str()));
    assert!(route.quote_expires_at.is_some());
    assert!(route.execution.is_none());
    let manager =
        deadpool_postgres::Manager::new(tokio_postgres::Config::new(), tokio_postgres::NoTls);
    let executions = RouteExecutionService::new(
        deadpool_postgres::Pool::builder(manager).build().unwrap(),
        "test-signing-key",
        true,
        NearIntentsProvider::new("http://127.0.0.1", None).unwrap(),
        Some(fixture.cow.clone()),
        SymbiosisRouteProvider::new("http://127.0.0.1", None, OWNER, 100).unwrap(),
    );
    let mut executable_route = route.clone();
    executions.attach_descriptor(&mut executable_route);
    assert!(
        executable_route.execution.is_none(),
        "cross-chain CoW must open in CoW Swap"
    );
    executable_route.route_path = vec!["USDT@ethereum".into(), "USDC@ethereum".into()];
    executions.attach_descriptor(&mut executable_route);
    assert!(
        executable_route.execution.is_some(),
        "same-chain CoW execution must remain available"
    );
    let requests = fixture.requests.bodies.lock().unwrap();
    assert_eq!(requests[0]["sellAmountBeforeFee"], "1000000000");
    assert_eq!(requests[1]["amount"], "998750000");
}

#[tokio::test]
async fn bridge_falls_back_to_usdc_and_preserves_both_fee_assets() {
    let fixture = fixture(true, false).await;
    let from = asset("USDT@ethereum");
    let quote = fixture
        .cow
        .quote(
            from.clone(),
            asset("SOL@solana"),
            Amount::new("1000", from).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        quote.path,
        [
            asset("USDT@ethereum"),
            asset("USDC@ethereum"),
            asset("SOL@solana")
        ]
    );
    assert_eq!(quote.fees[0].asset, asset("USDT@ethereum"));
    assert_eq!(quote.fees[1].asset, asset("USDC@ethereum"));
}

#[tokio::test]
async fn capabilities_exclude_networks_without_cow_endpoints_and_unrelated_bridges() {
    let fixture = fixture(false, false).await;
    let assets = fixture.cow.supported_assets().await;
    assert!(assets.contains(&asset("SOL@solana")));
    assert!(assets.contains(&asset("USDT@ethereum")));
    assert!(assets.contains(&asset("USDC@base")));
    assert!(!fixture
        .cow
        .supports_pair(&asset("USDC@base"), &asset("SOL@solana")));
    assert!(fixture
        .cow
        .supports_pair(&asset("USDT@ethereum"), &asset("USDC@base")));
    assert!(!assets.contains(&asset("USDT@tron")));
}

#[tokio::test]
async fn same_chain_quotes_accept_calculated_amounts_at_token_precision() {
    let fixture = fixture(false, false).await;
    let from = asset("USDT@ethereum");
    let quote = fixture
        .cow
        .quote(
            from.clone(),
            asset("USDC@ethereum"),
            Amount::new("100.123456789", from).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(quote.input.value, "100.123456");
    assert_eq!(quote.output.value, "98.873456");
    assert_eq!(fixture.requests.bodies.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn unavailable_bridge_produces_no_quote() {
    let fixture = fixture(false, true).await;
    let from = asset("USDT@ethereum");
    assert!(fixture
        .cow
        .quote(
            from.clone(),
            asset("SOL@solana"),
            Amount::new("1000", from).unwrap()
        )
        .await
        .is_err());
}

#[tokio::test]
async fn discovers_cross_chain_pairs_and_unknown_tokens_without_configuration_entries() {
    let fixture = fixture(false, false).await;
    for (from, to, expected, destination_id) in [
        ("USDT@ethereum", "SOL@solana", "25", "1000000001"),
        ("USDT@ethereum", "USDC@base", "995", "8453"),
        ("USDC@ethereum", "BTC@bitcoin", "0.02", "1000000000"),
        ("UNLISTED@ethereum", "NEWCOIN@base", "0.12345678", "8453"),
        ("ONLYCOW@ethereum", "NEWCOIN@base", "0.12345678", "8453"),
        ("USDT@bnb-smart-chain", "SOL@solana", "25", "1000000001"),
    ] {
        let from = asset(from);
        let to = asset(to);
        let quote = fixture
            .cow
            .quote(
                from.clone(),
                to.clone(),
                Amount::new("1000", from.clone()).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(quote.output.value, expected, "{from} -> {to}");
        assert_eq!(quote.path.first(), Some(&from));
        assert_eq!(quote.path.last(), Some(&to));
        assert!(quote
            .source_url
            .unwrap()
            .ends_with(&format!("targetChainId={destination_id}")));
    }
    let requests = fixture.requests.bodies.lock().unwrap();
    let bsc = requests
        .iter()
        .find(|body| body["sellToken"] == BSC_USDT)
        .unwrap();
    assert_eq!(
        bsc["sellAmountBeforeFee"], "1000000000000000000000",
        "BSC USDT must use catalog precision, not symbol-inferred six decimals"
    );
    assert!(requests
        .iter()
        .any(|body| body["originAsset"] == "unlisted-eth"));
    assert!(!requests
        .iter()
        .any(|body| body["destinationAsset"] == "nep141:btc.omft.near"));
}

#[tokio::test]
async fn search_discovers_unknown_symbols_from_runtime_capabilities() {
    let fixture = fixture(false, false).await;
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(5))
        .with_route_providers(vec![Arc::new(fixture.cow.clone())]);
    let query = serde_json::from_value(json!({
        "source_fiat":"ONLYCOW", "target_fiat":"NEWCOIN", "source_amount":1000,
        "source_network":"ethereum", "target_network":"base", "sources":"cow-swap"
    }))
    .unwrap();
    let response = service.search_routes(query).await.unwrap();
    assert_eq!(response.routes.len(), 1);
    assert_eq!(response.routes[0].target_amount, "0.12345678");
}

#[tokio::test]
async fn native_solana_quotes_use_the_wrapped_mint_for_pricing() {
    let fixture = fixture(false, false).await;
    for (from, to) in [("SOL@solana", "USDC@solana"), ("USDC@solana", "SOL@solana")] {
        let from = asset(from);
        fixture
            .cow
            .quote(from.clone(), asset(to), Amount::new("100", from).unwrap())
            .await
            .unwrap();
    }
    let requests = fixture.requests.bodies.lock().unwrap();
    let wrapped_sol = "So11111111111111111111111111111111111111112";
    assert_eq!(requests[0]["sellToken"], wrapped_sol);
    assert_eq!(requests[1]["buyToken"], wrapped_sol);
    assert_eq!(requests[0]["signingScheme"], "eip712");
    assert_eq!(requests[0]["from"], wrapped_sol);
}

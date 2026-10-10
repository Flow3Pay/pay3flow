use super::{
    amount,
    booking::{QuoteRequest, RfqRequest},
    config::{Config, PROFILE},
    model::{Direction, Evidence, FeePolicy, Terms},
    protocol, Service,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

fn config() -> Config {
    Config { enabled:true, demo:true, origin:"https://app.example".into(), relay_origin:"https://relay.example".into(),matcher_actor:"https://relay.example/actors/matcher".into(),desk_actor:"https://relay.example/actors/desk".into(),desk_name:"Fixture desk".into(),ever_resource:"https://assets.example/native-ever-everscale-42".into(),usdt_resource:"https://assets.example/usdt-ethereum-1-dac17f958d2ee523a2206206994597c13d831ec7".into(),ever_wallet:format!("0:{}","a".repeat(64)),usdt_wallet:format!("0x{}","a".repeat(40)),ever_gas_reserve:"1".into(),eth_gas_reserve:"0.01".into(),min_usdt:"1".into(),max_usdt:"100000".into(),quote_seconds:120,payment_seconds:300,payout_seconds:600,staffed_utc_start:0,staffed_utc_end:24,support_owner:"support".into(),incident_owner:"engineer".into(),incident_backup:"backup".into(),refund_policy:"Return verified payment to the booked proved customer wallet after original payout is resolved.".into(),net_delivery_policy:"Verify exact balance increase; quoted receiving allowance covers recipient transaction cost.".into(),..Config::default() }
}
fn terms(direction: Direction) -> Terms {
    let cfg = config();
    let now = Utc::now();
    let (input, output) = if direction == Direction::Buy {
        ("100", "200")
    } else {
        ("200", "100")
    };
    let input_units =
        amount::units(input, if direction == Direction::Buy { 6 } else { 9 }).unwrap();
    let output_units =
        amount::units(output, if direction == Direction::Buy { 9 } else { 6 }).unwrap();
    let mut terms = Terms {
        demo: true,
        direction,
        input: input.into(),
        output: output.into(),
        input_units: input_units.to_string(),
        output_units: output_units.to_string(),
        customer_actor: "https://relay.example/actors/customer".into(),
        desk_actor: cfg.desk_actor.clone(),
        customer_ever: format!("0:{}", "b".repeat(64)),
        customer_usdt: format!("0x{}", "b".repeat(40)),
        desk_ever: cfg.ever_wallet.clone(),
        desk_usdt: cfg.usdt_wallet.clone(),
        quote_by: now + Duration::minutes(2),
        pay_by: now + Duration::minutes(7),
        payout_by: now + Duration::minutes(17),
        network_costs: "Fixture gas separate".into(),
        ever_receiving_cost_units: "1000000".into(),
        refund_policy: cfg.refund_policy.clone(),
        fee_policy: FeePolicy {
            version: "desk-0.25pct-floor5-v1".into(),
            rate: "0.0025".into(),
            floor: "5".into(),
            basis_units: "100000000".into(),
            fee_units: "5000000".into(),
            precision: 6,
        },
        proposal: Value::Null,
    };
    terms.proposal = protocol::proposal(
        "https://relay.example/actors/desk/quotes/fixture",
        &terms,
        &cfg,
    );
    terms
}
#[test]
fn negotiation_rejects_changed_terms_and_identities() {
    for direction in [Direction::Buy, Direction::Sell] {
        let terms = terms(direction);
        let offer = protocol::offer("https://relay.example/actors/customer/offers/test", &terms);
        let accepted = protocol::acceptance(
            "https://relay.example/actors/desk/decisions/test",
            &offer,
            &terms,
        );
        protocol::validate_offer(&offer, &terms, Utc::now()).unwrap();
        protocol::validate_acceptance(&accepted, &offer, &terms).unwrap();
        for pointer in [
            "/result/stipulates/resourceQuantity/hasNumericalValue",
            "/result/stipulates/satisfies",
            "/result/stipulates/resourceQuantity/hasUnit",
            "/actor",
            "/object",
            "/result/id",
            "/result/stipulates/id",
            "/result/type",
        ] {
            let mut bad = accepted.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("altered");
            assert!(
                protocol::validate_acceptance(&bad, &offer, &terms).is_err(),
                "{pointer}"
            );
        }
        let mut public = accepted;
        public["cc"] = json!(["https://www.w3.org/ns/activitystreams#Public"]);
        assert!(protocol::validate_acceptance(&public, &offer, &terms).is_err());
        assert!(
            protocol::validate_offer(&offer, &terms, terms.quote_by + Duration::seconds(1))
                .is_err()
        );
    }
}
#[test]
fn ethereum_receipt_requires_exact_token_sender_nonce_and_finality() {
    let leg = terms(Direction::Buy).leg("payment").unwrap();
    let hash = format!("0x{}", "c".repeat(64));
    let tx = json!({"hash":hash,"from":leg.sender,"to":super::config::USDT,"input":super::chain::ethereum::transfer_data(&leg).unwrap(),"nonce":"0x3"});
    let block = json!({"hash":"canonical","timestamp":"0x640","number":"0x64"});
    let receipt = json!({"status":"0x1","blockHash":"canonical","blockNumber":"0x64","transactionHash":hash,"logs":[{"address":super::config::USDT,"topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",format!("0x{:0>64}",leg.sender.trim_start_matches("0x")),format!("0x{:0>64}",leg.recipient.trim_start_matches("0x"))],"data":"0x5f5e100","logIndex":"0x0","removed":false}]});
    assert!(
        !super::chain::ethereum::receipt(&receipt, &tx, &block, 99, &leg, Some(3))
            .unwrap()
            .unwrap()
            .finalized
    );
    assert!(
        super::chain::ethereum::receipt(&receipt, &tx, &block, 100, &leg, Some(3))
            .unwrap()
            .unwrap()
            .finalized
    );
    for pointer in [
        "/status",
        "/blockHash",
        "/logs/0/address",
        "/logs/0/data",
        "/logs/0/topics/1",
        "/logs/0/topics/2",
    ] {
        let mut bad = receipt.clone();
        *bad.pointer_mut(pointer).unwrap() = json!("0x0");
        assert!(
            super::chain::ethereum::receipt(&bad, &tx, &block, 100, &leg, Some(3)).is_err(),
            "{pointer}"
        );
    }
    assert!(super::chain::ethereum::receipt(&receipt, &tx, &block, 100, &leg, Some(4)).is_err());
    let mut bad = receipt.clone();
    bad["logs"][0]["removed"] = json!(true);
    assert!(super::chain::ethereum::receipt(&bad, &tx, &block, 100, &leg, Some(3)).is_err());
    let mut bad = receipt;
    let duplicated = bad["logs"][0].clone();
    bad["logs"].as_array_mut().unwrap().push(duplicated);
    assert!(super::chain::ethereum::receipt(&bad, &tx, &block, 100, &leg, Some(3)).is_err());
}
#[test]
fn ever_receipt_traces_net_delivery_and_bounce() {
    let leg = terms(Direction::Buy).leg("payout").unwrap();
    let transaction = json!({"id":"receiving-tx","now":1600,"status":3,"account_addr":leg.recipient,"aborted":false,"balance_delta":leg.amount,"total_fees":"1000000","in_message":{"id":"receiving-message","src":leg.sender,"dst":leg.recipient,"value":leg.transfer_amount,"bounced":false},"compute":{"success":true,"exit_code":0},"action":null,"bounce":null});
    assert!(
        super::chain::everscale::receipt(&transaction, &leg)
            .unwrap()
            .successful
    );
    let mut bounced = transaction.clone();
    bounced["in_message"]["bounced"] = json!(true);
    assert!(
        !super::chain::everscale::receipt(&bounced, &leg)
            .unwrap()
            .successful
    );
    for pointer in [
        "/balance_delta",
        "/in_message/src",
        "/in_message/dst",
        "/in_message/value",
        "/account_addr",
    ] {
        let mut bad = transaction.clone();
        *bad.pointer_mut(pointer).unwrap() = json!("0");
        assert!(
            super::chain::everscale::receipt(&bad, &leg).is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn wallet_signature_cannot_be_reused_for_another_actor_or_nonce() {
    use k256::ecdsa::SigningKey;
    use sha3::{Digest, Keccak256};
    let key = SigningKey::from_bytes((&[7u8; 32]).into()).unwrap();
    let point = key.verifying_key().to_encoded_point(false);
    let hashed = Keccak256::digest(&point.as_bytes()[1..]);
    let address = format!("0x{}", super::wallet::hex(&hashed[12..]));
    let message = "Origin app, actor customer, nonce single-use";
    let prefixed = format!("\x19Ethereum Signed Message:\n{}{message}", message.len());
    let (signature, recovery) = key
        .sign_digest_recoverable(Keccak256::new_with_prefix(prefixed))
        .unwrap();
    let mut bytes = signature.to_bytes().to_vec();
    bytes.push(recovery.to_byte() + 27);
    let signature = super::wallet::hex(&bytes);
    super::wallet::verify_evm(message, &signature, &address).unwrap();
    assert!(super::wallet::verify_evm("other actor or nonce", &signature, &address).is_err());
}

async fn fixture() -> (Service, tokio::task::JoinHandle<()>, String) {
    let url = std::env::var("OTC_TEST_DATABASE_URL")
        .expect("set OTC_TEST_DATABASE_URL to an isolated PostgreSQL database");
    let root = crate::db::build_pool(&url).await.unwrap();
    let schema = format!("otc_test_{}", Uuid::new_v4().simple());
    root.get()
        .await
        .unwrap()
        .batch_execute(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let pool = crate::db::build_pool(&format!("{url}{sep}options=-csearch_path%3D{schema}"))
        .await
        .unwrap();
    pool.get()
        .await
        .unwrap()
        .batch_execute(include_str!("../../migrations/otc.sql"))
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut cfg = config();
    cfg.demo = false;
    cfg.relay_origin = base.clone();
    cfg.desk_actor = format!("{base}/actors/desk");
    cfg.matcher_actor = format!("{base}/actors/matcher");
    cfg.ethereum_rpc = vec![base.clone(), base.clone()];
    let proposals = json!({"orderedItems":[protocol::listing(&format!("{}/listings/buy",cfg.desk_actor),Direction::Buy,&cfg),protocol::listing(&format!("{}/listings/sell",cfg.desk_actor),Direction::Sell,&cfg)]});
    let app = axum::Router::new()
        .route(
            "/actors/desk/outbox",
            axum::routing::get(move || {
                let body = proposals.clone();
                async move {
                    (
                        [
                            ("x-relay-profile", PROFILE),
                            ("x-relay-signatures", "required"),
                        ],
                        axum::Json(body),
                    )
                }
            }),
        )
        .route(
            "/",
            axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":if body["method"]=="eth_chainId" {"0x1"}else{"0x3"}}))
            }),
        );
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let service = Service {
        pool,
        cfg: Arc::new(cfg),
        secrets: crate::core::crypto::SecretBox::new("fixture-key"),
        jwt: crate::core::jwt::Jwt::new("fixture-jwt"),
        http: reqwest::Client::new(),
    };
    let client = service.pool.get().await.unwrap();
    client.batch_execute("UPDATE otc_controls SET paused=FALSE; UPDATE otc_observers SET paused=FALSE,last_scan=now(),checkpoint='{\"block\":\"0x1\",\"timestamp\":\"2026-10-10T00:00:00Z\"}'; INSERT INTO otc_inventory(chain,balance,gas_reserve,observed_at) VALUES('ethereum',1000000000,0,now()),('everscale',10000000000000,1000000000,now())").await.unwrap();
    for (actor, ever, usdt) in [
        (
            service.cfg.desk_actor.clone(),
            service.cfg.ever_wallet.clone(),
            service.cfg.usdt_wallet.clone(),
        ),
        (
            format!("{base}/actors/customer"),
            format!("0:{}", "b".repeat(64)),
            format!("0x{}", "b".repeat(40)),
        ),
        (
            format!("{base}/actors/customer2"),
            format!("0:{}", "d".repeat(64)),
            format!("0x{}", "d".repeat(40)),
        ),
    ] {
        for (chain, address) in [("everscale", ever), ("ethereum", usdt)] {
            client
                .execute(
                    "INSERT INTO otc_wallets VALUES($1,$2,$3,now())",
                    &[&actor, &chain, &address],
                )
                .await
                .unwrap();
        }
    }
    (service, task, format!("{base}/actors/customer"))
}
async fn negotiated(
    service: &Service,
    actor: &str,
    direction: Direction,
    input: &str,
    output: &str,
) -> Uuid {
    let slug = if direction == Direction::Buy {
        "buy"
    } else {
        "sell"
    };
    let rfq = service
        .rfq(
            actor,
            &Uuid::new_v4().to_string(),
            RfqRequest {
                direction,
                input: input.into(),
                listing_id: format!("{}/listings/{slug}", service.cfg.desk_actor),
            },
        )
        .await
        .unwrap();
    let rfq_id = Uuid::parse_str(rfq["id"].as_str().unwrap()).unwrap();
    let client = service.pool.get().await.unwrap();
    let body: Value = client
        .query_one("SELECT body FROM otc_rfq WHERE id=$1", &[&rfq_id])
        .await
        .unwrap()
        .get(0);
    service
        .ingest(&service.cfg.desk_actor, &body, 1)
        .await
        .unwrap();
    let quote = service
        .quote(
            &service.cfg.desk_actor,
            QuoteRequest {
                rfq_id,
                output: output.into(),
                network_costs: "Fixture sender gas + 0.001 EVER receiving allowance".into(),
                ever_receiving_cost: "0.001".into(),
            },
        )
        .await
        .unwrap();
    let id = Uuid::parse_str(quote["trade"]["id"].as_str().unwrap()).unwrap();
    let trade = service.owned_trade(actor, id).await.unwrap();
    service
        .ingest(actor, &trade.terms.proposal, 2)
        .await
        .unwrap();
    service.apply(actor, id).await.unwrap();
    let offer = service.owned_trade(actor, id).await.unwrap().offer.unwrap();
    service
        .ingest(&service.cfg.desk_actor, &offer, 3)
        .await
        .unwrap();
    id
}
fn evidence(trade: &super::model::Trade, kind: &str, identity: &str) -> Evidence {
    Evidence {
        chain: trade.terms.leg(kind).unwrap().chain.clone(),
        identity: identity.into(),
        leg: trade.terms.leg(kind).unwrap(),
        included_at: Utc::now(),
        canonical: true,
        finalized: true,
        successful: true,
        raw: json!({"simulation":true}),
    }
}
async fn accept(service: &Service, actor: &str, id: Uuid) {
    service
        .decide(&service.cfg.desk_actor, id, true)
        .await
        .unwrap();
    let decision = service
        .owned_trade(actor, id)
        .await
        .unwrap()
        .decision
        .unwrap();
    service.ingest(actor, &decision, 4).await.unwrap();
}
async fn cleanup(service: Service, task: tokio::task::JoinHandle<()>) {
    task.abort();
    let schema: String = service
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT current_schema()", &[])
        .await
        .unwrap()
        .get(0);
    service
        .pool
        .get()
        .await
        .unwrap()
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires OTC_TEST_DATABASE_URL; run cargo test otc::tests -- --include-ignored"]
async fn database_settlement_recovery_and_access() {
    let (service, task, actor) = fixture().await;
    for (direction, input, output) in [
        (Direction::Buy, "100", "200"),
        (Direction::Sell, "200", "100"),
    ] {
        let id = negotiated(&service, &actor, direction, input, output).await;
        assert!(service
            .prepare(&actor, id, "before-acceptance", "payment")
            .await
            .is_err());
        accept(&service, &actor, id).await;
        let trade = service.owned_trade(&actor, id).await.unwrap();
        assert_eq!(trade.state, "accepted");
        assert!(service.details("outsider", id).await.is_err());
        assert!(service.decide(&actor, id, true).await.is_err());
        let payment = evidence(&trade, "payment", &format!("payment-{id}"));
        service.credit(id, "payment", &payment).await.unwrap();
        service.credit(id, "payment", &payment).await.unwrap();
        assert_eq!(
            service.owned_trade(&actor, id).await.unwrap().state,
            "funded"
        );
        let payout = evidence(&trade, "payout", &format!("payout-{id}"));
        let (a, b) = tokio::join!(
            service.credit(id, "payout", &payout),
            service.credit(id, "payout", &payout)
        );
        a.unwrap();
        b.unwrap();
        assert_eq!(
            service.owned_trade(&actor, id).await.unwrap().state,
            "completed"
        );
        let row = service
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*),sum(amount)::text FROM otc_ledger WHERE trade_id=$1",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, i64>(0), 1);
        assert_eq!(row.get::<_, String>(1), "5000000");
        assert!(service
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE otc_trades SET terms=jsonb_set(terms,'{output}','\"999\"') WHERE id=$1",
                &[&id]
            )
            .await
            .is_err());
    }
    let id = negotiated(&service, &actor, Direction::Buy, "100", "200").await;
    accept(&service, &actor, id).await;
    let mut live = service.clone();
    let mut cfg = (*live.cfg).clone();
    cfg.demo = false;
    live.cfg = Arc::new(cfg);
    let prepared = live
        .prepare(&actor, id, "persistent-handoff", "payment")
        .await
        .unwrap();
    assert_eq!(prepared["state"], "unknown");
    let restarted = live.clone();
    assert_eq!(
        restarted
            .prepare(&actor, id, "persistent-handoff", "payment")
            .await
            .unwrap()["id"],
        prepared["id"]
    );
    assert!(restarted
        .prepare(&actor, id, "another-send", "payment")
        .await
        .is_err());
    let trade = service.owned_trade(&actor, id).await.unwrap();
    let payment = evidence(&trade, "payment", &format!("payment-{id}"));
    service.credit(id, "payment", &payment).await.unwrap();
    // A lost desk payout response blocks a refund and a new payout.
    service.pool.get().await.unwrap().execute("INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,'payout','everscale','unknown',$3,'lost-response')",&[&Uuid::new_v4(),&id,&json!({"leg":trade.terms.leg("payout").unwrap()})]).await.unwrap();
    assert!(live
        .prepare(&service.cfg.desk_actor, id, "duplicate-payout", "payout")
        .await
        .is_err());
    assert!(live
        .prepare(&service.cfg.desk_actor, id, "unsafe-refund", "refund")
        .await
        .is_err());
    let dashboard = service.dashboard(&service.cfg.desk_actor).await.unwrap();
    assert_eq!(dashboard["economics"]["accrued_usdt_units"], "10000000");
    assert_eq!(dashboard["economics"]["collected_usdt_units"], "0");
    assert_eq!(dashboard["open_funded_cases"].as_array().unwrap().len(), 1);
    service
        .incident(
            "lost-payout",
            "P1",
            Some(id),
            "payout",
            json!({"simulation":true}),
        )
        .await
        .unwrap();
    assert!(service
        .incident_action(
            &service.cfg.desk_actor,
            "lost-payout",
            super::operations::IncidentAction {
                action: "close".into(),
                evidence: json!({"service":"restored"}),
                customer_update: None
            }
        )
        .await
        .is_err());
    cleanup(service, task).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn concurrent_reservation_and_unknown_booking_are_blocked() {
    let (service, task, actor) = fixture().await;
    let other = actor.replace("customer", "customer2");
    let a = negotiated(&service, &actor, Direction::Sell, "100", "800").await;
    assert!(service
        .rfq(
            &actor,
            "blocked",
            RfqRequest {
                direction: Direction::Buy,
                input: "100".into(),
                listing_id: format!("{}/listings/buy", service.cfg.desk_actor)
            }
        )
        .await
        .is_err());
    let b = negotiated(&service, &other, Direction::Sell, "100", "800").await;
    let (left, right) = tokio::join!(
        service.decide(&service.cfg.desk_actor, a, true),
        service.decide(&service.cfg.desk_actor, b, true)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let reserved: String = service
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT sum(amount)::text FROM otc_reservations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(reserved, "800000000");
    cleanup(service, task).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn late_payment_verified_refund_and_incident_disposition() {
    let (service, task, actor) = fixture().await;
    let id = negotiated(&service, &actor, Direction::Sell, "200", "100").await;
    accept(&service, &actor, id).await;
    let trade = service.owned_trade(&actor, id).await.unwrap();
    let mut late = evidence(&trade, "payment", "late-payment");
    late.included_at = trade.terms.pay_by + Duration::seconds(1);
    service.credit(id, "payment", &late).await.unwrap();
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().state,
        "review"
    );
    let key = format!("late-payment:{id}");
    service
        .incident_action(
            &service.cfg.desk_actor,
            &key,
            super::operations::IncidentAction {
                action: "service_restored".into(),
                evidence: json!({"regression_test":"late payment remains review"}),
                customer_update: Some(
                    "Your payment is in review; the desk is arranging the verified refund.".into(),
                ),
            },
        )
        .await
        .unwrap();
    assert!(service
        .incident_action(
            &service.cfg.desk_actor,
            &key,
            super::operations::IncidentAction {
                action: "close".into(),
                evidence: json!({"health":"restored"}),
                customer_update: None
            }
        )
        .await
        .is_err());
    let refund = evidence(&trade, "refund", "refund-proof");
    service.credit(id, "refund", &refund).await.unwrap();
    service.credit(id, "refund", &refund).await.unwrap();
    let count: i64 = service
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM otc_ledger WHERE trade_id=$1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    for action in ["acknowledge", "update", "financially_resolved", "close"] {
        service.incident_action(&service.cfg.desk_actor,&key,super::operations::IncidentAction{action:action.into(),evidence:json!({"refund":"refund-proof","root_cause":"fixture late payment","preventive_test":"late_payment_verified_refund_and_incident_disposition","follow_up_owner":"desk","follow_up_date":"2026-10-12"}),customer_update:None}).await.unwrap();
    }
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().state,
        "refunded"
    );
    cleanup(service, task).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn monitoring_zero_traffic_stale_data_and_fee_accounting() {
    let (service, task, actor) = fixture().await;
    let dashboard = service.dashboard(&service.cfg.desk_actor).await.unwrap();
    assert!(dashboard["quote_coverage"]["rate"].is_null());
    assert!(dashboard["on_time_payout"]["rate"].is_null());
    assert!(dashboard["reporting"]["quote_response_p95_seconds"].is_null());
    assert_eq!(
        dashboard["reporting"]["agreed_usdt_leg_notional_units"],
        "0"
    );
    let metrics = service.metrics(&service.cfg.desk_actor).await.unwrap();
    assert!(metrics.contains("pay3flow_otc_quote_response_seconds_count"));
    assert!(!metrics.contains(&actor));
    assert!(!metrics.contains("0xaaaa"));
    assert!(service.metrics(&actor).await.is_err());
    let id = negotiated(&service, &actor, Direction::Buy, "100", "200").await;
    accept(&service, &actor, id).await;
    let trade = service.owned_trade(&actor, id).await.unwrap();
    service
        .credit(id, "payment", &evidence(&trade, "payment", "paid"))
        .await
        .unwrap();
    service
        .credit(id, "payout", &evidence(&trade, "payout", "delivered"))
        .await
        .unwrap();
    let request = || super::accounting::EntryRequest {
        trade_id: id,
        kind: "fee_collected".into(),
        amount: "5".into(),
        reference: "fixture-invoice-bank-receipt".into(),
        idempotency_key: "collect-once".into(),
    };
    let first = service
        .accounting_entry(&service.cfg.desk_actor, request())
        .await
        .unwrap();
    assert_eq!(
        first,
        service
            .accounting_entry(&service.cfg.desk_actor, request())
            .await
            .unwrap()
    );
    let mut duplicate = request();
    duplicate.idempotency_key = "second-charge".into();
    assert!(service
        .accounting_entry(&service.cfg.desk_actor, duplicate)
        .await
        .is_err());
    let dashboard = service.dashboard(&service.cfg.desk_actor).await.unwrap();
    assert_eq!(dashboard["economics"]["collected_usdt_units"], "5000000");
    assert_eq!(dashboard["economics"]["unpaid_usdt_units"], "0");
    service
        .pool
        .get()
        .await
        .unwrap()
        .batch_execute("UPDATE otc_observers SET last_scan=NULL,paused=TRUE")
        .await
        .unwrap();
    assert!(service.ready().await.is_err());
    service.alert_tick().await.unwrap();
    let severity: String = service
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT severity FROM otc_incidents WHERE incident_key='observer:ethereum'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(severity, "P2");
    assert!(service
        .session(
            &crate::core::jwt::Jwt::new("legacy")
                .sign("demo-email")
                .unwrap()
        )
        .await
        .is_err());
    cleanup(service, task).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn alerts_and_demo_signing_gate() {
    let (service, task, actor) = fixture().await;
    let mut cfg = (*service.cfg).clone();
    cfg.alert_webhook = Some(service.cfg.relay_origin.clone());
    let mut alerts = service.clone();
    alerts.cfg = Arc::new(cfg);
    alerts
        .incident(
            "fixture-alert",
            "P0",
            None,
            "integrity",
            json!({"simulation":true}),
        )
        .await
        .unwrap();
    alerts.notify_incidents().await.unwrap();
    let client = alerts.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT notifications FROM otc_incidents WHERE incident_key='fixture-alert'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i32>(0), 1);
    assert!(alerts.ready().await.is_err());
    client
        .batch_execute("UPDATE otc_controls SET paused=FALSE,signing_paused=FALSE")
        .await
        .unwrap();
    let mut cfg = (*service.cfg).clone();
    cfg.demo = true;
    let mut demo = service.clone();
    demo.cfg = Arc::new(cfg);
    let id = negotiated(&demo, &actor, Direction::Buy, "100", "200").await;
    accept(&demo, &actor, id).await;
    assert!(service
        .prepare(&actor, id, "demo-must-stay-demo", "payment")
        .await
        .is_err());
    cleanup(service, task).await;
}
#[test]
fn pinned_peer_fixtures_preserve_direction_mapping() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/otc/negotiation.json")).unwrap();
    for fixture in fixtures.as_array().unwrap() {
        let direction = if fixture["direction"] == "buy" {
            Direction::Buy
        } else {
            Direction::Sell
        };
        let mut terms = terms(direction);
        terms.proposal = fixture["proposal"].clone();
        terms.customer_actor = "https://customer.example/actors/customer".into();
        terms.desk_actor = "https://desk.example/actors/desk".into();
        assert_eq!(terms.proposal["@context"], protocol::context());
        assert_eq!(terms.proposal["unitBased"], false);
        assert_eq!(
            terms.proposal["publishes"]["resourceQuantity"]["hasNumericalValue"],
            terms.output
        );
        assert_eq!(
            terms.proposal["reciprocal"]["resourceQuantity"]["hasNumericalValue"],
            terms.input
        );
        let cfg = config();
        assert_eq!(
            terms.proposal["publishes"]["resourceConformsTo"],
            if direction == Direction::Buy {
                cfg.ever_resource
            } else {
                cfg.usdt_resource
            }
        );
        protocol::validate_offer(&fixture["offer"], &terms, Utc::now()).unwrap();
        protocol::validate_acceptance(&fixture["acceptance"], &fixture["offer"], &terms).unwrap();
    }
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn invalidated_receipt_reconciles_without_duplicate_fee() {
    let (service, task, actor) = fixture().await;
    let id = negotiated(&service, &actor, Direction::Buy, "100", "200").await;
    accept(&service, &actor, id).await;
    let trade = service.owned_trade(&actor, id).await.unwrap();
    let payment = evidence(&trade, "payment", "original-payment");
    let payout = evidence(&trade, "payout", "original-payout");
    service.credit(id, "payment", &payment).await.unwrap();
    service.credit(id, "payout", &payout).await.unwrap();
    let client = service.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE otc_evidence SET valid=FALSE WHERE trade_id=$1 AND kind='payment'",
            &[&id],
        )
        .await
        .unwrap();
    client
        .execute("UPDATE otc_trades SET state='review' WHERE id=$1", &[&id])
        .await
        .unwrap();
    service.credit(id, "payment", &payment).await.unwrap();
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().state,
        "completed"
    );
    let fees: i64 = client
        .query_one(
            "SELECT count(*) FROM otc_ledger WHERE trade_id=$1 AND kind='fee_accrued'",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(fees, 1);
    let metrics = service.metrics(&service.cfg.desk_actor).await.unwrap();
    assert!(metrics.contains("event=\"completed\",direction=\"buy\"} 1\n"));
    assert_eq!(
        service.dashboard(&service.cfg.desk_actor).await.unwrap()["funnel"]["completed"],
        1
    );
    cleanup(service, task).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn short_payment_can_only_refund_the_verified_actual_amount() {
    let (service, task, actor) = fixture().await;
    let id = negotiated(&service, &actor, Direction::Buy, "200", "400").await;
    accept(&service, &actor, id).await;
    let trade = service.owned_trade(&actor, id).await.unwrap();
    let mut partial = evidence(&trade, "payment", "short-payment");
    partial.leg.amount = "100000000".into();
    partial.leg.transfer_amount = "100000000".into();
    let attempt = Uuid::new_v4();
    service.pool.get().await.unwrap().execute("INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,'payment','ethereum','submitted',$3,'partial-fixture')",&[&attempt,&id,&json!({"leg":trade.terms.leg("payment").unwrap()})]).await.unwrap();
    assert!(service.credit(id, "payment", &partial).await.is_err());
    service.review_payment(id, attempt, &partial).await.unwrap();
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().state,
        "review"
    );
    assert!(service
        .prepare(&service.cfg.desk_actor, id, "no-short-fill", "payout")
        .await
        .is_err());
    let dashboard = service.dashboard(&service.cfg.desk_actor).await.unwrap();
    assert_eq!(
        dashboard["reporting"]["outstanding_obligations_by_asset"][0]["units"],
        "100000000"
    );
    assert_eq!(
        dashboard["reporting"]["agreed_usdt_leg_notional_units"],
        "200000000"
    );
    let prepared = service
        .prepare(&service.cfg.desk_actor, id, "actual-refund", "refund")
        .await
        .unwrap();
    assert_eq!(prepared["instructions"]["leg"]["amount"], "100000000");
    let mut refund = evidence(&trade, "refund", "actual-refund");
    refund.leg = serde_json::from_value(prepared["instructions"]["leg"].clone()).unwrap();
    service.credit(id, "refund", &refund).await.unwrap();
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().state,
        "refunded"
    );
    let count: i64 = service
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM otc_ledger WHERE trade_id=$1", &[&id])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    assert_eq!(
        service.owned_trade(&actor, id).await.unwrap().terms.input,
        "200"
    );
    cleanup(service, task).await;
}

use super::*;
use crate::otc::{
    model::{Direction, Evidence, Trade},
    Service,
};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

fn policy() -> EscrowPolicy {
    EscrowPolicy {
        factory: format!("0x{}", "8".repeat(40)),
        factory_code_hash: format!("0x{}", "a".repeat(64)),
        arbitrator: format!("0x{}", "9".repeat(40)),
        singleton: format!("0x{}", "7".repeat(40)),
        singleton_code_hash: format!("0x{}", "b".repeat(64)),
    }
}
async fn trade(service: &Service, customer: &str, direction: Direction, p: EscrowPolicy) -> Trade {
    trade_with_wallets(service, customer, direction, p, None).await
}
async fn trade_with_wallets(
    service: &Service,
    customer: &str,
    direction: Direction,
    p: EscrowPolicy,
    owners: Option<(String, String)>,
) -> Trade {
    let mut terms = crate::otc::tests::terms(direction);
    terms.demo = false;
    terms.customer_actor = customer.into();
    terms.desk_actor = service.cfg.desk_actor.clone();
    terms.escrow = Some(p);
    if let Some((customer_wallet, desk_wallet)) = owners {
        terms.customer_usdt = customer_wallet;
        terms.desk_usdt = desk_wallet;
    }
    let id = Uuid::new_v4();
    let rfq_id = Uuid::new_v4();
    let client = service.pool.get().await.unwrap();
    client.execute("INSERT INTO otc_rfq(id,actor,direction,input,listing_id,body,idempotency_key) VALUES($1,$2,$3,$4,'fixture','{}',$5)", &[&rfq_id,&customer,&if direction==Direction::Buy {"buy"}else{"sell"},&terms.input,&id.to_string()]).await.unwrap();
    client.execute("INSERT INTO otc_trades(id,rfq_id,customer_actor,desk_actor,terms,state) VALUES($1,$2,$3,$4,$5,'accepted')", &[&id,&rfq_id,&customer,&service.cfg.desk_actor,&serde_json::to_value(&terms).unwrap()]).await.unwrap();
    let t = Trade {
        id,
        rfq_id,
        state: "accepted".into(),
        terms,
        offer: None,
        decision: None,
        created_at: Utc::now(),
    };
    service.ensure_escrow(&t).await.unwrap();
    t
}

#[test]
fn cap_uses_ever_leg_in_both_directions_and_excludes_receiving_cost() {
    for direction in [Direction::Buy, Direction::Sell] {
        let mut terms = crate::otc::tests::terms(direction);
        terms.ever_receiving_cost_units = "1000000000".into();
        let amount = if direction == Direction::Buy {
            &mut terms.output_units
        } else {
            &mut terms.input_units
        };
        *amount = MAX_EVER_UNITS.to_string();
        assert!(validate_amounts(&terms).is_ok());
        let amount = if direction == Direction::Buy {
            &mut terms.output_units
        } else {
            &mut terms.input_units
        };
        *amount = (MAX_EVER_UNITS + 1).to_string();
        assert!(validate_amounts(&terms).is_err());
    }
}

#[tokio::test]
#[ignore = "needs OTC_TEST_DATABASE_URL"]
async fn database_arbitrator_reads_only_assigned_accepted_trades_and_has_no_desk_power() {
    let (service, task, customer) = crate::otc::tests::fixture().await;
    let assigned = trade(&service, &customer, Direction::Buy, policy()).await;
    let arb = "https://relay.example/actors/arbitrator";
    let client = service.pool.get().await.unwrap();
    client
        .execute(
            "INSERT INTO otc_login_wallets(chain,address,actor) VALUES('ethereum',$1,$2)",
            &[&policy().arbitrator, &arb],
        )
        .await
        .unwrap();
    assert!(service.escrow_details(arb, assigned.id).await.is_ok());
    assert!(service.details(arb, assigned.id).await.is_ok());
    assert!(service.desk(arb).is_err());
    assert!(service.dashboard(arb).await.is_err());
    assert!(service.owned_trade(arb, assigned.id).await.is_err());
    assert!(service
        .escrow_trade("https://relay.example/actors/unrelated", assigned.id)
        .await
        .is_err());
    let mut other_policy = policy();
    other_policy.arbitrator = format!("0x{}", "4".repeat(40));
    let other = trade(&service, &customer, Direction::Sell, other_policy).await;
    assert!(service.escrow_details(arb, other.id).await.is_err());
    let page = service.trades(arb).await.unwrap();
    assert_eq!(page["trades"].as_array().unwrap().len(), 1);
    assert_eq!(page["rfqs"], json!([]));
    let changed = json!({"changed":true});
    assert!(client
        .execute(
            "UPDATE otc_escrows SET deployment=$2 WHERE trade_id=$1",
            &[&assigned.id, &changed]
        )
        .await
        .is_err());
    task.abort();
}

#[tokio::test]
#[ignore = "needs OTC_TEST_DATABASE_URL"]
async fn database_refund_preserves_delivered_or_unresolved_ever_for_review() {
    for unresolved in [false, true] {
        let (service, task, customer) = crate::otc::tests::fixture().await;
        let t = trade(&service, &customer, Direction::Sell, policy()).await;
        let client = service.pool.get().await.unwrap();
        client
            .execute(
                "UPDATE otc_escrows SET state='refunded',expires_at=$2 WHERE trade_id=$1",
                &[&t.id, &(Utc::now() + Duration::hours(4))],
            )
            .await
            .unwrap();
        client.execute("INSERT INTO otc_escrow_evidence(trade_id,kind,identity,evidence,included_at) VALUES($1,'refund',$2,'{}',now())", &[&t.id,&t.id.to_string()]).await.unwrap();
        if unresolved {
            client.execute("INSERT INTO otc_attempts(id,trade_id,kind,chain,state,instructions,idempotency_key) VALUES($1,$2,'payment','everscale','unknown','{}','fixture')", &[&Uuid::new_v4(),&t.id]).await.unwrap();
        } else {
            let leg = t.terms.leg("payment").unwrap();
            let evidence = Evidence {
                chain: "everscale".into(),
                identity: "ever-receipt".into(),
                leg,
                included_at: Utc::now(),
                canonical: true,
                finalized: true,
                successful: true,
                raw: json!({"id":"ever-receipt"}),
            };
            client.execute("INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES('everscale','ever-receipt',$1,'payment',$2,now())", &[&t.id,&serde_json::to_value(evidence).unwrap()]).await.unwrap();
        }
        service.finish_escrow(t.id).await.unwrap();
        assert_eq!(
            service.owned_trade(&customer, t.id).await.unwrap().state,
            "review"
        );
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM otc_ledger WHERE trade_id=$1",
                &[&t.id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        task.abort();
    }
}

#[tokio::test]
#[ignore = "needs OTC_TEST_DATABASE_URL"]
async fn database_completed_escrow_accrues_fee_once_in_both_directions() {
    for direction in [Direction::Buy, Direction::Sell] {
        let (service, task, customer) = crate::otc::tests::fixture().await;
        let t = trade(&service, &customer, direction, policy()).await;
        let at = Utc::now();
        let client = service.pool.get().await.unwrap();
        client
            .execute(
                "UPDATE otc_escrows SET state='released',expires_at=$2 WHERE trade_id=$1",
                &[&t.id, &(at + Duration::hours(4))],
            )
            .await
            .unwrap();
        for (kind, identity, when) in [
            ("funding", "deposit", at - Duration::seconds(1)),
            ("release", "release", at + Duration::seconds(1)),
        ] {
            client.execute("INSERT INTO otc_escrow_evidence(trade_id,kind,identity,evidence,included_at) VALUES($1,$2,$3,'{}',$4)", &[&t.id,&kind,&identity,&when]).await.unwrap();
        }
        let kind = if direction == Direction::Buy {
            "payout"
        } else {
            "payment"
        };
        let evidence = Evidence {
            chain: "everscale".into(),
            identity: "ever-receipt".into(),
            leg: t.terms.leg(kind).unwrap(),
            included_at: at,
            canonical: true,
            finalized: true,
            successful: true,
            raw: json!({"id":"ever-receipt"}),
        };
        service
            .credit_escrow_ever(t.id, kind, &evidence)
            .await
            .unwrap();
        service
            .credit_escrow_ever(t.id, kind, &evidence)
            .await
            .unwrap();
        service.finish_escrow(t.id).await.unwrap();
        assert_eq!(
            service.owned_trade(&customer, t.id).await.unwrap().state,
            "completed"
        );
        let row=client.query_one("SELECT count(*),sum(amount)::text FROM otc_ledger WHERE trade_id=$1 AND kind='fee_accrued'", &[&t.id]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 1);
        assert_eq!(row.get::<_, String>(1), "5000000");
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM otc_outbox WHERE purpose='confirmation'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 1);
        task.abort();
    }
}

#[tokio::test]
#[ignore = "needs OTC_TEST_DATABASE_URL"]
async fn database_invalidated_deployment_blocks_completion_and_fee() {
    let (service, task, customer) = crate::otc::tests::fixture().await;
    let t = trade(&service, &customer, Direction::Buy, policy()).await;
    let client = service.pool.get().await.unwrap();
    let at = Utc::now();
    client
        .execute(
            "UPDATE otc_escrows SET state='review',expires_at=$2 WHERE trade_id=$1",
            &[&t.id, &(at + Duration::hours(4))],
        )
        .await
        .unwrap();
    for (kind, valid) in [("deployment", false), ("funding", true), ("release", true)] {
        client.execute("INSERT INTO otc_escrow_evidence(trade_id,kind,identity,evidence,included_at,valid) VALUES($1,$2,$2,'{}',$3,$4)", &[&t.id,&kind,&at,&valid]).await.unwrap();
    }
    let evidence = Evidence {
        chain: "everscale".into(),
        identity: "ever-receipt".into(),
        leg: t.terms.leg("payout").unwrap(),
        included_at: at,
        canonical: true,
        finalized: true,
        successful: true,
        raw: json!({}),
    };
    service
        .credit_escrow_ever(t.id, "payout", &evidence)
        .await
        .unwrap();
    assert_eq!(
        service.owned_trade(&customer, t.id).await.unwrap().state,
        "review"
    );
    let count: i64 = client
        .query_one(
            "SELECT count(*) FROM otc_ledger WHERE trade_id=$1",
            &[&t.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    task.abort();
}

#[test]
fn exact_handoff_rejects_wrong_destination_nonce_or_value() {
    let call = serde_json::json!({"from":format!("0x{}","1".repeat(40)),"to":format!("0x{}","2".repeat(40)),"data":"0x1234","nonce":"0x2"});
    let transaction = serde_json::json!({"from":call["from"],"to":call["to"],"input":call["data"],"nonce":"0x2","value":"0x0"});
    assert!(observer::matching_call(&transaction, &call));
    for (key, value) in [
        ("to", serde_json::json!(format!("0x{}", "3".repeat(40)))),
        ("nonce", serde_json::json!("0x3")),
        ("value", serde_json::json!("0x1")),
        ("input", serde_json::json!("0xabcd")),
    ] {
        let mut bad = transaction.clone();
        bad[key] = value;
        assert!(!observer::matching_call(&bad, &call), "changed {key}");
    }
}

#[test]
fn typed_approvals_bind_owner_chain_and_transaction() {
    use k256::ecdsa::SigningKey;
    let safe = format!("0x{}", "1".repeat(40));
    let vault = format!("0x{}", "2".repeat(40));
    let (digest, _) = abi::safe_tx(&safe, &vault, "release", alloy_primitives::U256::ZERO).unwrap();
    let key = SigningKey::from_bytes((&[3u8; 32]).into()).unwrap();
    let public = key.verifying_key().to_encoded_point(false);
    let owner = format!(
        "0x{}",
        wallet::hex(&alloy_primitives::keccak256(&public.as_bytes()[1..])[12..])
    );
    let (signature, recovery) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
    let mut bytes = signature.to_bytes().to_vec();
    bytes.push(recovery.to_byte() + 27);
    let signature = abi::hex(bytes);
    assert!(abi::verify_signature(digest, &signature, &owner).is_ok());
    let (changed, _) =
        abi::safe_tx(&safe, &vault, "refund_early", alloy_primitives::U256::ZERO).unwrap();
    assert!(abi::verify_signature(changed, &signature, &owner).is_err());
    assert!(abi::verify_signature(digest, &signature, &safe).is_err());
    let (changed, _) =
        abi::safe_tx(&safe, &vault, "release", alloy_primitives::U256::from(1)).unwrap();
    assert!(abi::verify_signature(changed, &signature, &owner).is_err());
}

fn local_owner(number: u8) -> (String, k256::ecdsa::SigningKey) {
    let mut bytes = [0u8; 32];
    bytes[31] = number;
    let key = k256::ecdsa::SigningKey::from_bytes((&bytes).into()).unwrap();
    let public = key.verifying_key().to_encoded_point(false);
    let address = format!(
        "0x{}",
        wallet::hex(&alloy_primitives::keccak256(&public.as_bytes()[1..])[12..])
    );
    (address, key)
}
fn local_signature(number: u8, digest: alloy_primitives::B256) -> String {
    let (_, key) = local_owner(number);
    let (signature, recovery) = key.sign_prehash_recoverable(digest.as_slice()).unwrap();
    let mut bytes = signature.to_bytes().to_vec();
    bytes.push(recovery.to_byte() + 27);
    abi::hex(bytes)
}
async fn local_rpc(service: &Service, method: &str, params: Value) -> Value {
    let body: Value = service
        .http
        .post(&service.cfg.ethereum_rpc[0])
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(body.get("error").is_none(), "{method}: {body}");
    body["result"].clone()
}
async fn local_send(service: &Service, instructions: &Value) -> String {
    local_rpc(
        service,
        "anvil_impersonateAccount",
        json!([instructions["from"]]),
    )
    .await;
    local_rpc(
        service,
        "anvil_setBalance",
        json!([instructions["from"], "0x3635c9adc5dea00000"]),
    )
    .await;
    let reference=local_rpc(service,"eth_sendTransaction",json!([{"from":instructions["from"],"to":instructions["to"],"data":instructions["data"],"value":"0x0","nonce":instructions["nonce"],"gas":"0xb71b00"}])).await.as_str().unwrap().to_owned();
    local_rpc(service, "anvil_mine", json!(["0x4"])).await;
    reference
}

#[tokio::test]
#[ignore = "run contracts/otc/scripts/integration.mjs with a disposable PostgreSQL database"]
async fn local_chain_roundtrip() {
    let p = EscrowPolicy {
        factory: std::env::var("ESCROW_TEST_FACTORY").unwrap(),
        factory_code_hash: std::env::var("ESCROW_TEST_FACTORY_CODE_HASH").unwrap(),
        singleton: std::env::var("ESCROW_TEST_SINGLETON").unwrap(),
        singleton_code_hash: std::env::var("ESCROW_TEST_SINGLETON_CODE_HASH").unwrap(),
        arbitrator: std::env::var("ESCROW_TEST_ARBITRATOR").unwrap(),
    };
    for (direction, action) in [
        (Direction::Buy, "release"),
        (Direction::Sell, "release"),
        (Direction::Buy, "refund_early"),
        (Direction::Sell, "refund_expired"),
    ] {
        let (mut service, task, customer) = crate::otc::tests::fixture().await;
        let mut cfg = (*service.cfg).clone();
        cfg.ethereum_rpc = vec![std::env::var("ESCROW_TEST_RPC_URL").unwrap(); 2];
        service.cfg = std::sync::Arc::new(cfg);
        let buyer = local_owner(11).0;
        let desk = local_owner(22).0;
        let t = trade_with_wallets(
            &service,
            &customer,
            direction,
            p.clone(),
            Some((buyer.clone(), desk.clone())),
        )
        .await;
        let client = service.pool.get().await.unwrap();
        let deployment = Deployment::from_terms(t.id, &t.terms).unwrap();
        for (actor, address) in [(&customer, &buyer), (&service.cfg.desk_actor, &desk)] {
            client
                .execute(
                    "UPDATE otc_wallets SET address=$2 WHERE actor=$1 AND chain='ethereum'",
                    &[&actor, &address],
                )
                .await
                .unwrap();
        }
        let arb = format!("{}/actors/arbitrator", service.cfg.relay_origin);
        client
            .execute(
                "INSERT INTO otc_login_wallets(chain,address,actor) VALUES('ethereum',$1,$2)",
                &[&p.arbitrator, &arb],
            )
            .await
            .unwrap();
        client.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES($1,'ethereum',$2,now())", &[&arb,&p.arbitrator]).await.unwrap();
        let digest = abi::consent(&deployment, &p).unwrap().0;
        service
            .escrow_consent(
                &customer,
                t.id,
                "customer-consent",
                ConsentRequest {
                    signature: local_signature(11, digest),
                },
            )
            .await
            .unwrap();
        service
            .escrow_consent(
                &service.cfg.desk_actor,
                t.id,
                "desk-consent",
                ConsentRequest {
                    signature: local_signature(22, digest),
                },
            )
            .await
            .unwrap();
        let call = service
            .escrow_prepare(
                &customer,
                t.id,
                "deploy",
                CallRequest {
                    action: "deploy".into(),
                    proposal_id: None,
                },
            )
            .await
            .unwrap();
        let reference = local_send(&service, &call["instructions"]).await;
        // Lost reference is intentionally not posted: the observer must recover it from its checkpoint.
        service.escrow_tick().await.unwrap();
        let mut current = service.escrow_details(&customer, t.id).await.unwrap();
        assert_eq!(
            current["state"], "created",
            "deployment did not reconcile: {current}; reference={reference}"
        );
        let payer = if direction == Direction::Buy {
            &customer
        } else {
            &service.cfg.desk_actor
        };
        let payer_address = if direction == Direction::Buy {
            &buyer
        } else {
            &desk
        };
        let mint = abi::hex(
            alloy_primitives::keccak256("mint(address,uint256)")[..4]
                .iter()
                .copied()
                .chain(alloy_sol_types::SolValue::abi_encode(&(
                    abi::addr(payer_address).unwrap(),
                    alloy_primitives::U256::from(100_000_000u64),
                )))
                .collect::<Vec<_>>(),
        );
        let nonce = local_rpc(
            &service,
            "eth_getTransactionCount",
            json!([buyer, "pending"]),
        )
        .await;
        local_send(
            &service,
            &json!({"from":buyer,"to":crate::otc::config::USDT,"data":mint,"nonce":nonce}),
        )
        .await;
        assert!(service
            .escrow_prepare(
                payer,
                t.id,
                "fund-without-allowance",
                CallRequest {
                    action: "fund".into(),
                    proposal_id: None
                }
            )
            .await
            .is_err());
        for action in ["approve", "fund"] {
            let call = service
                .escrow_prepare(
                    payer,
                    t.id,
                    action,
                    CallRequest {
                        action: action.into(),
                        proposal_id: None,
                    },
                )
                .await
                .unwrap();
            let reference = local_send(&service, &call["instructions"]).await;
            service
                .escrow_reference(
                    payer,
                    t.id,
                    ReferenceRequest {
                        call_id: serde_json::from_value(call["id"].clone()).unwrap(),
                        reference,
                    },
                )
                .await
                .unwrap();
            service.escrow_tick().await.unwrap();
        }
        current = service.escrow_details(&customer, t.id).await.unwrap();
        assert_eq!(
            current["state"], "funded",
            "deposit did not reconcile: {current}"
        );
        assert_eq!(
            service.dashboard(&service.cfg.desk_actor).await.unwrap()["open_funded_cases"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        if action == "refund_expired" {
            local_rpc(&service, "evm_increaseTime", json!([LIFETIME_SECONDS])).await;
            local_rpc(&service, "evm_mine", json!([])).await;
            let call = service
                .escrow_prepare(
                    &arb,
                    t.id,
                    "timeout",
                    CallRequest {
                        action: action.into(),
                        proposal_id: None,
                    },
                )
                .await
                .unwrap();
            local_send(&service, &call["instructions"]).await;
        } else {
            if action == "release" {
                let kind = if direction == Direction::Buy {
                    "payout"
                } else {
                    "payment"
                };
                let sender = if direction == Direction::Buy {
                    &service.cfg.desk_actor
                } else {
                    &customer
                };
                let attempt = service
                    .prepare(sender, t.id, "ever-handoff", kind)
                    .await
                    .unwrap();
                let funding=client.query_one("SELECT included_at FROM otc_escrow_evidence WHERE trade_id=$1 AND kind='funding'", &[&t.id]).await.unwrap().get::<_,chrono::DateTime<Utc>>(0);
                let evidence = Evidence {
                    chain: "everscale".into(),
                    identity: format!("ever-{}", t.id),
                    leg: t.terms.leg(kind).unwrap(),
                    included_at: funding + Duration::seconds(1),
                    canonical: true,
                    finalized: true,
                    successful: true,
                    raw: json!({"id":"fixture-ever"}),
                };
                service.credit(t.id, kind, &evidence).await.unwrap();
                assert_eq!(attempt["state"], "unknown");
            }
            service
                .escrow_propose(
                    &arb,
                    t.id,
                    "settlement",
                    ProposalRequest {
                        action: action.into(),
                        dispute: false,
                        reason: "".into(),
                        evidence: json!({}),
                    },
                )
                .await
                .unwrap();
            current = service.escrow_details(&arb, t.id).await.unwrap();
            let proposal = &current["proposals"][0];
            let proposal_id: Uuid = serde_json::from_value(proposal["id"].clone()).unwrap();
            let digest = abi::hash(proposal["hash"].as_str().unwrap()).unwrap();
            for (actor, key) in [(&customer, 11), (&arb, 33)] {
                service
                    .escrow_approve(
                        actor,
                        t.id,
                        ApprovalRequest {
                            proposal_id,
                            signature: local_signature(key, digest),
                        },
                    )
                    .await
                    .unwrap();
            }
            let call = service
                .escrow_prepare(
                    &service.cfg.desk_actor,
                    t.id,
                    "execute",
                    CallRequest {
                        action: "execute".into(),
                        proposal_id: Some(proposal_id),
                    },
                )
                .await
                .unwrap();
            local_send(&service, &call["instructions"]).await;
        }
        service.escrow_tick().await.unwrap();
        service.escrow_tick().await.unwrap();
        let final_trade = service.owned_trade(&customer, t.id).await.unwrap();
        assert_eq!(
            final_trade.state,
            if action == "release" {
                "completed"
            } else {
                "refunded"
            },
            "{direction:?}/{action}"
        );
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM otc_ledger WHERE trade_id=$1 AND kind='fee_accrued'",
                &[&t.id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, if action == "release" { 1 } else { 0 });
        task.abort();
    }
}

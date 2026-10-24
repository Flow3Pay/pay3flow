use super::*;
use ed25519_dalek::{Signer, SigningKey};

fn receipt_fixture() -> (Value, Value, Value) {
    (
        json!({"id":"hash","status":3,"account_addr":"wallet","now":100,
        "in_message":{"dst":"wallet","msg_type":1},"aborted":false,
        "compute":{"success":true,"exit_code":0},"action":{"success":true,"result_code":0}}),
        json!({"Header":{"pubkey":"owner","time":100010},"BodyCall":{"confirmTransaction":{"transactionId":"18446744073709551615"}}}),
        json!({"publicKey":"owner","payload":{"method":"confirmTransaction","params":{"transactionId":"18446744073709551615"}}}),
    )
}

#[test]
fn receipt_cannot_clear_an_unrelated_or_older_handoff() {
    let (receipt, decoded, call) = receipt_fixture();
    assert_eq!(
        receipt_state(&receipt, &decoded, &call, "hash", "wallet", 100000).unwrap(),
        "accepted"
    );
    for (pointer, replacement) in [
        ("/id", json!("other")),
        ("/status", json!(2)),
        ("/account_addr", json!("other")),
        ("/in_message/dst", json!("other")),
        ("/in_message/msg_type", json!(0)),
        ("/now", json!(50)),
        ("/compute/success", json!(false)),
        ("/action/result_code", json!(37)),
    ] {
        let mut bad = receipt.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            receipt_state(&bad, &decoded, &call, "hash", "wallet", 100000).is_err(),
            "{pointer}"
        );
    }
    for (pointer, replacement) in [
        ("/Header/pubkey", json!("other")),
        ("/Header/time", json!(99000)),
        ("/BodyCall/confirmTransaction/transactionId", json!("1")),
    ] {
        let mut bad = decoded.clone();
        *bad.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            receipt_state(&receipt, &bad, &call, "hash", "wallet", 100000).is_err(),
            "{pointer}"
        );
    }
    let mut failed = receipt.clone();
    failed["aborted"] = json!(true);
    assert_eq!(
        receipt_state(&failed, &decoded, &call, "hash", "wallet", 100000).unwrap(),
        "failed"
    );
}

#[test]
fn forged_or_cross_network_external_signature_is_rejected_even_on_abort() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let public = super::super::wallet::hex(key.verifying_key().as_bytes());
    let mut data = CellBuilder::new();
    data.store_u64(1234).unwrap();
    let data = data.build().unwrap();
    for network in [42i32, -1] {
        let mut to_sign = network.to_be_bytes().to_vec();
        to_sign.extend_from_slice(&data.repr_hash().0);
        let signature = key.sign(&to_sign);
        let mut body = CellBuilder::new();
        body.store_bit_one().unwrap();
        body.store_raw(&signature.to_bytes(), 512).unwrap();
        body.store_slice(data.as_slice().unwrap()).unwrap();
        let body = STANDARD.encode(Boc::encode(body.build().unwrap()));
        assert_eq!(
            verify_external_signature(&body, &public).is_ok(),
            network == 42
        );
        assert!(verify_external_signature(&body, &"00".repeat(32)).is_err());
    }
    assert!(verify_external_signature("te6ccgEBAQEAAgAAAA==", &public).is_err());
}

#[test]
fn browser_wire_request_is_strict_and_cli_hash_is_unchanged() {
    let value = agreement();
    let trade: Trade = serde_json::from_value(value.clone()).unwrap();
    let request = json!({"trade":value,"terms_hash":trade.terms_hash().unwrap(),"operation":{"public_key":value["terms"]["buyer"]["public_key"],"action":"release","transaction_id":null}});
    let parsed: PrepareRequest = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), request);
    let mut bad = request.clone();
    bad["secret"] = json!("do not upload keys");
    assert!(serde_json::from_value::<PrepareRequest>(bad).is_err());
    let mut bad = request;
    bad["operation"]["transaction_id"] = json!(18446744073709551615u64);
    assert!(serde_json::from_value::<PrepareRequest>(bad).is_err());
}

fn agreement() -> Value {
    let keys: Vec<_> = (1..=4)
        .map(|i| {
            super::super::wallet::hex(SigningKey::from_bytes(&[i; 32]).verifying_key().as_bytes())
        })
        .collect();
    json!({"schema_version":1,"terms":{"trade_id":"pay3flow001","network_id":42,
        "buyer":{"public_key":keys[0],"address":format!("0:{}","a".repeat(64))},
        "seller":{"public_key":keys[1],"address":format!("0:{}","b".repeat(64))},
        "arbitrator":keys[2],"deployment_key":keys[3],"amount":"2000000000"},
        "wallet_address":format!("0:{}","c".repeat(64))})
}

struct Runtime {
    _dir: tempfile::TempDir,
    script: std::path::PathBuf,
    state: std::path::PathBuf,
    agreement: Trade,
}
impl Runtime {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("backend.py");
        std::fs::write(
            &script,
            include_str!("../../../tests/fixtures/ever-multisig-backend.py"),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let state = script.with_extension("json");
        let manifest = agreement();
        let owners = [
            &manifest["terms"]["seller"]["public_key"],
            &manifest["terms"]["arbitrator"],
            &manifest["terms"]["buyer"]["public_key"],
        ]
        .map(|key| format!("0x{}", key.as_str().unwrap()));
        std::fs::write(
            &state,
            json!({"address":manifest["wallet_address"],"owners":owners,"required":2}).to_string(),
        )
        .unwrap();
        Self {
            _dir: dir,
            script,
            state,
            agreement: serde_json::from_value(manifest).unwrap(),
        }
    }
    fn review(&self, operation: Option<&BrowserOperation>) -> anyhow::Result<Value> {
        self.agreement.browser_review(
            &backend(
                self.script.to_string_lossy().into_owned(),
                Some("https://test.invalid".into()),
            ),
            &self.agreement.terms_hash()?,
            operation,
        )
    }
    fn change(&self, key: &str, value: Value) {
        let mut state: Value =
            serde_json::from_slice(&std::fs::read(&self.state).unwrap()).unwrap();
        state[key] = value;
        std::fs::write(&self.state, state.to_string()).unwrap();
    }
}

#[test]
fn native_runtime_checks_code_network_owners_and_builds_only_public_calls() {
    let runtime = Runtime::new();
    let manifest = serde_json::to_value(&runtime.agreement).unwrap();
    let operation: BrowserOperation = serde_json::from_value(json!({"public_key":manifest["terms"]["buyer"]["public_key"],"action":"release","transaction_id":null})).unwrap();
    let review = runtime.review(Some(&operation)).unwrap();
    assert_eq!(review["call"]["payload"]["method"], "submitTransaction");
    assert_eq!(
        review["call"]["payload"]["params"]["dest"],
        manifest["terms"]["buyer"]["address"]
    );
    assert_eq!(
        review["call"]["publicKey"],
        manifest["terms"]["buyer"]["public_key"]
    );
    assert_eq!(review["owners"][2]["index"], 2);
    for (field, bad, original) in [
        ("network_id", json!(-1), json!(42)),
        (
            "code_hash",
            json!("bad"),
            json!(evernode_multisig::otc::SAFE_CODE_HASH),
        ),
        ("required", json!(1), json!(2)),
        ("balance", json!("2000000000"), json!("100000000000")),
    ] {
        runtime.change(field, bad);
        assert!(runtime.review(Some(&operation)).is_err(), "{field}");
        runtime.change(field, original);
    }
    let events = std::fs::read_to_string(runtime.state.with_extension("events")).unwrap();
    for command in ["sendfile", "message", "deploy_message", "--sign", "--keys"] {
        assert!(!events.contains(command));
    }
}

#[test]
fn second_approval_uses_actual_owner_index_and_rejects_modified_transfer() {
    let runtime = Runtime::new();
    let manifest = serde_json::to_value(&runtime.agreement).unwrap();
    let mut tx = json!({"id":"18446744073709551615","dest":manifest["terms"]["buyer"]["address"],"value":"2000000000","signsRequired":"2","sendFlags":"3","bounce":true,"payload":"te6ccgEBAQEAAgAAAA==","confirmationsMask":"1","signsReceived":"1"});
    runtime.change("transactions", json!([tx]));
    let operation:BrowserOperation = serde_json::from_value(json!({"public_key":manifest["terms"]["buyer"]["public_key"],"action":"release","transaction_id":"18446744073709551615"})).unwrap();
    assert_eq!(
        runtime.review(Some(&operation)).unwrap()["call"]["payload"]["params"]["transactionId"],
        "18446744073709551615"
    );
    tx["confirmationsMask"] = json!("4");
    runtime.change("transactions", json!([tx]));
    assert!(runtime.review(Some(&operation)).is_err());
    tx["confirmationsMask"] = json!("1");
    tx["value"] = json!("2000000001");
    runtime.change("transactions", json!([tx]));
    assert!(runtime.review(Some(&operation)).is_err());
}

#[tokio::test]
#[ignore = "requires OTC_TEST_DATABASE_URL"]
async fn durable_wallet_lock_and_receipt_uniqueness_survive_service_restart() {
    let (service, task, _) = super::super::tests::fixture().await;
    let client = service.pool.get().await.unwrap();
    client
        .batch_execute(include_str!("../../../migrations/otc_ever_multisig.sql"))
        .await
        .unwrap();
    client.execute("INSERT INTO otc_ever_agreements(address,terms_hash,manifest) VALUES('wallet','hash','{}')",&[]).await.unwrap();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    client.execute("INSERT INTO otc_ever_calls(id,address,actor,public_key,terms_hash,instructions,idempotency_key) VALUES($1,'wallet','buyer','key','hash','{}','one')",&[&first]).await.unwrap();
    let duplicate = client.execute("INSERT INTO otc_ever_calls(id,address,actor,public_key,terms_hash,instructions,idempotency_key) VALUES($1,'wallet','seller','key2','hash','{}','two')",&[&second]).await;
    assert!(duplicate.is_err());
    assert_eq!(
        service.clone().multisig_call(first).await.unwrap()["state"],
        "unknown"
    );
    client
        .execute(
            "UPDATE otc_ever_calls SET state='accepted',reference='receipt' WHERE id=$1",
            &[&first],
        )
        .await
        .unwrap();
    client.execute("INSERT INTO otc_ever_calls(id,address,actor,public_key,terms_hash,instructions,idempotency_key) VALUES($1,'wallet','seller','key2','hash','{}','two')",&[&second]).await.unwrap();
    assert!(client
        .execute(
            "UPDATE otc_ever_calls SET reference='receipt' WHERE id=$1",
            &[&second]
        )
        .await
        .is_err());
    assert_eq!(
        service.multisig_call(first).await.unwrap()["settlement_delivery_verified"],
        false
    );
    assert!(client
        .execute(
            "UPDATE otc_ever_agreements SET terms_hash='changed' WHERE address='wallet'",
            &[]
        )
        .await
        .is_err());
    assert!(client
        .execute(
            "UPDATE otc_ever_calls SET instructions='{\"changed\":true}' WHERE id=$1",
            &[&second]
        )
        .await
        .is_err());
    task.abort();
}

#[test]
#[ignore = "requires EVER_CLI_TEST_BINARY; strictly offline encoding and decoding"]
fn official_codec_message_matches_signature_and_receipt_validation() {
    let executable = std::env::var("EVER_CLI_TEST_BINARY").unwrap();
    let backend = backend(executable, None);
    let assets = Assets::new(Contract::Safe).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let key = SigningKey::from_bytes(&[1; 32]);
    let public = super::super::wallet::hex(key.verifying_key().as_bytes());
    let key_path = directory.path().join("test-only-key.json");
    evernode_multisig::storage::write_json(
        &key_path,
        &json!({"public":public,"secret":super::super::wallet::hex(&key.to_bytes())}),
    )
    .unwrap();
    let address = format!("0:{}", "c".repeat(64));
    let created = Utc::now().timestamp_millis();
    let params = json!({"dest":format!("0:{}","a".repeat(64)),"value":"2000000000","bounce":true,"allBalance":false,"payload":""});
    for method in ["submitTransaction", "confirmTransaction"] {
        let params = if method == "confirmTransaction" {
            json!({"transactionId":"18446744073709551615"})
        } else {
            params.clone()
        };
        let output = directory.path().join(format!("{method}.boc"));
        backend
            .run(&[
                "message".into(),
                address.clone(),
                method.into(),
                params.to_string(),
                "--abi".into(),
                assets.abi.to_string_lossy().into_owned(),
                "--sign".into(),
                key_path.to_string_lossy().into_owned(),
                "--raw".into(),
                "--output".into(),
                output.to_string_lossy().into_owned(),
                "--signature_id".into(),
                "42".into(),
                "--time".into(),
                (created + 10).to_string(),
            ])
            .unwrap();
        let message = backend
            .run(&[
                "decode".into(),
                "msg".into(),
                output.to_string_lossy().into_owned(),
                "--abi".into(),
                assets.abi.to_string_lossy().into_owned(),
            ])
            .unwrap();
        let body = message["Body"].as_str().unwrap();
        verify_external_signature(body, &public).unwrap();
        let decoded = backend
            .run(&[
                "decode".into(),
                "body".into(),
                body.into(),
                "--abi".into(),
                assets.abi.to_string_lossy().into_owned(),
            ])
            .unwrap();
        let receipt = json!({"id":"hash","status":3,"aborted":false,"account_addr":address,"now":(created+10)/1000,"in_message":{"dst":address,"msg_type":1},"compute":{"success":true,"exit_code":0},"action":null});
        let call = json!({"publicKey":public,"payload":{"method":method,"params":params}});
        assert_eq!(
            receipt_state(&receipt, &decoded, &call, "hash", &address, created).unwrap(),
            "accepted"
        );
    }
}

#[tokio::test]
#[ignore = "requires OTC_TEST_DATABASE_URL, EVER_CLI_TEST_BINARY, Python 3 and OpenSSL; local fixtures only"]
async fn configured_service_prepares_replays_blocks_and_reconciles_original_call() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let codec = std::env::var("EVER_CLI_TEST_BINARY").unwrap();
    let runtime = Runtime::new();
    let manifest = serde_json::to_value(&runtime.agreement).unwrap();
    let (mut service, relay, _) = super::super::tests::fixture().await;
    let actor = service.cfg.desk_actor.clone();
    let root = runtime._dir.path();
    let cert = root.join("local-test-cert.pem");
    let tls_key = root.join("local-test-key.pem");
    assert!(Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=127.0.0.1",
            "-addext",
            "subjectAltName=IP:127.0.0.1",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "extendedKeyUsage=serverAuth",
            "-keyout"
        ])
        .arg(&tls_key)
        .arg("-out")
        .arg(&cert)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
    let fixture = root.join("graphql.py");
    std::fs::write(
        &fixture,
        include_str!("../../../tests/fixtures/ever-multisig-graphql.py"),
    )
    .unwrap();
    let mut code = CellBuilder::new();
    code.store_u32(0x12345678).unwrap();
    let code = code.build().unwrap();
    let signing = SigningKey::from_bytes(&[1; 32]);
    let mut data = CellBuilder::new();
    data.store_raw(signing.verifying_key().as_bytes(), 256)
        .unwrap();
    let code_hash = super::super::wallet::hex(&code.repr_hash().0);
    runtime.change("single_key_account",json!({"id":manifest["terms"]["buyer"]["address"],"acc_type":1,"code_hash":code_hash,"code":STANDARD.encode(Boc::encode(code)),"data":STANDARD.encode(Boc::encode(data.build().unwrap()))}));
    runtime.change(
        "head",
        json!({"id":"head","global_id":42,"gen_utime":Utc::now().timestamp()}),
    );
    runtime.change("codec", json!(codec));
    let mut server = Child(
        Command::new("python3")
            .arg(&fixture)
            .arg(&runtime.state)
            .arg(&cert)
            .arg(&tls_key)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(server.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let port: u16 = line.trim().parse().unwrap();
    let endpoint = format!("https://127.0.0.1:{port}");
    let mut cfg = (*service.cfg).clone();
    cfg.enabled = false;
    cfg.demo = false;
    cfg.everscale_global_id = 42;
    cfg.everscale_lag_seconds = 60;
    cfg.everscale_graphql = vec![format!("{endpoint}/one"), format!("{endpoint}/two")];
    cfg.everscale_multisig_backend = Some(runtime.script.to_string_lossy().into_owned());
    cfg.everwallet_code_hashes = vec![code_hash];
    service.cfg = Arc::new(cfg);
    service.http = reqwest::Client::builder()
        .add_root_certificate(
            reqwest::Certificate::from_pem(&std::fs::read(&cert).unwrap()).unwrap(),
        )
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    assert!(service.multisig_available());
    let client = service.pool.get().await.unwrap();
    client
        .batch_execute(include_str!("../../../migrations/otc_ever_multisig.sql"))
        .await
        .unwrap();
    client
        .execute(
            "UPDATE otc_wallets SET address=$2 WHERE actor=$1 AND chain='everscale'",
            &[
                &actor,
                &manifest["terms"]["buyer"]["address"].as_str().unwrap(),
            ],
        )
        .await
        .unwrap();
    let hash = runtime.agreement.terms_hash().unwrap();
    let request:PrepareRequest = serde_json::from_value(json!({"trade":manifest,"terms_hash":hash,"operation":{"public_key":manifest["terms"]["buyer"]["public_key"],"action":"release","transaction_id":null}})).unwrap();
    let prepared = service
        .multisig_prepare(&actor, "one", request.clone())
        .await
        .unwrap();
    let id = Uuid::parse_str(prepared["id"].as_str().unwrap()).unwrap();
    let repeated = service
        .multisig_prepare(&actor, "one", request.clone())
        .await
        .unwrap();
    assert_eq!(prepared["id"], repeated["id"]);
    assert_eq!(repeated["resumed"], true);
    runtime.change("balance", json!("2000000000"));
    assert!(service
        .multisig_prepare(&actor, "one", request.clone())
        .await
        .is_err());
    runtime.change("balance", json!("100000000000"));
    client
        .execute("UPDATE otc_controls SET signing_paused=TRUE", &[])
        .await
        .unwrap();
    assert!(matches!(
        service
            .multisig_prepare(&actor, "one", request.clone())
            .await,
        Err(Error::Disabled)
    ));
    client
        .execute("UPDATE otc_controls SET signing_paused=FALSE", &[])
        .await
        .unwrap();
    assert!(matches!(
        service
            .multisig_prepare(&actor, "two", request.clone())
            .await,
        Err(Error::Conflict(_))
    ));
    let mut changed = request.clone();
    changed.operation.action = evernode_multisig::otc::Action::Refund;
    assert!(matches!(
        service.multisig_prepare(&actor, "one", changed).await,
        Err(Error::Conflict(_))
    ));
    let review = service
        .multisig_review(ReviewRequest {
            trade: runtime.agreement.clone(),
            terms_hash: hash,
        })
        .await
        .unwrap();
    assert_eq!(review["handoff"]["id"], id.to_string());
    let created: DateTime<Utc> = client
        .query_one("SELECT created_at FROM otc_ever_calls WHERE id=$1", &[&id])
        .await
        .unwrap()
        .get(0);
    let key_path = root.join("test-only-signing-key.json");
    evernode_multisig::storage::write_json(&key_path,&json!({"public":manifest["terms"]["buyer"]["public_key"],"secret":super::super::wallet::hex(&signing.to_bytes())})).unwrap();
    let assets = Assets::new(Contract::Safe).unwrap();
    let output = root.join("original.boc");
    let real = backend(codec, None);
    let call = &prepared["instructions"]["review"]["call"];
    real.run(&[
        "message".into(),
        manifest["wallet_address"].as_str().unwrap().into(),
        "submitTransaction".into(),
        call["payload"]["params"].to_string(),
        "--abi".into(),
        assets.abi.to_string_lossy().into_owned(),
        "--sign".into(),
        key_path.to_string_lossy().into_owned(),
        "--raw".into(),
        "--output".into(),
        output.to_string_lossy().into_owned(),
        "--signature_id".into(),
        "42".into(),
        "--time".into(),
        (created.timestamp_millis() + 10).to_string(),
    ])
    .unwrap();
    let decoded = real
        .run(&[
            "decode".into(),
            "msg".into(),
            output.to_string_lossy().into_owned(),
            "--abi".into(),
            assets.abi.to_string_lossy().into_owned(),
        ])
        .unwrap();
    let tx_hash = "e".repeat(64);
    let receipt = json!({"id":tx_hash,"account_addr":manifest["wallet_address"],"status":3,"aborted":false,"now":Utc::now().timestamp(),"in_message":{"id":"f".repeat(64),"dst":manifest["wallet_address"],"msg_type":1,"body":decoded["Body"]},"compute":{"success":true,"exit_code":0},"action":null});
    runtime.change("receipts", json!([receipt]));
    let reconciled = service
        .multisig_reference(
            id,
            ReferenceRequest {
                transaction_hash: tx_hash.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(reconciled["state"], "accepted");
    assert_eq!(reconciled["settlement_delivery_verified"], false);
    assert_eq!(
        service
            .multisig_reference(
                id,
                ReferenceRequest {
                    transaction_hash: tx_hash
                }
            )
            .await
            .unwrap()["state"],
        "accepted"
    );
    assert!(service
        .multisig_reference(
            id,
            ReferenceRequest {
                transaction_hash: "d".repeat(64)
            }
        )
        .await
        .is_err());
    runtime.change(
        "head",
        json!({"id":"bad-head","global_id":42,"gen_utime":i64::MIN}),
    );
    assert!(service
        .multisig_prepare(&actor, "three", request)
        .await
        .is_err());
    relay.abort();
}

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
};

use evernode_multisig::{crypto::Keypair, domain::PublicKey, storage};
use serde_json::{Value, json};

const PHRASES: [&str; 4] = [
    "ghost frost pool buzz rival mad naive rare shell tooth smart praise",
    "blanket time net universe ketchup maid way poem scatter blur limit drill",
    "abuse boss fly battle rubber wasp afraid hamster guide essence vibrant tattoo",
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
];
const ROLES: [&str; 4] = ["buyer", "seller", "arbitrator", "deployer"];

struct Harness {
    _temp: tempfile::TempDir,
    root: PathBuf,
    backend: PathBuf,
    state: PathBuf,
    public_keys: Vec<PublicKey>,
    real_backend: Option<PathBuf>,
}

impl Harness {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let backend = root.join("backend");
        fs::write(&backend, include_str!("fixtures/backend.py")).unwrap();
        fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
        let keys: Vec<_> = PHRASES
            .iter()
            .map(|p| Keypair::derive(p).unwrap())
            .collect();
        for (role, key) in ROLES.iter().zip(&keys) {
            let directory = root.join(role);
            storage::create_directory(&directory).unwrap();
            storage::write_json(&directory.join("key.json"), key).unwrap();
        }
        let harness = Self {
            state: root.join("state.json"),
            root,
            backend,
            public_keys: keys.iter().map(|k| k.public.clone()).collect(),
            _temp: temp,
            real_backend: None,
        };
        harness.set_state(json!({"address":format!("0:{}", "33".repeat(32)),"owners":[harness.public_keys[1].abi_value(),harness.public_keys[2].abi_value(),harness.public_keys[0].abi_value()],"required":2}));
        harness
    }
    fn key(&self, role: &str) -> String {
        self.root
            .join(role)
            .join("key.json")
            .to_str()
            .unwrap()
            .to_owned()
    }
    fn store(&self, peer: &str) -> PathBuf {
        self.root.join("stores").join(peer)
    }
    fn directory(&self, peer: &str) -> PathBuf {
        self.store(peer).join("otc/deal001")
    }
    fn address(&self) -> String {
        self.state()["address"].as_str().unwrap().to_owned()
    }
    fn state(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.state).unwrap()).unwrap()
    }
    fn set_state(&self, state: Value) {
        fs::write(&self.state, serde_json::to_vec(&state).unwrap()).unwrap();
    }
    fn change(&self, key: &str, value: Value) {
        let mut state = self.state();
        state[key] = value;
        self.set_state(state);
    }
    fn invoke(&self, peer: &str, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_evernode-rs"));
        command
            .args([
                "--wallet-dir",
                self.store(peer).to_str().unwrap(),
                "--backend",
                self.backend.to_str().unwrap(),
                "--url",
                "http://test.invalid",
            ])
            .args(args)
            .env("FAKE_BACKEND_STATE", &self.state)
            .env_remove("EVERNODE_URL");
        if let Some(backend) = &self.real_backend {
            command.env("FAKE_TVM_BACKEND", backend);
        } else {
            command.env_remove("FAKE_TVM_BACKEND");
        }
        command.output().unwrap()
    }
    fn ok(&self, peer: &str, args: &[&str]) -> Value {
        let output = self.invoke(peer, args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        for phrase in PHRASES {
            assert!(!text.contains(phrase));
        }
        serde_json::from_str(&text).unwrap()
    }
    fn fails(&self, peer: &str, args: &[&str]) -> String {
        let output = self.invoke(peer, args);
        assert!(
            !output.status.success(),
            "Unexpected success: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(!error.contains("PRIVATE_LEAK_SENTINEL"));
        error
    }
    fn create(&self) -> Value {
        self.ok(
            "deployer",
            &[
                "otc",
                "create",
                "-n",
                "deal001",
                "--buyer-pubkey",
                self.public_keys[0].as_hex(),
                "--buyer-address",
                &format!("0:{}", "11".repeat(32)),
                "--seller-pubkey",
                self.public_keys[1].as_hex(),
                "--seller-address",
                &format!("0:{}", "22".repeat(32)),
                "--arbitrator-pubkey",
                self.public_keys[2].as_hex(),
                "--deployment-pubkey",
                self.public_keys[3].as_hex(),
                "--amount",
                "1.000000001",
            ],
        )
    }
    fn share(&self, hash: &str) {
        let file = self.root.join("shared-trade.json");
        self.ok(
            "deployer",
            &[
                "otc",
                "export",
                "-n",
                "deal001",
                "--output",
                file.to_str().unwrap(),
            ],
        );
        for peer in &ROLES[..3] {
            self.ok(
                peer,
                &[
                    "otc",
                    "import",
                    "--file",
                    file.to_str().unwrap(),
                    "--terms-hash",
                    hash,
                ],
            );
        }
    }
    fn pending(&self, action: &str, mask: u8) -> Value {
        json!({"id":"18446744073709551615","dest":format!("0:{}",if action == "release" {"11".repeat(32)} else {"22".repeat(32)}),"value":"1000000001","signsRequired":"2","signsReceived":"1","confirmationsMask":mask.to_string(),"sendFlags":"3","bounce":true,"payload":"te6ccgEBAQEAAgAAAA=="})
    }
    fn events(&self) -> Vec<Value> {
        fs::read_to_string(self.state.with_extension("events"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

#[test]
fn cli_exposes_the_otc_workflow() {
    let output = Command::new(env!("CARGO_BIN_EXE_evernode-rs"))
        .args(["otc", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "OTC subcommand is missing: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8(output.stdout).unwrap();
    for action in [
        "keygen",
        "create",
        "import",
        "export",
        "propose",
        "confirm",
        "reconcile",
    ] {
        assert!(help.contains(action), "OTC action {action} is missing");
    }
}

#[test]
fn create_and_share_need_only_public_keys_and_install_no_signing_material() {
    let h = Harness::new();
    for role in ROLES {
        fs::remove_file(h.key(role)).unwrap();
    }
    let created = h.create();
    let hash = created["terms_hash"].as_str().unwrap();
    h.share(hash);
    for peer in ROLES {
        let status = h.ok(peer, &["otc", "status", "-n", "deal001"]);
        assert_eq!(status["terms_hash"], hash);
        assert_eq!(status["required_signatures"], 2);
        let entries: Vec<_> = fs::read_dir(h.directory(peer))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(entries, ["trade.json"]);
        assert!(
            !fs::read_to_string(h.directory(peer).join("trade.json"))
                .unwrap()
                .contains("secret")
        );
    }
}

#[test]
fn individual_keygen_keeps_the_seed_and_secret_off_stdout_and_refuses_overwrite() {
    let h = Harness::new();
    let key = h.root.join("buyer/new-key.json");
    let backup = h.root.join("buyer/seed.txt");
    let result = h.ok(
        "buyer",
        &[
            "otc",
            "keygen",
            "--output",
            key.to_str().unwrap(),
            "--seed-output",
            backup.to_str().unwrap(),
        ],
    );
    let actual = Keypair::derive(fs::read_to_string(&backup).unwrap().trim()).unwrap();
    assert_eq!(result["public_key"], actual.public.as_hex());
    assert!(Keypair::from_json(&fs::read(&key).unwrap()).is_ok());
    assert_eq!(
        fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before = fs::read(&key).unwrap();
    h.fails(
        "buyer",
        &[
            "otc",
            "keygen",
            "--output",
            key.to_str().unwrap(),
            "--seed-output",
            backup.to_str().unwrap(),
        ],
    );
    assert_eq!(before, fs::read(&key).unwrap());
    assert!(!h.store("buyer").exists());
}

#[test]
fn every_two_participant_pair_can_encode_release_and_refund_using_separate_keys() {
    let h = Harness::new();
    let created = h.create();
    let hash = created["terms_hash"].as_str().unwrap();
    h.share(hash);
    let chain_indices = [2, 0, 1];
    for action in ["release", "refund"] {
        for proposer in 0..3 {
            for confirmer in (0..3).filter(|i| *i != proposer) {
                h.change("transactions", json!([]));
                h.ok(
                    ROLES[proposer],
                    &[
                        "otc",
                        "propose",
                        "-n",
                        "deal001",
                        "--action",
                        action,
                        "--signer",
                        &h.key(ROLES[proposer]),
                        "--terms-hash",
                        hash,
                    ],
                );
                h.change(
                    "transactions",
                    json!([h.pending(action, 1 << chain_indices[proposer])]),
                );
                assert!(
                    h.fails(
                        ROLES[proposer],
                        &[
                            "otc",
                            "confirm",
                            "-n",
                            "deal001",
                            "--action",
                            action,
                            "--transaction-id",
                            "18446744073709551615",
                            "--signer",
                            &h.key(ROLES[proposer]),
                            "--terms-hash",
                            hash
                        ]
                    )
                    .contains("already signed")
                );
                h.ok(
                    ROLES[confirmer],
                    &[
                        "otc",
                        "confirm",
                        "-n",
                        "deal001",
                        "--action",
                        action,
                        "--transaction-id",
                        "18446744073709551615",
                        "--signer",
                        &h.key(ROLES[confirmer]),
                        "--terms-hash",
                        hash,
                    ],
                );
            }
        }
    }
    let events = h.events();
    assert!(events.iter().all(|e| e[0] != "sendfile"));
    let messages: Vec<_> = events.iter().filter(|e| e[0] == "message").collect();
    assert_eq!(messages.len(), 24);
    for message in messages {
        assert!(
            message
                .as_array()
                .unwrap()
                .windows(2)
                .any(|w| w == [json!("--signature_id"), json!("42")])
        );
    }
}

#[test]
fn confirmation_refuses_changed_destinations_amounts_payloads_and_signing_flags() {
    let h = Harness::new();
    let created = h.create();
    let hash = created["terms_hash"].as_str().unwrap();
    h.share(hash);
    for (field, value) in [
        ("dest", json!(format!("0:{}", "99".repeat(32)))),
        ("value", json!("1000000002")),
        ("sendFlags", json!("130")),
        ("bounce", json!(false)),
        ("payload", json!("nonempty")),
        ("signsRequired", json!("1")),
        ("confirmationsMask", json!("8")),
        ("signsReceived", json!("0")),
    ] {
        let mut pending = h.pending("release", 4);
        pending[field] = value;
        h.change("transactions", json!([pending]));
        h.fails(
            "seller",
            &[
                "otc",
                "confirm",
                "-n",
                "deal001",
                "--action",
                "release",
                "--transaction-id",
                "18446744073709551615",
                "--signer",
                &h.key("seller"),
                "--terms-hash",
                hash,
                "--yes",
            ],
        );
    }
    assert!(
        h.events()
            .iter()
            .all(|e| e[0] != "message" && e[0] != "sendfile")
    );
}

#[test]
fn verification_pins_network_code_and_the_three_owner_two_signature_quorum() {
    let h = Harness::new();
    h.create();
    let original = h.state();
    for (field, value) in [
        ("network_id", json!(43)),
        ("code_hash", json!("00".repeat(32))),
        ("required", json!(1)),
        (
            "owners",
            json!([
                h.public_keys[0].abi_value(),
                h.public_keys[1].abi_value(),
                h.public_keys[3].abi_value()
            ]),
        ),
    ] {
        h.set_state(original.clone());
        h.change(field, value);
        h.fails("deployer", &["otc", "verify", "-n", "deal001"]);
    }
    h.set_state(original);
    assert_eq!(
        h.ok("deployer", &["otc", "verify", "-n", "deal001"])["verified"],
        true
    );
}

#[test]
fn changed_terms_wrong_signer_and_duplicate_wallet_assignments_are_rejected() {
    let h = Harness::new();
    let created = h.create();
    let hash = created["terms_hash"].as_str().unwrap();
    h.share(hash);
    h.fails(
        "seller",
        &[
            "otc",
            "propose",
            "-n",
            "deal001",
            "--action",
            "release",
            "--signer",
            &h.key("seller"),
            "--terms-hash",
            hash,
            "--signature-id",
            "43",
        ],
    );
    let mut alias = created["trade"].clone();
    alias["terms"]["trade_id"] = json!("deal002");
    let alias: evernode_multisig::otc::Trade = serde_json::from_value(alias).unwrap();
    let alias_file = h.root.join("alias.json");
    storage::write_json(&alias_file, &alias).unwrap();
    assert!(
        h.fails(
            "deployer",
            &[
                "otc",
                "import",
                "--file",
                alias_file.to_str().unwrap(),
                "--terms-hash",
                &alias.terms_hash().unwrap()
            ]
        )
        .contains("already assigned")
    );
    h.fails(
        "buyer",
        &[
            "otc",
            "propose",
            "-n",
            "deal001",
            "--action",
            "release",
            "--signer",
            &h.key("deployer"),
            "--terms-hash",
            hash,
        ],
    );
    h.fails(
        "buyer",
        &[
            "otc",
            "propose",
            "-n",
            "deal001",
            "--action",
            "release",
            "--signer",
            &h.key("buyer"),
            "--terms-hash",
            &"0".repeat(64),
        ],
    );
    let path = h.directory("buyer").join("trade.json");
    let mut trade = created["trade"].clone();
    trade["terms"]["buyer"]["address"] = json!(format!("0:{}", "55".repeat(32)));
    fs::write(path, serde_json::to_vec(&trade).unwrap()).unwrap();
    assert!(
        h.fails(
            "buyer",
            &[
                "otc",
                "propose",
                "-n",
                "deal001",
                "--action",
                "release",
                "--signer",
                &h.key("buyer"),
                "--terms-hash",
                hash
            ]
        )
        .contains("hash differs")
    );
    h.fails(
        "deployer",
        &[
            "otc",
            "deploy",
            "-n",
            "deal001",
            "--signer",
            &h.key("buyer"),
            "--terms-hash",
            hash,
        ],
    );
    h.fails(
        "buyer",
        &[
            "otc",
            "propose",
            "-n",
            "deal001",
            "--action",
            "release",
            "--signer",
            &h.key("buyer"),
            "--terms-hash",
            hash,
            "--signature-id",
            "43",
        ],
    );
    assert!(
        h.events()
            .iter()
            .all(|e| e[0] != "message" && e[0] != "deploy_message")
    );
}

#[test]
fn trade_documents_reject_duplicate_weak_owners_invalid_amounts_and_self_payments() {
    let h = Harness::new();
    let valid = h.create()["trade"].clone();
    for (pointer, bad) in [
        ("/terms/buyer/public_key", json!(h.public_keys[1].as_hex())),
        ("/terms/arbitrator", json!("00".repeat(32))),
        (
            "/terms/deployment_key",
            json!(format!("01{}", "00".repeat(31))),
        ),
        (
            "/terms/buyer/address",
            valid["terms"]["seller"]["address"].clone(),
        ),
        ("/terms/amount", json!("0")),
        ("/terms/amount", json!("999999")),
        ("/terms/amount", json!(true)),
        ("/terms/trade_id", json!("../outside")),
        ("/schema_version", json!(2)),
        (
            "/wallet_address",
            valid["terms"]["buyer"]["address"].clone(),
        ),
    ] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<evernode_multisig::otc::Trade>(changed).is_err(),
            "Invalid terms accepted at {pointer}"
        );
    }
}

#[test]
fn real_tvm_backend_encodes_otc_setup_and_separately_signed_release_refund_confirmations() {
    let Some(executable) = std::env::var_os("EVERNODE_TEST_BACKEND") else {
        println!("Real OTC TVM codec test did not run: set EVERNODE_TEST_BACKEND");
        return;
    };
    use evernode_multisig::{
        backend::{Assets, Backend},
        domain::Contract,
        otc::SAFE_CODE_HASH,
    };
    let executable = PathBuf::from(executable).canonicalize().unwrap();
    let mut h = Harness::new();
    h.real_backend = Some(executable.clone());
    let created = h.create();
    h.change("address", created["trade"]["wallet_address"].clone());
    let hash = created["terms_hash"].as_str().unwrap();
    h.share(hash);
    h.change("acc_type", json!("Uninit"));
    let deployed = h.ok(
        "deployer",
        &[
            "otc",
            "deploy",
            "-n",
            "deal001",
            "--signer",
            &h.key("deployer"),
            "--terms-hash",
            hash,
        ],
    );
    h.change("acc_type", json!("Active"));
    let backend = Backend {
        executable,
        config: None,
        url: None,
        signature_id: None,
        timeout: std::time::Duration::from_secs(20),
    };
    let assets = Assets::new(Contract::Safe).unwrap();
    let decoded_code = backend
        .run(&[
            "decode".into(),
            "stateinit".into(),
            "--tvc".into(),
            assets.tvc.to_str().unwrap().into(),
        ])
        .unwrap();
    assert_eq!(decoded_code["code_hash"], SAFE_CODE_HASH);
    let mut encoded = vec![(deployed, "constructor")];
    for action in ["release", "refund"] {
        h.change("transactions", json!([]));
        encoded.push((
            h.ok(
                "buyer",
                &[
                    "otc",
                    "propose",
                    "-n",
                    "deal001",
                    "--action",
                    action,
                    "--signer",
                    &h.key("buyer"),
                    "--terms-hash",
                    hash,
                ],
            ),
            "submitTransaction",
        ));
        h.change("transactions", json!([h.pending(action, 4)]));
        encoded.push((
            h.ok(
                "seller",
                &[
                    "otc",
                    "confirm",
                    "-n",
                    "deal001",
                    "--action",
                    action,
                    "--transaction-id",
                    "18446744073709551615",
                    "--signer",
                    &h.key("seller"),
                    "--terms-hash",
                    hash,
                ],
            ),
            "confirmTransaction",
        ));
    }
    for (message, method) in encoded {
        let decoded = backend
            .run(&[
                "decode".into(),
                "msg".into(),
                "--abi".into(),
                assets.abi.to_str().unwrap().into(),
                message["file"].as_str().unwrap().into(),
            ])
            .unwrap();
        let text = decoded.to_string();
        assert!(
            text.contains(method),
            "OTC message failed real TVM decoding: {decoded}"
        );
        assert!(
            text.contains(&h.address()),
            "OTC message addresses another wallet"
        );
        if method == "constructor" {
            let arbitrator_decimal =
                num_bigint::BigUint::parse_bytes(h.public_keys[2].as_hex().as_bytes(), 16)
                    .unwrap()
                    .to_string();
            assert!(
                text.contains("reqConfirms")
                    && (text.contains(h.public_keys[2].as_hex())
                        || text.contains(&arbitrator_decimal)),
                "Decoded OTC owners differ: {decoded}"
            );
        }
        if method == "submitTransaction" {
            assert!(text.contains("1000000001"));
        }
    }
    assert!(h.events().iter().all(|e| e[0] != "sendfile"));
}

#[test]
fn unknown_broadcast_blocks_retries_until_exact_finalized_evidence_is_reconciled() {
    for aborted in [false, true] {
        let h = Harness::new();
        let created = h.create();
        let hash = created["terms_hash"].as_str().unwrap();
        h.share(hash);
        h.change("fail", json!("sendfile"));
        let args = [
            "otc",
            "propose",
            "-n",
            "deal001",
            "--action",
            "release",
            "--signer",
            &h.key("buyer"),
            "--terms-hash",
            hash,
            "--yes",
        ];
        h.fails("buyer", &args);
        assert!(h.directory("buyer").join("broadcast.json").exists());
        assert!(h.fails("buyer", &args).contains("unresolved"));
        h.fails("buyer", &["otc", "reconcile", "-n", "deal001"]);
        let receipt = json!({"id":"b".repeat(64),"status":3,"aborted":aborted,"account_addr":h.address(),"in_message":{"id":"a".repeat(64),"dst":h.address()},"compute":{"success":!aborted,"exit_code":if aborted {77} else {0}},"action":{"success":!aborted}});
        let mut incorrect = receipt.clone();
        incorrect["in_message"]["id"] = json!("c".repeat(64));
        h.change("receipts", json!([incorrect]));
        h.fails("buyer", &["otc", "reconcile", "-n", "deal001"]);
        h.change("receipts", json!([receipt]));
        let reconciled = h.ok("buyer", &["otc", "reconcile", "-n", "deal001"]);
        assert_eq!(
            reconciled["status"],
            if aborted { "failed" } else { "accepted" }
        );
        assert_eq!(reconciled["settlement_delivery_verified"], false);
        assert!(!h.directory("buyer").join("broadcast.json").exists());
        assert_eq!(h.events().iter().filter(|e| e[0] == "message").count(), 1);
    }
}

use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use num_bigint::BigUint;
use serde_json::{Value, json};

const PHRASES: [&str; 2] = [
    "ghost frost pool buzz rival mad naive rare shell tooth smart praise",
    "blanket time net universe ketchup maid way poem scatter blur limit drill",
];
const PUBLIC: [&str; 2] = [
    "99af508d097fa0ce97cd533a4f964ca2e21f4fb03c3f3be01d3524b7ccc09f8d",
    "c8bd66f90d61f7e1e1a6151a0dbe9d8640666920d8c0cf399cbfb72e089d2e41",
];

struct Harness {
    _temp: tempfile::TempDir,
    root: PathBuf,
    store: PathBuf,
    backend: PathBuf,
    state: PathBuf,
    real_backend: Option<PathBuf>,
}

impl Harness {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let backend = root.join("backend");
        fs::write(&backend, include_str!("fixtures/backend.py")).unwrap();
        fs::set_permissions(&backend, fs::Permissions::from_mode(0o700)).unwrap();
        let state = root.join("state.json");
        let harness = Self {
            store: root.join("wallets"),
            _temp: temp,
            root,
            backend,
            state,
            real_backend: None,
        };
        harness.set_state(json!({"address": format!("0:{}", "12".repeat(32)), "owners": PUBLIC.iter().rev().map(|key| BigUint::parse_bytes(key.as_bytes(), 16).unwrap().to_string()).collect::<Vec<_>>(), "required": 2}));
        for (i, phrase) in PHRASES.iter().enumerate() {
            harness.private_file(&harness.seed(i), phrase);
        }
        harness
    }

    fn private_file(&self, path: &Path, content: &str) {
        fs::write(path, content).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn seed(&self, i: usize) -> PathBuf {
        self.root.join(format!("seed{i}.txt"))
    }
    fn set_state(&self, state: Value) {
        fs::write(&self.state, serde_json::to_vec(&state).unwrap()).unwrap();
    }
    fn state(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.state).unwrap()).unwrap()
    }
    fn change(&self, name: &str, value: Value) {
        let mut state = self.state();
        state[name] = value;
        self.set_state(state);
    }
    fn invoke(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_evernode-rs"));
        if let Some(backend) = &self.real_backend {
            command.env("FAKE_TVM_BACKEND", backend);
        } else {
            command.env_remove("FAKE_TVM_BACKEND");
        }
        command
            .args([
                "--wallet-dir",
                self.store.to_str().unwrap(),
                "--backend",
                self.backend.to_str().unwrap(),
                "--url",
                "http://test.invalid",
            ])
            .args(args)
            .env("FAKE_BACKEND_STATE", &self.state)
            .env_remove("EVERNODE_URL")
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> Value {
        let output = self.invoke(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn fails(&self, args: &[&str]) -> String {
        let output = self.invoke(args);
        assert!(
            !output.status.success(),
            "Unexpected success: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!text.contains("PRIVATE_LEAK_SENTINEL"));
        for phrase in PHRASES {
            assert!(!text.contains(phrase));
        }
        text
    }
    fn recovery_args(&self) -> Vec<String> {
        [
            "wallet",
            "recover",
            "-n",
            "validator",
            "--custodians",
            "2",
            "--required-signatures",
            "2",
            "--address",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain([
            self.state()["address"].as_str().unwrap().to_owned(),
            "--seed-file".into(),
            self.seed(0).to_str().unwrap().to_owned(),
            "--seed-file".into(),
            self.seed(1).to_str().unwrap().to_owned(),
        ])
        .collect()
    }
    fn recover(&self) -> Value {
        let args = self.recovery_args();
        self.ok(&args.iter().map(String::as_str).collect::<Vec<_>>())
    }
    fn create(&self, contract: &str) -> Value {
        self.ok(&[
            "wallet",
            "create",
            "-n",
            "validator",
            "--custodians",
            "2",
            "--required-signatures",
            "2",
            "--contract",
            contract,
        ])
    }
    fn events(&self) -> Vec<Value> {
        fs::read_to_string(self.state.with_extension("events"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

#[test]
fn recovery_preserves_expected_address_checks_keys_and_creates_private_files() {
    let h = Harness::new();
    let result = h.recover();
    assert_eq!(result["wallet"]["address"], h.state()["address"]);
    assert_eq!(result["wallet"]["public_keys"], json!(PUBLIC));
    assert!(h.events().iter().all(|e| e[0] != "genaddr"));
    let directory = h.store.join("validator");
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in [
        "wallet.json",
        "validator.addr",
        "validator_1.keys.json",
        "MSKeys_validator/validator_seed_1.txt",
    ] {
        assert_eq!(
            fs::metadata(directory.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let metadata = fs::read_to_string(directory.join("wallet.json")).unwrap();
    assert!(!metadata.contains("secret"));
    assert!(!metadata.contains(PHRASES[0]));
    h.ok(&["wallet", "verify", "-n", "validator"]);
}

#[test]
fn failed_recovery_does_not_publish_wallet_or_install_signing_keys() {
    let h = Harness::new();
    h.change("required", json!(1));
    let args = h.recovery_args();
    assert!(
        h.fails(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .contains("threshold")
    );
    let directory = h.store.join("validator");
    assert!(!directory.join("wallet.json").exists());
    assert!(!directory.join("validator_1.keys.json").exists());
    assert!(
        directory
            .join("MSKeys_validator/validator_seed_1.txt")
            .exists()
    );
}

#[test]
fn recovery_accepts_quoted_seed_backups_written_by_the_original_script() {
    let h = Harness::new();
    for (i, phrase) in PHRASES.iter().enumerate() {
        h.private_file(&h.seed(i), &format!("\"{phrase}\"\n"));
    }
    h.recover();
    let backed_up = fs::read_to_string(
        h.store
            .join("validator/MSKeys_validator/validator_seed_1.txt"),
    )
    .unwrap();
    assert_eq!(backed_up.trim(), PHRASES[0]);
}

#[test]
fn duplicate_seeds_and_existing_wallets_are_rejected_without_overwriting() {
    let h = Harness::new();
    h.private_file(&h.seed(1), PHRASES[0]);
    let args = h.recovery_args();
    assert!(
        h.fails(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .contains("distinct")
    );
    assert!(!h.store.join("validator").exists());
    h.private_file(&h.seed(1), PHRASES[1]);
    h.recover();
    let before = fs::read(h.store.join("validator/wallet.json")).unwrap();
    h.fails(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(
        before,
        fs::read(h.store.join("validator/wallet.json")).unwrap()
    );
}

#[test]
fn submit_keeps_exact_abi_values_and_broadcasts_only_with_yes() {
    let h = Harness::new();
    h.recover();
    let dest = format!("-1:{}", "34".repeat(32));
    let out = h.root.join("transfer.boc");
    let result = h.ok(&[
        "wallet",
        "submit",
        "-n",
        "validator",
        "--dest",
        &dest,
        "--amount",
        "1.000000001",
        "--custodian",
        "2",
        "--bounce",
        "false",
        "--output",
        out.to_str().unwrap(),
    ]);
    assert_eq!(result["status"], "encoded");
    let events = h.events();
    assert!(events.iter().all(|e| e[0] != "sendfile"));
    let call = events.iter().find(|e| e[0] == "message").unwrap();
    assert_eq!(call[2], "submitTransaction");
    let params: Value = serde_json::from_str(call[3].as_str().unwrap()).unwrap();
    assert_eq!(
        params,
        json!({"dest":dest,"value":"1000000001","bounce":false,"allBalance":false,"payload":""})
    );
    assert!(call.as_array().unwrap().iter().any(|v| {
        v.as_str()
            .is_some_and(|s| s.ends_with("validator_2.keys.json"))
    }));
    assert_eq!(
        fs::metadata(out).unwrap().permissions().mode() & 0o777,
        0o600
    );
    h.ok(&[
        "wallet",
        "submit",
        "-n",
        "validator",
        "--dest",
        &dest,
        "--amount",
        "1",
        "--yes",
    ]);
    assert_eq!(h.events().iter().filter(|e| e[0] == "sendfile").count(), 1);
}

#[test]
fn confirmation_uses_chain_index_instead_of_local_key_order_and_exact_id() {
    let h = Harness::new();
    h.recover();
    let id = u64::MAX.to_string();
    h.change(
        "transactions",
        json!([{"id":id,"confirmationsMask":"1","signsRequired":"2","signsReceived":"1"}]),
    );
    assert!(
        h.fails(&[
            "wallet",
            "confirm",
            "-n",
            "validator",
            "--transaction-id",
            &id,
            "--custodian",
            "2"
        ])
        .contains("already confirmed")
    );
    h.ok(&[
        "wallet",
        "confirm",
        "-n",
        "validator",
        "--transaction-id",
        &id,
        "--custodian",
        "1",
        "--yes",
    ]);
    let events = h.events();
    let call = events.iter().find(|e| e[0] == "message").unwrap();
    assert_eq!(call[2], "confirmTransaction");
    let params: Value = serde_json::from_str(call[3].as_str().unwrap()).unwrap();
    assert_eq!(params["transactionId"], id);
    h.fails(&[
        "wallet",
        "confirm",
        "-n",
        "validator",
        "--transaction-id",
        "123",
        "--custodian",
        "1",
    ]);
}

#[test]
fn deploy_uses_constructor_owner_order_and_rejects_active_or_unfunded_accounts() {
    for contract in ["safe", "set-code"] {
        let h = Harness::new();
        let created = h.create(contract);
        assert!(
            h.fails(&["wallet", "deploy", "-n", "validator", "--yes"])
                .contains("uninitialized")
        );
        h.change("acc_type", json!("Uninit"));
        h.change("balance", json!("899999999"));
        assert!(
            h.fails(&["wallet", "deploy", "-n", "validator", "--yes"])
                .contains("0.9")
        );
        h.change("balance", json!("900000000"));
        h.ok(&[
            "wallet",
            "deploy",
            "-n",
            "validator",
            "--signature-id",
            "42",
            "--yes",
        ]);
        let events = h.events();
        let call = events.iter().find(|e| e[0] == "deploy_message").unwrap();
        let params: Value = serde_json::from_str(call[2].as_str().unwrap()).unwrap();
        assert_eq!(params["reqConfirms"], 2);
        assert_eq!(
            params["owners"][0],
            format!(
                "0x{}",
                created["wallet"]["public_keys"][0].as_str().unwrap()
            )
        );
        assert!(
            call.as_array()
                .unwrap()
                .windows(2)
                .any(|v| v == [json!("--signature_id"), json!("42")])
        );
        assert_eq!(events.iter().filter(|e| e[0] == "sendfile").count(), 1);
    }
}

#[test]
fn backend_failures_malformed_json_timeouts_and_invalid_bocs_do_not_broadcast() {
    for (field, command) in [
        ("fail", "genaddr"),
        ("malformed", "genaddr"),
        ("sleep", "genaddr"),
    ] {
        let h = Harness::new();
        h.change(field, json!(command));
        h.fails(&[
            "wallet",
            "create",
            "-n",
            "validator",
            "--custodians",
            "1",
            "--required-signatures",
            "1",
            "--timeout",
            "1",
        ]);
        assert!(!h.store.join("validator/wallet.json").exists());
    }
    let h = Harness::new();
    h.recover();
    h.change("bad_boc", json!(true));
    let dest = h.state()["address"].as_str().unwrap().to_owned();
    assert!(
        h.fails(&[
            "wallet",
            "submit",
            "-n",
            "validator",
            "--dest",
            &dest,
            "--amount",
            "1",
            "--yes"
        ])
        .contains("BOC")
    );
    assert!(h.events().iter().all(|e| e[0] != "sendfile"));
}

#[test]
fn changed_keys_permissions_symlinks_and_path_traversal_are_rejected() {
    let h = Harness::new();
    h.recover();
    let key = h.store.join("validator/validator_1.keys.json");
    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        h.fails(&["wallet", "status", "-n", "validator"])
            .contains("accessible")
    );
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
    let contents = fs::read(&key).unwrap();
    fs::remove_file(&key).unwrap();
    symlink(h.seed(0), &key).unwrap();
    assert!(
        h.fails(&["wallet", "status", "-n", "validator"])
            .contains("Symlink")
    );
    fs::remove_file(&key).unwrap();
    fs::write(&key, contents).unwrap();
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
    let other = fs::read(h.store.join("validator/validator_2.keys.json")).unwrap();
    fs::write(&key, other).unwrap();
    assert!(
        h.fails(&["wallet", "status", "-n", "validator"])
            .contains("manifest")
    );
    h.fails(&[
        "wallet",
        "create",
        "-n",
        "../escape",
        "--custodians",
        "1",
        "--required-signatures",
        "1",
    ]);
    assert!(!h.root.join("escape").exists());
}

#[test]
fn output_collision_and_recovered_deploy_do_not_broadcast() {
    let h = Harness::new();
    h.recover();
    assert!(
        h.fails(&["wallet", "deploy", "-n", "validator", "--yes"])
            .contains("Recovered")
    );
    let out = h.root.join("existing.boc");
    h.private_file(&out, "keep me");
    let dest = h.state()["address"].as_str().unwrap().to_owned();
    h.fails(&[
        "wallet",
        "submit",
        "-n",
        "validator",
        "--dest",
        &dest,
        "--amount",
        "1",
        "--output",
        out.to_str().unwrap(),
        "--yes",
    ]);
    assert_eq!(fs::read_to_string(out).unwrap(), "keep me");
    assert!(h.events().iter().all(|e| e[0] != "sendfile"));
}

#[test]
fn explicit_network_is_required_even_when_a_backend_config_exists() {
    let h = Harness::new();
    let args = h.recovery_args();
    let output = Command::new(env!("CARGO_BIN_EXE_evernode-rs"))
        .args([
            "--wallet-dir",
            h.store.to_str().unwrap(),
            "--backend",
            h.backend.to_str().unwrap(),
        ])
        .args(args)
        .env_remove("EVERNODE_URL")
        .env("FAKE_BACKEND_STATE", &h.state)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--url"));
    assert!(h.events().is_empty());
}

#[test]
fn real_backend_encodes_and_decodes_deployment_transfer_and_confirmation_offline() {
    let Some(executable) = std::env::var_os("EVERNODE_TEST_BACKEND") else {
        println!(
            "Real TVM encoding check did not run: set EVERNODE_TEST_BACKEND to an installed ever-cli executable"
        );
        return;
    };
    use evernode_multisig::{
        backend::{Assets, Backend},
        domain::Contract,
    };
    let executable = PathBuf::from(executable).canonicalize().unwrap();
    for (name, contract) in [("safe", Contract::Safe), ("set-code", Contract::SetCode)] {
        let mut h = Harness::new();
        h.real_backend = Some(executable.clone());
        let created = h.create(name);
        let address = created["wallet"]["address"].as_str().unwrap();
        h.change("address", json!(address));
        h.change(
            "owners",
            json!(
                created["wallet"]["public_keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|key| format!("0x{}", key.as_str().unwrap()))
                    .collect::<Vec<_>>()
            ),
        );
        h.change("acc_type", json!("Uninit"));
        let deployed = h.ok(&[
            "wallet",
            "deploy",
            "-n",
            "validator",
            "--signature-id",
            "42",
        ]);
        h.change("acc_type", json!("Active"));
        let submitted = h.ok(&[
            "wallet",
            "submit",
            "-n",
            "validator",
            "--dest",
            address,
            "--amount",
            "1.000000001",
            "--signature-id",
            "42",
        ]);
        h.change(
            "transactions",
            json!([{"id":"123","confirmationsMask":"1"}]),
        );
        let confirmed = h.ok(&[
            "wallet",
            "confirm",
            "-n",
            "validator",
            "--transaction-id",
            "123",
            "--custodian",
            "2",
            "--signature-id",
            "42",
        ]);
        let backend = Backend {
            executable: executable.clone(),
            config: None,
            url: None,
            signature_id: None,
            timeout: std::time::Duration::from_secs(20),
        };
        let assets = Assets::new(contract).unwrap();
        for (message, method) in [
            (deployed, "constructor"),
            (submitted, "submitTransaction"),
            (confirmed, "confirmTransaction"),
        ] {
            let decoded = backend
                .run(&[
                    "decode".into(),
                    "msg".into(),
                    "--abi".into(),
                    assets.abi.to_str().unwrap().into(),
                    message["file"].as_str().unwrap().into(),
                ])
                .unwrap();
            assert!(
                decoded.to_string().contains(method),
                "Real TVM message does not decode to {method}: {decoded}"
            );
            assert!(
                decoded.to_string().contains(address),
                "Real message has the wrong destination"
            );
        }
        assert!(h.events().iter().all(|e| e[0] != "sendfile"));
    }
}

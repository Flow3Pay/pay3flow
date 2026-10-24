use evernode_multisig::{
    crypto::Keypair,
    domain::{self, Address, Contract, PublicKey, Quorum},
};
use serde_json::{Value, json};

#[test]
fn native_key_derivation_matches_upstream_ever_cli_vectors() {
    let vectors = [
        (
            "abuse boss fly battle rubber wasp afraid hamster guide essence vibrant tattoo",
            "0cd34af58fc9cef235e1ad3aafb3d9c18c388b99c7089842eb9a49538e18d67d",
        ),
        (
            "blanket time net universe ketchup maid way poem scatter blur limit drill",
            "c8bd66f90d61f7e1e1a6151a0dbe9d8640666920d8c0cf399cbfb72e089d2e41",
        ),
        (
            "ghost frost pool buzz rival mad naive rare shell tooth smart praise",
            "99af508d097fa0ce97cd533a4f964ca2e21f4fb03c3f3be01d3524b7ccc09f8d",
        ),
    ];
    for (phrase, public) in vectors {
        let key = Keypair::derive(phrase).unwrap();
        assert_eq!(
            key.public.as_hex(),
            public,
            "BIP32/Ed25519 public key differs from upstream"
        );
    }
    let key = Keypair::derive(vectors[2].0).unwrap();
    let value = serde_json::to_value(&key).unwrap();
    assert_eq!(
        value["secret"],
        "cad7c9a658982f93ea5d6f59caf8dece9fa925738a378ce6dfdbaf477e5bb929"
    );
}

#[test]
fn key_material_rejects_malformed_mnemonics_and_mismatched_public_keys() {
    for phrase in [
        "not a phrase",
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon",
        "unit follow zone decline glare flower crisp vocal adapt magic much mesh cherry",
    ] {
        assert!(Keypair::derive(phrase).is_err());
    }
    let secret = "00".repeat(32);
    assert!(
        Keypair::from_json(
            json!({"public": "01".repeat(32), "secret": secret})
                .to_string()
                .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn token_parsing_preserves_nanotokens_and_uint128_limits() {
    for (input, expected) in [
        ("0.000000001", 1),
        ("1.000000001", 1_000_000_001),
        ("9007199254740993", 9_007_199_254_740_993_000_000_000),
        ("340282366920938463463374607431.768211455", u128::MAX),
    ] {
        assert_eq!(domain::nanotokens(input).unwrap(), expected, "{input}");
    }
    for input in [
        "0",
        "0.000000000",
        "-1",
        "1e9",
        "NaN",
        "0.0000000001",
        "1.1.1",
        ".1",
        "340282366920938463463374607431.768211456",
        "340282366920938463463374607432",
    ] {
        assert!(domain::nanotokens(input).is_err(), "{input}");
    }
    let dest: Address = format!("-1:{}", "ab".repeat(32)).parse().unwrap();
    let transfer = domain::transfer(&dest, u128::MAX, false, "");
    assert_eq!(transfer["value"], u128::MAX.to_string());
    assert_eq!(transfer["allBalance"], false);
    assert_eq!(
        domain::confirmation(u64::MAX).unwrap()["transactionId"],
        u64::MAX.to_string()
    );
    assert!(domain::confirmation(0).is_err());
}

#[test]
fn abi_assets_match_vendored_contracts_and_method_shapes() {
    use sha2::{Digest, Sha256};
    // SHA-256 of the bundled upstream ABI/TVC bytes, pinned independently.
    for (contract, abi_hash, tvc_hash) in [
        (
            Contract::Safe,
            "bbd2b8605aa795623b1ace322f0416638c1656c99355873d0460dafd9de5f891",
            "c08b1b9f8949b087ef79f53c7f18ef7e9bae71b38b2550ef1119f132105b2d5f",
        ),
        (
            Contract::SetCode,
            "28814c8f32d728133e975bdf6bf3d84346f4a5fca32b596d066539a14e4f0bb9",
            "66c36b99e8ba8fdb0bbf9488d5168ca44eff14e109391962edf62d4d6756a455",
        ),
    ] {
        let (abi, tvc) = contract.assets();
        assert_eq!(hex::encode(Sha256::digest(abi.as_bytes())), abi_hash);
        assert_eq!(hex::encode(Sha256::digest(tvc)), tvc_hash);
        let abi: Value = serde_json::from_str(abi).unwrap();
        let owners = [
            PublicKey::from_hex("1").unwrap(),
            PublicKey::from_hex("2").unwrap(),
        ];
        let constructor = domain::constructor(&owners, Quorum::new(2, 2).unwrap()).unwrap();
        assert_eq!(constructor["owners"][0], format!("0x{:064x}", 1));
        for method in [
            "constructor",
            "submitTransaction",
            "confirmTransaction",
            "getCustodians",
            "getParameters",
            "getTransactions",
        ] {
            assert!(
                abi["functions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["name"] == method)
            );
        }
    }
}

#[test]
fn manifest_deserialization_enforces_domain_invariants() {
    let valid = json!({"schema_version":1,"name":"validator","mode":"new","contract":"safe","address":format!("0:{}", "00".repeat(32)),"quorum":{"custodians":1,"required":1},"public_keys":["1"]});
    assert!(serde_json::from_value::<evernode_multisig::wallet::Manifest>(valid.clone()).is_ok());
    for (pointer, bad) in [
        ("/schema_version", json!(2)),
        ("/name", json!("../escape")),
        ("/quorum/required", json!(0)),
        ("/quorum/custodians", json!(32)),
        ("/quorum/custodians", json!(true)),
        ("/public_keys", json!(["1", "1"])),
        ("/address", json!("invalid")),
    ] {
        let mut value = valid.clone();
        *value.pointer_mut(pointer).unwrap() = bad;
        assert!(
            serde_json::from_value::<evernode_multisig::wallet::Manifest>(value).is_err(),
            "{pointer}"
        );
    }
}

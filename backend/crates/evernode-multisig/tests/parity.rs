use evernode_multisig::domain::{PublicKey, Quorum, verify_owners};
use serde_json::{Value, json};

fn quorum(case: &Value, count: &str, required: &str) -> anyhow::Result<Quorum> {
    Quorum::new(
        case[count]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Invalid count"))?,
        case[required]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Invalid threshold"))?,
    )
}

#[test]
fn matches_recorded_python_wallet_behavior() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("fixtures/parity.json")).unwrap();
    for (index, case) in cases.iter().enumerate() {
        let actual = match case["kind"].as_str().unwrap() {
            "quorum" => json!(quorum(case, "custodians", "required").is_ok()),
            "chain_key" => PublicKey::from_chain(&case["input"])
                .map(|key| json!(key.as_hex()))
                .unwrap_or(json!(false)),
            "owners" => {
                let verify = || -> anyhow::Result<()> {
                    let expected = quorum(case, "custodians", "required")?;
                    let local = case["local"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| PublicKey::from_hex(key.as_str().unwrap()))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let remote = case["remote"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| PublicKey::from_hex(key.as_str().unwrap()))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let observed = Quorum::new(
                        remote.len() as u64,
                        case["observed_required"].as_u64().unwrap(),
                    )?;
                    verify_owners(&local, expected, &remote, observed)
                };
                json!(verify().is_ok())
            }
            _ => panic!("Unknown corpus kind"),
        };
        assert_eq!(
            actual, case["expected"],
            "Differential case {index}: {case}"
        );
    }
    println!(
        "Rust/Python parity: {} cases compared, 0 differences",
        cases.len()
    );
}

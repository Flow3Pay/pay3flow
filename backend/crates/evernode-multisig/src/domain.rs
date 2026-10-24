use std::{collections::BTreeSet, fmt, str::FromStr};

use anyhow::{Context, Result, bail, ensure};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Address(String);

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let (wc, hash) = value
            .split_once(':')
            .context("Expected workchain:64_HEX address")?;
        let digits = wc.strip_prefix('-').unwrap_or(wc);
        ensure!(
            !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()),
            "Invalid workchain"
        );
        let wc: i32 = wc.parse().context("Workchain is outside the int32 range")?;
        ensure!(
            hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
            "Expected a 64-digit address hash"
        );
        Ok(Self(format!("{wc}:{}", hash.to_ascii_lowercase())))
    }
}

impl Address {
    pub fn workchain(&self) -> Result<i8> {
        self.0
            .split_once(':')
            .context("Missing workchain")?
            .0
            .parse()
            .context("Backend requires an int8 workchain")
    }
}

impl TryFrom<String> for Address {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}
impl From<Address> for String {
    fn from(value: Address) -> Self {
        value.0
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PublicKey(String);

impl PublicKey {
    pub fn from_hex(value: &str) -> Result<Self> {
        let value = value.strip_prefix("0x").unwrap_or(value);
        ensure!(
            !value.is_empty() && value.len() <= 64 && value.bytes().all(|c| c.is_ascii_hexdigit()),
            "Invalid uint256 public key"
        );
        Ok(Self(format!("{value:0>64}").to_ascii_lowercase()))
    }

    pub fn from_chain(value: &Value) -> Result<Self> {
        let raw = match value {
            Value::String(s) => s.to_owned(),
            Value::Number(n) if n.is_u64() => n.to_string(),
            _ => bail!("Invalid on-chain public key"),
        };
        let (digits, radix) = if raw.to_ascii_lowercase().starts_with("0x") {
            (&raw[2..], 16)
        } else {
            (raw.as_str(), 10)
        };
        ensure!(
            !digits.is_empty()
                && digits.bytes().all(|c| if radix == 16 {
                    c.is_ascii_hexdigit()
                } else {
                    c.is_ascii_digit()
                }),
            "Invalid on-chain public key"
        );
        let number = BigUint::parse_bytes(digits.as_bytes(), radix)
            .context("Invalid on-chain public key")?;
        ensure!(number.bits() <= 256, "Public key exceeds uint256");
        Self::from_hex(&format!("{number:064x}"))
    }

    pub fn as_hex(&self) -> &str {
        &self.0
    }
    pub fn abi_value(&self) -> String {
        format!("0x{}", self.0)
    }
}

impl TryFrom<String> for PublicKey {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        Self::from_hex(&value)
    }
}
impl FromStr for PublicKey {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        Self::from_hex(value)
    }
}
impl From<PublicKey> for String {
    fn from(value: PublicKey) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawQuorum")]
pub struct Quorum {
    custodians: u8,
    required: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuorum {
    custodians: u64,
    required: u64,
}

impl TryFrom<RawQuorum> for Quorum {
    type Error = anyhow::Error;
    fn try_from(raw: RawQuorum) -> Result<Self> {
        Self::new(raw.custodians, raw.required)
    }
}

impl Quorum {
    pub fn new(custodians: u64, required: u64) -> Result<Self> {
        ensure!(
            (1..=31).contains(&custodians),
            "Custodian count must be between 1 and 31"
        );
        ensure!(
            (1..=custodians).contains(&required),
            "Required signatures must be between 1 and the custodian count"
        );
        Ok(Self {
            custodians: custodians as u8,
            required: required as u8,
        })
    }
    pub fn custodians(self) -> u8 {
        self.custodians
    }
    pub fn required(self) -> u8 {
        self.required
    }
}

pub fn verify_owners(
    local: &[PublicKey],
    expected: Quorum,
    remote: &[PublicKey],
    observed: Quorum,
) -> Result<()> {
    ensure!(
        expected == observed,
        "Imported wallet custodians or signature threshold do not match the supplied profile"
    );
    let local: BTreeSet<_> = local.iter().collect();
    let remote: BTreeSet<_> = remote.iter().collect();
    ensure!(
        local.len() == usize::from(expected.custodians()) && local == remote,
        "Imported seed public keys do not match the on-chain wallet custodians"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Contract {
    Safe,
    SetCode,
}

impl Contract {
    pub fn assets(self) -> (&'static str, &'static [u8]) {
        match self {
            Self::Safe => (
                include_str!("../assets/SafeMultisigWallet.abi.json"),
                include_bytes!("../assets/SafeMultisigWallet.tvc"),
            ),
            Self::SetCode => (
                include_str!("../assets/SetcodeMultisigWallet.abi.json"),
                include_bytes!("../assets/SetcodeMultisigWallet.tvc"),
            ),
        }
    }
}

pub fn constructor(owners: &[PublicKey], quorum: Quorum) -> Result<Value> {
    ensure!(
        owners.len() == usize::from(quorum.custodians())
            && owners.iter().collect::<BTreeSet<_>>().len() == owners.len(),
        "Expected distinct custodian keys matching the quorum"
    );
    Ok(
        json!({"owners": owners.iter().map(PublicKey::abi_value).collect::<Vec<_>>(), "reqConfirms": quorum.required()}),
    )
}

/// Parses EVER with up to nine decimal places without a floating point step.
pub fn nanotokens(value: &str) -> Result<u128> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    ensure!(
        !whole.is_empty()
            && whole.bytes().all(|c| c.is_ascii_digit())
            && fraction.len() <= 9
            && fraction.bytes().all(|c| c.is_ascii_digit()),
        "Use a positive decimal amount with at most 9 fractional digits"
    );
    let whole: u128 = whole.parse().context("Token amount exceeds uint128")?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        format!("{fraction:0<9}").parse::<u128>()?
    };
    let amount = whole
        .checked_mul(1_000_000_000)
        .and_then(|n| n.checked_add(fraction))
        .context("Token amount exceeds uint128")?;
    ensure!(amount > 0, "Amount must be positive");
    Ok(amount)
}

pub fn transfer(dest: &Address, amount: u128, bounce: bool, payload: &str) -> Value {
    // ABI integer strings avoid JSON number precision loss beyond 2^53.
    json!({"dest": dest, "value": amount.to_string(), "bounce": bounce, "allBalance": false, "payload": payload})
}

pub fn confirmation(id: u64) -> Result<Value> {
    ensure!(id != 0, "Transaction ID must be nonzero");
    Ok(json!({"transactionId": id.to_string()}))
}

pub fn abi_u64(value: &Value) -> Result<u64> {
    match value {
        Value::String(s) => s.parse().context("Expected uint64 ABI integer"),
        Value::Number(n) => n.as_u64().context("Expected uint64 ABI integer"),
        _ => bail!("Expected uint64 ABI integer"),
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
        "Name must contain 1..64 letters, digits, hyphens or underscores and start with a letter or digit"
    );
    Ok(())
}

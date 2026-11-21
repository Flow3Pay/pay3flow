//! Aptos key material and fungible-asset transfers (spec §5.1.1).
//!
//! `aptos-sdk` version 0.8 unconditionally enables the
//! `serde_json/arbitrary_precision` feature, which changes how `serde_json`
//! parses numbers and breaks the BestChange provider's untagged number enum.
//! The transaction layout here is therefore encoded with `aptos-bcs`, the same
//! BCS implementation the SDK uses, and pinned by an SDK-produced golden vector
//! in the tests.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
use sha3::{Digest, Sha3_256};

/// A fungible-asset transfer from the temp wallet to a payee main account.
pub(crate) struct FaTransfer {
    /// Address of the asset's `0x1::fungible_asset::Metadata` object.
    pub metadata: String,
    pub recipient: String,
    pub amount: u64,
    /// Account sequence number read from the chain.
    pub sequence_number: u64,
    pub gas_unit_price: u64,
    pub max_gas_amount: u64,
    pub expiration_secs: u64,
}

/// Aptos addresses are exactly 32 bytes on the wire.
type AccountAddress = [u8; 32];

impl WalletMaterial {
    pub(crate) fn sign_aptos_fa_transfer(&self, transfer: &FaTransfer) -> Result<Vec<u8>> {
        if self.chain() != super::Chain::Aptos {
            return Err(invalid(
                "fungible-asset transfers require an Aptos temporary wallet",
            ));
        }
        if transfer.amount == 0 {
            return Err(invalid("Aptos transfer amount must be positive"));
        }
        let metadata = parse_address(&transfer.metadata)?;
        let recipient = parse_address(&transfer.recipient)?;
        let key = self.ed25519_secret()?;
        let key = SigningKey::from_bytes(&key);
        let sender = account_address(&key.verifying_key().to_bytes());
        let raw = RawTransaction {
            sender,
            sequence_number: transfer.sequence_number,
            payload: TransactionPayload::EntryFunction(EntryFunction {
                module: ModuleId {
                    address: parse_address("0x1")?,
                    name: "primary_fungible_store".to_owned(),
                },
                function: "transfer".to_owned(),
                ty_args: vec![TypeTag::Struct(StructTag {
                    address: parse_address("0x1")?,
                    module: "fungible_asset".to_owned(),
                    name: "Metadata".to_owned(),
                    type_args: Vec::new(),
                })],
                args: vec![
                    aptos_bcs::to_bytes(&metadata)
                        .map_err(|_| invalid("Aptos transfer payload could not be built"))?,
                    aptos_bcs::to_bytes(&recipient)
                        .map_err(|_| invalid("Aptos transfer payload could not be built"))?,
                    aptos_bcs::to_bytes(&transfer.amount)
                        .map_err(|_| invalid("Aptos transfer payload could not be built"))?,
                ],
            }),
            max_gas_amount: transfer.max_gas_amount,
            gas_unit_price: transfer.gas_unit_price,
            expiration_timestamp_secs: transfer.expiration_secs,
            chain_id: MAINNET_CHAIN_ID,
        };
        let mut message = Sha3_256::digest(SIGNING_PREFIX).to_vec();
        message.extend(
            aptos_bcs::to_bytes(&raw)
                .map_err(|_| invalid("Aptos transaction could not be serialized"))?,
        );
        let signature = key.sign(&message);
        let signed = SignedTransaction {
            raw_txn: raw,
            authenticator: TransactionAuthenticator::Ed25519 {
                public_key: key.verifying_key().to_bytes().to_vec(),
                signature: signature.to_bytes().to_vec(),
            },
        };
        aptos_bcs::to_bytes(&signed)
            .map_err(|_| invalid("Aptos transaction could not be serialized"))
    }
}

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| invalid("Aptos public key has the wrong length"))?;
    Ok(format!("0x{}", hex(&account_address(&public_key))))
}

/// The Aptos authentication key is the SHA3-256 of scheme || public key.
fn account_address(public_key: &[u8; 32]) -> AccountAddress {
    let mut hasher = Sha3_256::new();
    hasher.update(public_key);
    hasher.update([0u8]);
    hasher.finalize().into()
}

fn parse_address(value: &str) -> Result<AccountAddress> {
    let trimmed = value.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if digits.is_empty()
        || digits.len() > 64
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid("Aptos address is invalid"));
    }
    let mut bytes = [0u8; 32];
    // Addresses are written big-endian and left-padded with zeroes.
    for (index, chunk) in digits.as_bytes().rchunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| invalid("Aptos address is invalid"))?;
        bytes[31 - index] =
            u8::from_str_radix(text, 16).map_err(|_| invalid("Aptos address is invalid"))?;
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

const SIGNING_PREFIX: &[u8] = b"APTOS::RawTransaction";
const MAINNET_CHAIN_ID: u8 = 1;

#[derive(Serialize)]
struct RawTransaction {
    sender: AccountAddress,
    sequence_number: u64,
    payload: TransactionPayload,
    max_gas_amount: u64,
    gas_unit_price: u64,
    expiration_timestamp_secs: u64,
    chain_id: u8,
}

#[derive(Serialize)]
struct SignedTransaction {
    raw_txn: RawTransaction,
    authenticator: TransactionAuthenticator,
}

#[allow(dead_code)]
#[derive(Serialize)]
enum TransactionPayload {
    Script(Script),
    ModuleBundle(ModuleBundle),
    EntryFunction(EntryFunction),
}

#[allow(dead_code)]
#[derive(Serialize)]
struct Script {
    code: Vec<u8>,
    ty_args: Vec<TypeTag>,
    args: Vec<Vec<u8>>,
}

#[allow(dead_code)]
#[derive(Serialize)]
struct ModuleBundle;

#[derive(Serialize)]
struct EntryFunction {
    module: ModuleId,
    function: String,
    ty_args: Vec<TypeTag>,
    args: Vec<Vec<u8>>,
}

#[derive(Serialize)]
struct ModuleId {
    address: AccountAddress,
    name: String,
}

#[allow(dead_code)]
#[derive(Serialize)]
enum TypeTag {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<TypeTag>),
    Struct(StructTag),
}

#[derive(Serialize)]
struct StructTag {
    address: AccountAddress,
    module: String,
    name: String,
    type_args: Vec<TypeTag>,
}

#[allow(dead_code)]
#[derive(Serialize)]
enum TransactionAuthenticator {
    Ed25519 {
        public_key: Vec<u8>,
        signature: Vec<u8>,
    },
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;
    use ed25519_dalek::Verifier;

    /// Signed transaction produced by `aptos-sdk` 0.8 for this key and payload.
    const GOLDEN_SIGNED: &str = "bc5098814f4d54a0d175ec70a72d9a95910f6fae0decfd34bf24d7c5a2341ace0300000000000000020000000000000000000000000000000000000000000000000000000000000001167072696d6172795f66756e6769626c655f73746f7265087472616e73666572010700000000000000000000000000000000000000000000000000000000000000010e66756e6769626c655f6173736574084d657461646174610003201111111111111111111111111111111111111111111111111111111111111111202222222222222222222222222222222222222222222222222222222222222222084081ba0100000000204e000000000000640000000000000000d2496b000000000100206aed022ad63d599e6a2962202bb29ae9258b53789b58d1d097d1d91cb6ffbc3d401fe27199491d6ac76069040b6c0b89408fff42fdbabc3fc3fb635d3c759dd3ff8107887636f478a5ab1af8cba6b9bbfa2e9cf06f79241ada9f5cafd1aa54840d";

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Aptos, &[7u8; 32], 0).expect("material")
    }

    fn transfer() -> FaTransfer {
        FaTransfer {
            metadata: format!("0x{}", "11".repeat(32)),
            recipient: format!("0x{}", "22".repeat(32)),
            amount: 29_000_000,
            sequence_number: 3,
            gas_unit_price: 100,
            max_gas_amount: 20_000,
            expiration_secs: 1_800_000_000,
        }
    }

    #[test]
    fn fungible_asset_transfer_matches_the_sdk_golden_vector() {
        let bytes = material().sign_aptos_fa_transfer(&transfer()).unwrap();
        assert_eq!(hex(&bytes), GOLDEN_SIGNED);
    }

    #[test]
    fn signed_transfer_carries_the_wallet_sender_and_a_valid_signature() {
        let material = material();
        let bytes = material.sign_aptos_fa_transfer(&transfer()).unwrap();
        let sender = parse_address(&material.address().unwrap()).unwrap();
        assert_eq!(&bytes[..32], &sender);
        assert_eq!(bytes[32..40], 3u64.to_le_bytes());
        assert_eq!(bytes[40], 2, "EntryFunction payload variant");

        let raw_end = bytes.len() - 99;
        assert_eq!(bytes[raw_end], 0, "Ed25519 authenticator variant");
        assert_eq!(bytes[raw_end + 1], 0x20, "public key length prefix");
        assert_eq!(bytes[bytes.len() - 65], 0x40, "signature length prefix");
        let public_key = &bytes[raw_end + 2..raw_end + 34];
        let signature = &bytes[bytes.len() - 64..];
        let mut message = Sha3_256::digest(SIGNING_PREFIX).to_vec();
        message.extend(&bytes[..raw_end]);
        ed25519_dalek::VerifyingKey::from_bytes(public_key.try_into().unwrap())
            .unwrap()
            .verify(
                &message,
                &ed25519_dalek::Signature::from_slice(signature).unwrap(),
            )
            .expect("Aptos signature verifies");
    }

    #[test]
    fn allocation_rejects_other_chains_and_bad_addresses() {
        assert!(WalletMaterial::from_seed(Chain::Solana, &[7u8; 32], 0)
            .unwrap()
            .sign_aptos_fa_transfer(&transfer())
            .is_err());
        let mut bad = transfer();
        bad.recipient = "0xnot-hex".to_owned();
        assert!(material().sign_aptos_fa_transfer(&bad).is_err());
        let mut zero = transfer();
        zero.amount = 0;
        assert!(material().sign_aptos_fa_transfer(&zero).is_err());
    }

    #[test]
    fn addresses_are_left_padded_to_32_bytes() {
        assert_eq!(parse_address("0x1").unwrap()[31], 1);
        assert_eq!(&parse_address("0x1").unwrap()[..31], &[0u8; 31]);
        assert_eq!(
            parse_address(&format!("0x{}", "ab".repeat(32))).unwrap(),
            [0xab; 32]
        );
        assert!(parse_address("0x").is_err());
        assert!(parse_address(&"a".repeat(65)).is_err());
    }
}

//! Address derivation for the immutable, single-key Broxus EVER Wallet.
//!
//! A signature by its initial key proves ownership even before the first
//! transaction deploys the wallet. This proves identity, never funds or receipt.
//! Reference: broxus/everscale-standalone-client, AccountsStorage/EverWallet.ts.
use super::error::{invalid, Result};
use ed25519_dalek::{Signer, SigningKey};
use everscale_types::{
    boc::Boc,
    cell::{Cell, CellBuilder},
    models::StateInit,
};
use sha2::{Digest, Sha256};

/// ABI 2.0 signature of the EverWallet transfer function.
const SEND_TRANSACTION: &str = "sendTransaction(address,uint128,bool,uint8,cell)()v2";

pub(crate) const CODE_HASH: &str =
    "3ba6528ab2694c118180aa3bd10dd19ff400b909ab4dcf58fc69925b2c7b12a6";
const CODE: &str = "te6cckEBBgEA/AABFP8A9KQT9LzyyAsBAgEgAgMABNIwAubycdcBAcAA8nqDCNcY7UTQgwfXAdcLP8j4KM8WI88WyfkAA3HXAQHDAJqDB9cBURO68uBk3oBA1wGAINcBgCDXAVQWdfkQ8qj4I7vyeWa++COBBwiggQPoqFIgvLHydAIgghBM7mRsuuMPAcjL/8s/ye1UBAUAmDAC10zQ+kCDBtcBcdcBeNcB10z4AHCAEASqAhSxyMsFUAXPFlAD+gLLaSLQIc8xIddJoIQJuZgzcAHLAFjPFpcwcQHLABLM4skB+wAAPoIQFp4+EbqOEfgAApMg10qXeNcB1AL7AOjRkzLyPOI+zYS/";

pub(crate) fn approved(hashes: &[String]) -> bool {
    hashes
        .iter()
        .any(|hash| hash.eq_ignore_ascii_case(CODE_HASH))
}

/// StateInit of the pinned single-key EverWallet for `key`.
fn state_init(key: &[u8; 32]) -> Result<Cell> {
    let code = Boc::decode_base64(CODE).map_err(|_| invalid("invalid pinned EverWallet code"))?;
    if code.repr_hash().to_string() != CODE_HASH {
        return Err(invalid("EverWallet code hash mismatch"));
    }
    let mut data = CellBuilder::new();
    data.store_raw(key, 256)
        .map_err(|_| invalid("invalid EverWallet initial data"))?;
    data.store_u64(0)
        .map_err(|_| invalid("invalid EverWallet initial data"))?;
    let init = StateInit {
        code: Some(code),
        data: Some(
            data.build()
                .map_err(|_| invalid("invalid EverWallet data cell"))?,
        ),
        ..StateInit::default()
    };
    CellBuilder::build_from(init).map_err(|_| invalid("invalid EverWallet initial state"))
}

pub(crate) fn address(key: &[u8; 32]) -> Result<String> {
    Ok(format!("0:{}", state_init(key)?.repr_hash()))
}

/// `sendTransaction(address,uint128,bool,uint8,cell)()v2` function id: the
/// first 32 bits of the signature hash with the external-call high bit clear.
fn function_id() -> [u8; 4] {
    let digest = Sha256::digest(SEND_TRANSACTION.as_bytes());
    let mut id = [0u8; 4];
    id.copy_from_slice(&digest[..4]);
    id[0] &= 0x7f;
    id
}

/// Parses `workchain:hex` into a workchain id and a 32-byte address.
fn parse_address(value: &str) -> Result<(i8, [u8; 32])> {
    let (workchain, hash) = value
        .trim()
        .split_once(':')
        .ok_or_else(|| invalid("EVER recipient must be written as workchain:hex"))?;
    let workchain: i8 = workchain
        .parse()
        .map_err(|_| invalid("invalid EVER workchain"))?;
    let hash: [u8; 32] = super::wallet::unhex(hash)?
        .as_slice()
        .try_into()
        .map_err(|_| invalid("invalid EVER recipient address"))?;
    Ok((workchain, hash))
}

fn empty_cell() -> Result<Cell> {
    CellBuilder::new()
        .build()
        .map_err(|_| invalid("EVER message body could not be built"))
}

/// Builds and signs an EverWallet `sendTransaction` external inbound message.
///
/// Returns the message hash (the identity Everscale assigns to the transaction
/// it produces) and the base64 BOC the GraphQL endpoint accepts. The signature
/// covers the ABI 2.0 body without the 513-bit signature prefix, so the
/// contract can verify it after skipping that prefix.
pub(crate) fn transfer(
    key: &[u8; 32],
    to: &str,
    amount: u128,
    now_ms: u64,
    expire: u32,
) -> Result<(String, String)> {
    if amount == 0 {
        return Err(invalid("EVER transfer amount must be positive"));
    }
    let (workchain, address) = parse_address(to)?;
    let id = function_id();

    // Function arguments overflow the root cell after the 513-bit signature
    // prefix, so ABI 2.0 continues them in the first reference.
    let mut args = CellBuilder::new();
    args.store_uint(0b10, 2)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_bit(false)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_u8(workchain as u8)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_raw(&address, 256)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_u128(amount)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_bit(false)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_u8(3)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    args.store_reference(empty_cell()?)
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;
    let args = args
        .build()
        .map_err(|_| invalid("EVER transfer arguments could not be built"))?;

    let mut unsigned = CellBuilder::new();
    unsigned
        .store_bit(true)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    unsigned
        .store_raw(key, 256)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    unsigned
        .store_u64(now_ms)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    unsigned
        .store_u32(expire)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    unsigned
        .store_raw(&id, 32)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    unsigned
        .store_reference(args.clone())
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    let unsigned = unsigned
        .build()
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    let signature = SigningKey::from_bytes(key).sign(unsigned.repr_hash().as_ref());

    let mut body = CellBuilder::new();
    body.store_bit(true)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_raw(&signature.to_bytes(), 512)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_bit(true)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_raw(key, 256)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_u64(now_ms)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_u32(expire)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_raw(&id, 32)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    body.store_reference(args)
        .map_err(|_| invalid("EVER transfer body could not be built"))?;
    let body = body
        .build()
        .map_err(|_| invalid("EVER transfer body could not be built"))?;

    // ext_in_msg_info$10 src:addr_none dest:addr_std init:^StateInit body:^Body
    let mut message = CellBuilder::new();
    message
        .store_uint(0b10, 2)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_uint(0, 2)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_uint(0b10, 2)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_bit(false)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_u8(workchain as u8)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_raw(&address, 256)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_uint(0, 4)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_bit(true)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_bit(true)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_reference(state_init(key)?)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_bit(true)
        .map_err(|_| invalid("EVER message could not be built"))?;
    message
        .store_reference(body)
        .map_err(|_| invalid("EVER message could not be built"))?;
    let message = message
        .build()
        .map_err(|_| invalid("EVER message could not be built"))?;
    Ok((
        message.repr_hash().to_string(),
        Boc::encode_base64(&message),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_keys_cannot_claim_the_same_wallet() {
        // Independently hash the TVM StateInit: refs=(code depth 3, data depth 0),
        // root bits 00110; data=uint256 key followed by uint64 zero timestamp.
        assert_eq!(
            address(&[1; 32]).unwrap(),
            "0:fd6341394ece1a15b12b18cf6cbc68988fd3b2024261b9256d6be88cf0e2d597"
        );
        assert_ne!(address(&[1; 32]).unwrap(), address(&[2; 32]).unwrap());
        assert!(address(&[1; 32]).unwrap().starts_with("0:"));
        assert!(!approved(&[]));
        assert!(!approved(&["arbitrary-code".into()]));
        assert!(approved(&[CODE_HASH.to_uppercase()]));
    }

    #[test]
    fn transfer_builds_a_deterministic_signed_boc_for_the_derived_wallet() {
        let key = [7u8; 32];
        let to = format!("0:{}", "11".repeat(32));
        let (hash, boc) =
            transfer(&key, &to, 29_000_000_000, 1_760_000_000_000, 1_760_000_600).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let root = Boc::decode_base64(&boc).expect("BOC decodes");
        assert_eq!(root.repr_hash().to_string(), hash);
        let (again, _) =
            transfer(&key, &to, 29_000_000_000, 1_760_000_000_000, 1_760_000_600).unwrap();
        assert_eq!(again, hash);
        let (other, _) = transfer(
            &[8u8; 32],
            &to,
            29_000_000_000,
            1_760_000_000_000,
            1_760_000_600,
        )
        .unwrap();
        assert_ne!(other, hash);
        assert!(transfer(&key, "not-an-address", 1, 1, 1).is_err());
        assert!(transfer(&key, &to, 0, 1, 1).is_err());
    }
}

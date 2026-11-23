//! TRON key material and TRC-20 transfers built with `anychain-tron`
//! (spec §5.1.1): Base58Check addresses, the protobuf contract and the raw
//! transaction all come from the SDK.
use crate::otc::error::{invalid, Result};
use anychain_core::Address;
use anychain_tron::{TronAddress, TronFormat, TronPublicKey};
use std::str::FromStr;

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    if public_key.len() != 64 {
        return Err(invalid("TRON public key has the wrong length"));
    }
    let encoded = format!("04{}", super::super::wallet::hex(public_key));
    let key = TronPublicKey::from_str(&encoded)
        .map_err(|_| invalid("temporary wallet TRON key is invalid"))?;
    let address = <TronAddress as Address>::from_public_key(&key, &TronFormat::Standard)
        .map_err(|_| invalid("temporary wallet TRON address could not be derived"))?;
    Ok(address.to_string())
}

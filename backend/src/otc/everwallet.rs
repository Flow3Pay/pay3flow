//! Address derivation for the immutable, single-key Broxus EVER Wallet.
//!
//! A signature by its initial key proves ownership even before the first
//! transaction deploys the wallet. This proves identity, never funds or receipt.
//! Reference: broxus/everscale-standalone-client, AccountsStorage/EverWallet.ts.
use super::error::{invalid, Result};
use everscale_types::{boc::Boc, cell::CellBuilder, models::StateInit};

pub(crate) const CODE_HASH: &str =
    "3ba6528ab2694c118180aa3bd10dd19ff400b909ab4dcf58fc69925b2c7b12a6";
const CODE: &str = "te6cckEBBgEA/AABFP8A9KQT9LzyyAsBAgEgAgMABNIwAubycdcBAcAA8nqDCNcY7UTQgwfXAdcLP8j4KM8WI88WyfkAA3HXAQHDAJqDB9cBURO68uBk3oBA1wGAINcBgCDXAVQWdfkQ8qj4I7vyeWa++COBBwiggQPoqFIgvLHydAIgghBM7mRsuuMPAcjL/8s/ye1UBAUAmDAC10zQ+kCDBtcBcdcBeNcB10z4AHCAEASqAhSxyMsFUAXPFlAD+gLLaSLQIc8xIddJoIQJuZgzcAHLAFjPFpcwcQHLABLM4skB+wAAPoIQFp4+EbqOEfgAApMg10qXeNcB1AL7AOjRkzLyPOI+zYS/";

pub(crate) fn approved(hashes: &[String]) -> bool {
    hashes
        .iter()
        .any(|hash| hash.eq_ignore_ascii_case(CODE_HASH))
}

pub(crate) fn address(key: &[u8; 32]) -> Result<String> {
    let code = Boc::decode_base64(CODE).map_err(|_| invalid("invalid pinned EverWallet code"))?;
    if code.repr_hash().to_string() != CODE_HASH {
        return Err(invalid("EverWallet code hash mismatch"));
    }
    let mut data = CellBuilder::new();
    data.store_raw(key, 256)
        .and_then(|_| data.store_u64(0))
        .map_err(|_| invalid("invalid EverWallet initial data"))?;
    let init = StateInit {
        code: Some(code),
        data: Some(
            data.build()
                .map_err(|_| invalid("invalid EverWallet data cell"))?,
        ),
        ..StateInit::default()
    };
    let cell =
        CellBuilder::build_from(init).map_err(|_| invalid("invalid EverWallet initial state"))?;
    Ok(format!("0:{}", cell.repr_hash()))
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
}

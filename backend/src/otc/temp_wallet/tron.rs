//! TRON key material and TRC-20 transfers built with `anychain-tron`
//! (spec §5.1.1): Base58Check addresses, the protobuf contract and the raw
//! transaction all come from the SDK.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use anychain_core::{Address, Transaction};
use anychain_tron::{
    trx, TronAddress, TronFormat, TronPublicKey, TronTransaction, TronTransactionParameters,
};
use sha2::{Digest, Sha256};
use std::str::FromStr;

pub(crate) struct Trc20Transfer {
    /// TRC-20 token contract, Base58Check.
    pub contract: String,
    /// Payee address, Base58Check.
    pub recipient: String,
    pub amount: u128,
    /// Reference block fields read from the chain.
    pub ref_block_number: i64,
    pub ref_block_hash: String,
    pub fee_limit: i64,
    /// Milliseconds the transaction stays valid.
    pub expiration: i64,
}

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

impl WalletMaterial {
    pub(crate) fn sign_trc20_transfer(&self, transfer: &Trc20Transfer) -> Result<Vec<u8>> {
        if self.chain() != super::Chain::Tron {
            return Err(invalid("TRC-20 transfers require a TRON temporary wallet"));
        }
        if transfer.amount == 0 {
            return Err(invalid("TRC-20 transfer amount must be positive"));
        }
        let owner = self.address()?;
        let contract = trx::build_trc20_transfer_contract(
            &owner,
            &transfer.contract,
            &transfer.recipient,
            &transfer.amount.to_string(),
        )
        .map_err(|_| invalid("TRC-20 transfer contract could not be built"))?;
        let mut params = TronTransactionParameters::default();
        params.set_contract(contract);
        params.set_ref_block(transfer.ref_block_number, &transfer.ref_block_hash);
        params.set_fee_limit(transfer.fee_limit);
        params.set_expiration(transfer.expiration);
        let mut transaction = <TronTransaction as Transaction>::new(&params)
            .map_err(|_| invalid("TRON transaction could not be built"))?;
        let raw = transaction
            .to_bytes()
            .map_err(|_| invalid("TRON transaction could not be serialized"))?;
        let digest = Sha256::digest(&raw);
        let signature = self.sign_digest(&digest)?;
        let recovery = signature
            .recovery
            .ok_or_else(|| invalid("TRON signature has no recovery id"))?;
        transaction
            .sign(signature.bytes, recovery)
            .map_err(|_| invalid("TRON transaction could not be signed"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Tron, &[7u8; 32], 0).expect("material")
    }

    #[test]
    fn trc20_transfer_is_built_and_signed_by_the_sdk() {
        let material = material();
        let bytes = material
            .sign_trc20_transfer(&Trc20Transfer {
                contract: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".into(),
                recipient: "TG7jQ7eGsns6nmQNfcKNgZKyKBFkx7CvXr".into(),
                amount: 29_000_000,
                ref_block_number: 65_000_000,
                ref_block_hash: "0000000003df1e6e0f7f2f9c5b8d4a3e2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f"
                    .into(),
                fee_limit: 30_000_000,
                expiration: 600_000,
            })
            .unwrap();
        let signed = <TronTransaction as Transaction>::from_bytes(&bytes).unwrap();
        assert!(signed.signature.is_some());
        assert_eq!(signed.data.ref_block_hash.len(), 8);
        assert_eq!(material.address().unwrap().len(), 34);
    }
}

//! Aptos key material and fungible-asset transfers built with `aptos-sdk`
//! (spec §5.1.1): the authentication key address, BCS payload and signed
//! transaction all come from the SDK.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use aptos_sdk::account::Ed25519Account;
use aptos_sdk::crypto::Ed25519PrivateKey;
use aptos_sdk::transaction::builder::TransactionBuilder;
use aptos_sdk::transaction::input::InputEntryFunctionData;
use aptos_sdk::types::{AccountAddress, ChainId};

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

fn account(private_key: &[u8]) -> Result<Ed25519Account> {
    let key = Ed25519PrivateKey::from_bytes(private_key)
        .map_err(|_| invalid("temporary wallet Aptos key is invalid"))?;
    Ok(Ed25519Account::from_private_key(key))
}

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    if public_key.len() != 32 {
        return Err(invalid("Aptos public key has the wrong length"));
    }
    let bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| invalid("Aptos public key has the wrong length"))?;
    Ok(aptos_address(bytes)?.to_hex())
}

fn aptos_address(bytes: [u8; 32]) -> Result<AccountAddress> {
    // The Aptos authentication key is the SHA3-256 of scheme || public key.
    let mut payload = bytes.to_vec();
    payload.push(0);
    AccountAddress::from_bytes(aptos_sdk::crypto::sha3_256(&payload))
        .map_err(|_| invalid("temporary wallet Aptos address could not be derived"))
}

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
        let key = self.ed25519_secret()?;
        let account = account(&key[..])?;
        let metadata = AccountAddress::from_hex(&transfer.metadata)
            .map_err(|_| invalid("Aptos asset metadata address is invalid"))?;
        let recipient = AccountAddress::from_hex(&transfer.recipient)
            .map_err(|_| invalid("Aptos recipient address is invalid"))?;
        let payload =
            InputEntryFunctionData::transfer_fungible_asset(metadata, recipient, transfer.amount)
                .map_err(|_| invalid("Aptos transfer payload could not be built"))?;
        let signed = TransactionBuilder::new()
            .sender(account.address())
            .sequence_number(transfer.sequence_number)
            .payload(payload)
            .max_gas_amount(transfer.max_gas_amount)
            .gas_unit_price(transfer.gas_unit_price)
            .expiration_timestamp_secs(transfer.expiration_secs)
            .chain_id(ChainId::mainnet())
            .build_and_sign(&account)
            .map_err(|_| invalid("Aptos transaction could not be signed"))?;
        aptos_sdk::aptos_bcs::to_bytes(&signed)
            .map_err(|_| invalid("Aptos transaction could not be serialized"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Aptos, &[7u8; 32], 0).expect("material")
    }

    #[test]
    fn fungible_asset_transfer_is_built_and_signed_by_the_sdk() {
        let material = material();
        let bytes = material
            .sign_aptos_fa_transfer(&FaTransfer {
                metadata: format!("0x{}", "1".repeat(64)),
                recipient: format!("0x{}", "2".repeat(64)),
                amount: 29_000_000,
                sequence_number: 3,
                gas_unit_price: 100,
                max_gas_amount: 20_000,
                expiration_secs: 1_800_000_000,
            })
            .unwrap();
        let signed: aptos_sdk::transaction::SignedTransaction =
            aptos_sdk::aptos_bcs::from_bytes(&bytes).unwrap();
        assert_eq!(signed.raw_txn.sender.to_hex(), material.address().unwrap());
        assert_eq!(signed.raw_txn.sequence_number, 3);
    }
}

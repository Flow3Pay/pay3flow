//! NEAR key material and NEP-141 transfers built with `near-api-types`
//! (spec §5.1.1): implicit account ids, Borsh-serialized transactions and the
//! `ft_transfer` action all come from the SDK types.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use near_api_types::crypto::public_key::ED25519PublicKey;
use near_api_types::crypto::secret_key::ED25519SecretKey;
use near_api_types::transaction::actions::FunctionCallAction;
use near_api_types::transaction::{SignedTransaction, Transaction, TransactionV0};
use near_api_types::{AccountId, Action, CryptoHash, NearGas, NearToken, PublicKey, SecretKey};

/// A NEP-141 `ft_transfer` from the temp wallet to a payee main wallet.
pub(crate) struct FtTransfer {
    /// NEP-141 token contract account.
    pub receiver_id: String,
    /// Payee account that receives the tokens.
    pub recipient_id: String,
    pub amount: u128,
    pub gas: u64,
    /// Access-key nonce read from the chain.
    pub nonce: u64,
    pub block_hash: [u8; 32],
    pub memo: Option<String>,
}

/// Implicit account id: the hex encoding of the ed25519 public key, which NEAR
/// itself treats as the account name for a key-only account.
pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    if public_key.len() != 32 {
        return Err(invalid("NEAR public key has the wrong length"));
    }
    Ok(super::super::wallet::hex(public_key))
}

impl WalletMaterial {
    fn near_ft_transfer(&self, transfer: &FtTransfer) -> Result<SignedTransaction> {
        if self.chain() != super::Chain::Near {
            return Err(invalid("NEP-141 transfers require a NEAR temporary wallet"));
        }
        if transfer.amount == 0 {
            return Err(invalid("NEP-141 transfer amount must be positive"));
        }
        let public = self.public_key()?;
        let public: [u8; 32] = public
            .try_into()
            .map_err(|_| invalid("NEAR public key has the wrong length"))?;
        let seed = self.ed25519_secret()?;
        let secret = SecretKey::ED25519(ED25519SecretKey::from_secret_key(*seed));
        let args = serde_json::to_vec(&serde_json::json!({
            "receiver_id": transfer.recipient_id,
            "amount": transfer.amount.to_string(),
            "memo": transfer.memo,
        }))
        .map_err(|_| invalid("NEP-141 arguments could not be encoded"))?;
        let action = Action::FunctionCall(Box::new(FunctionCallAction {
            method_name: "ft_transfer".to_string(),
            args,
            gas: NearGas::from_gas(transfer.gas),
            deposit: NearToken::from_yoctonear(1),
        }));
        let transaction = Transaction::V0(TransactionV0 {
            signer_id: self
                .address()?
                .parse::<AccountId>()
                .map_err(|_| invalid("temporary wallet account id is invalid"))?,
            public_key: PublicKey::ED25519(ED25519PublicKey(public)),
            nonce: transfer.nonce,
            receiver_id: transfer
                .receiver_id
                .parse::<AccountId>()
                .map_err(|_| invalid("NEP-141 token account is invalid"))?,
            block_hash: CryptoHash(transfer.block_hash),
            actions: vec![action],
        });
        let signature = secret.sign(transaction.get_hash());
        Ok(SignedTransaction::new(signature, transaction))
    }

    /// Borsh bytes of a signed `ft_transfer`, the form the RPC accepts.
    pub(crate) fn sign_near_ft_transfer(&self, transfer: &FtTransfer) -> Result<Vec<u8>> {
        let signed = self.near_ft_transfer(transfer)?;
        borsh::to_vec(&signed).map_err(|_| invalid("NEAR transaction could not be serialized"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Near, &[7u8; 32], 0).expect("material")
    }

    #[test]
    fn ft_transfer_is_borsh_encoded_and_signed_by_the_sdk() {
        let material = material();
        let transfer = FtTransfer {
            receiver_id: "usdt.near".into(),
            recipient_id: "ab".repeat(32),
            amount: 29_000_000,
            gas: 30_000_000_000_000,
            nonce: 7,
            block_hash: [9u8; 32],
            memo: None,
        };
        let signed = material.near_ft_transfer(&transfer).unwrap();
        assert!(signed.signature.verify(
            signed.transaction.get_hash(),
            signed.transaction.public_key()
        ));
        assert_eq!(signed.transaction.nonce(), 7);
        assert_eq!(signed.transaction.receiver_id().as_str(), "usdt.near");
        assert_eq!(
            signed.transaction.signer_id().as_str(),
            material.address().unwrap()
        );
        // The broadcast bytes are the SDK's Borsh encoding of that transaction.
        let bytes = material.sign_near_ft_transfer(&transfer).unwrap();
        assert_eq!(bytes, borsh::to_vec(&signed).unwrap());
    }
}

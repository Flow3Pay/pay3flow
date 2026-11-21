//! Solana key material and SPL-token transfers built with `solana-sdk` and
//! `spl-token` (spec §5.1.1). No instruction bytes are hand-encoded here.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use solana_sdk::hash::Hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::keypair_from_seed;
use solana_sdk::transaction::Transaction;

/// An SPL-token transfer between two token accounts owned by the temp wallet
/// and the payee. Token-account addresses are supplied by the caller because
/// the payer's associated account may not exist yet.
pub(crate) struct SplTransfer {
    pub source_token_account: [u8; 32],
    pub recipient_token_account: [u8; 32],
    /// The temporary wallet that owns the source token account.
    pub owner: [u8; 32],
    pub amount: u64,
    pub recent_blockhash: [u8; 32],
}

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    let bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| invalid("Solana public key has the wrong length"))?;
    Ok(Pubkey::new_from_array(bytes).to_string())
}

/// `spl-token` builds the instruction with its own `solana-instruction`
/// version; this maps the already-encoded instruction into the SDK version the
/// message builder consumes. Only the struct fields are copied, never the
/// instruction data.
fn instruction(ix: spl_token::solana_program::instruction::Instruction) -> Instruction {
    Instruction {
        program_id: Pubkey::new_from_array(ix.program_id.to_bytes()),
        accounts: ix
            .accounts
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: Pubkey::new_from_array(meta.pubkey.to_bytes()),
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: ix.data,
    }
}

impl WalletMaterial {
    pub(crate) fn sign_spl_transfer(&self, transfer: &SplTransfer) -> Result<Vec<u8>> {
        if self.chain() != super::Chain::Solana {
            return Err(invalid("SPL transfers require a Solana temporary wallet"));
        }
        if transfer.amount == 0 {
            return Err(invalid("SPL transfer amount must be positive"));
        }
        let seed = self.ed25519_secret()?;
        let keypair = keypair_from_seed(seed.as_slice())
            .map_err(|_| invalid("temporary wallet key is invalid"))?;
        let source = Pubkey::new_from_array(transfer.source_token_account);
        let destination = Pubkey::new_from_array(transfer.recipient_token_account);
        let owner = Pubkey::new_from_array(transfer.owner);
        let built = spl_token::instruction::transfer(
            &spl_token::id(),
            &source,
            &destination,
            &owner,
            &[],
            transfer.amount,
        )
        .map_err(|_| invalid("SPL transfer instruction could not be built"))?;
        let blockhash = Hash::new_from_array(transfer.recent_blockhash);
        let message = Message::new_with_blockhash(&[instruction(built)], Some(&owner), &blockhash);
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&keypair], blockhash);
        bincode::serialize(&transaction)
            .map_err(|_| invalid("Solana transaction could not be serialized"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Solana, &[7u8; 32], 0).expect("material")
    }

    #[test]
    fn spl_transfer_is_built_and_signed_by_the_sdk() {
        let material = material();
        let owner: [u8; 32] = material.public_key().unwrap().try_into().unwrap();
        let bytes = material
            .sign_spl_transfer(&SplTransfer {
                source_token_account: [3u8; 32],
                recipient_token_account: [4u8; 32],
                owner,
                amount: 29_000_000,
                recent_blockhash: [5u8; 32],
            })
            .unwrap();
        let transaction: Transaction = bincode::deserialize(&bytes).unwrap();
        assert_eq!(transaction.signatures.len(), 1);
        assert_eq!(transaction.message.account_keys[0].to_bytes(), owner);
        assert!(transaction.signatures[0].verify(
            transaction.message.account_keys[0].as_ref(),
            &transaction.message_data(),
        ));
    }
}

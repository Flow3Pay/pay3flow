//! TON key material and wallet-V4R2 transfers built with `anychain-ton`
//! (the maintained tonlib-rs cell/wallet stack, spec §5.1.1). Addresses, cells
//! and the external message BoC all come from the SDK.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use anychain_core::{Address, Transaction};
use anychain_ton::{TonAddress, TonFormat, TonPublicKey, TonTransaction, TonTransactionParameters};
use std::str::FromStr;

/// A native TON or jetton transfer from the temp wallet.
pub(crate) struct TonTransfer {
    pub to: String,
    pub amount: u64,
    /// Jetton wallet address owned by the temp wallet; `None` sends TON.
    pub jetton_wallet: Option<String>,
    /// Wallet seqno read from the chain.
    pub seqno: u32,
    /// Unix seconds when the message was built.
    pub now: u32,
    pub comment: Option<String>,
}

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    let bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| invalid("TON public key has the wrong length"))?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&bytes)
        .map_err(|_| invalid("temporary wallet TON key is invalid"))?;
    let address = <TonAddress as Address>::from_public_key(
        &TonPublicKey(key),
        &TonFormat::MainnetNonBounceable,
    )
    .map_err(|_| invalid("temporary wallet TON address could not be derived"))?;
    Ok(address.to_string())
}

impl WalletMaterial {
    pub(crate) fn sign_ton_transfer(&self, transfer: &TonTransfer) -> Result<Vec<u8>> {
        if self.chain() != super::Chain::Ton {
            return Err(invalid("TON transfers require a TON temporary wallet"));
        }
        if transfer.amount == 0 {
            return Err(invalid("TON transfer amount must be positive"));
        }
        let public: [u8; 32] = self
            .public_key()?
            .try_into()
            .map_err(|_| invalid("TON public key has the wrong length"))?;
        let params = TonTransactionParameters {
            jetton_wallet: transfer
                .jetton_wallet
                .as_deref()
                .map(TonAddress::from_str)
                .transpose()
                .map_err(|_| invalid("jetton wallet address is invalid"))?,
            fee: None,
            from: TonAddress::from_str(&self.address()?)
                .map_err(|_| invalid("temporary wallet TON address is invalid"))?,
            to: TonAddress::from_str(&transfer.to)
                .map_err(|_| invalid("TON recipient address is invalid"))?,
            amount: transfer.amount,
            seqno: transfer.seqno,
            comment: transfer.comment.clone().unwrap_or_default(),
            now: transfer.now,
            public_key: public,
        };
        let mut transaction = <TonTransaction as Transaction>::new(&params)
            .map_err(|_| invalid("TON transaction could not be built"))?;
        // The unsigned form is the wallet-V4 signing hash the SDK builds.
        let hash = transaction
            .to_bytes()
            .map_err(|_| invalid("TON signing hash could not be built"))?;
        let signature = self.sign_message(&hash)?;
        transaction
            .sign(signature.bytes, 0)
            .map_err(|_| invalid("TON transaction could not be signed"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chain;
    use super::*;

    fn material() -> WalletMaterial {
        WalletMaterial::from_seed(Chain::Ton, &[7u8; 32], 0).expect("material")
    }

    #[test]
    fn transfer_is_built_and_signed_by_the_sdk() {
        let material = material();
        let bytes = material
            .sign_ton_transfer(&TonTransfer {
                to: "UQArwydSwhC0V8pMeBmezODPCTeqzPv56TvtprsbSgYziIVG".into(),
                amount: 1_000_000,
                jetton_wallet: None,
                seqno: 3,
                now: 1_760_000_000,
                comment: Some("otc".into()),
            })
            .unwrap();
        assert!(!bytes.is_empty());
        let parsed = <TonTransaction as Transaction>::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.signature.as_deref().map(<[u8]>::len), Some(64));
        assert_eq!(parsed.params.seqno, 3);
        assert_eq!(parsed.params.amount, 1_000_000);
    }
}

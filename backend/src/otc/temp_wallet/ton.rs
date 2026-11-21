//! TON key material and wallet-V4R2 transfers built with `tonlib-core`, the
//! pure-Rust cell/wallet part of tonlib-rs (spec §5.1.1). The native `tonlib`
//! crate needs cmake and a compiled tonlib, so only the pure-Rust core is
//! linked; addresses, cells, the state init and the external-message BoC all
//! come from it.
use super::WalletMaterial;
use crate::otc::error::{invalid, Result};
use num_bigint::BigUint;
use std::str::FromStr;
use tonlib_core::cell::{BagOfCells, Cell, CellBuilder};
use tonlib_core::message::{CommonMsgInfo, JettonTransferMessage, TonMessage, TransferMessage};
use tonlib_core::types::TonAddress;
use tonlib_core::wallet::mnemonic::KeyPair;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::wallet_version::WalletVersion;
use zeroize::Zeroize;

/// Value forwarded to the jetton wallet so it can send the transfer onward.
const JETTON_MESSAGE_VALUE: u64 = 100_000_000;
/// Wallet V4R2 valid-until window, in seconds.
const VALID_FOR: u32 = 600;

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

fn wallet(public_key: &[u8], secret_key: Vec<u8>) -> Result<TonWallet> {
    if public_key.len() != 32 {
        return Err(invalid("TON public key has the wrong length"));
    }
    TonWallet::new(
        WalletVersion::V4R2,
        KeyPair {
            public_key: public_key.to_vec(),
            secret_key,
        },
    )
    .map_err(|_| invalid("temporary wallet TON address could not be derived"))
}

pub(crate) fn address(public_key: &[u8]) -> Result<String> {
    let wallet = wallet(public_key, Vec::new())?;
    Ok(wallet.address.to_base64_url_flags(true, false))
}

fn comment_cell(comment: Option<&str>) -> Result<Option<Cell>> {
    let Some(text) = comment.filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    if text.len() > 120 {
        return Err(invalid("TON transfer comment is too long"));
    }
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, 0)
        .map_err(|_| invalid("TON comment could not be encoded"))?;
    builder
        .store_string(text)
        .map_err(|_| invalid("TON comment could not be encoded"))?;
    Ok(Some(builder.build().map_err(|_| {
        invalid("TON comment could not be encoded")
    })?))
}

fn empty_cell() -> Result<Cell> {
    CellBuilder::new()
        .build()
        .map_err(|_| invalid("TON message body could not be built"))
}

impl WalletMaterial {
    pub(crate) fn sign_ton_transfer(&self, transfer: &TonTransfer) -> Result<Vec<u8>> {
        if self.chain() != super::Chain::Ton {
            return Err(invalid("TON transfers require a TON temporary wallet"));
        }
        if transfer.amount == 0 {
            return Err(invalid("TON transfer amount must be positive"));
        }
        let public_key = self.public_key()?;
        let secret = self.ed25519_secret()?;
        // `tonlib-core` signs through nacl, whose secret key is the 64-byte
        // `seed || public key` pair rather than the bare 32-byte seed.
        let mut key_pair = Vec::with_capacity(64);
        key_pair.extend_from_slice(&*secret);
        key_pair.extend_from_slice(&public_key);
        let mut wallet = wallet(&public_key, key_pair)?;
        let destination = TonAddress::from_str(&transfer.to)
            .map_err(|_| invalid("TON recipient address is invalid"))?;
        let comment = comment_cell(transfer.comment.as_deref())?;
        let message = match &transfer.jetton_wallet {
            Some(jetton_wallet) => {
                let jetton = TonAddress::from_str(jetton_wallet)
                    .map_err(|_| invalid("jetton wallet address is invalid"))?;
                let mut body =
                    JettonTransferMessage::new(&destination, &BigUint::from(transfer.amount));
                body.query_id = u64::from(transfer.seqno);
                if let Some(comment) = comment.clone() {
                    body.forward_payload = comment.to_arc();
                    body.forward_ton_amount = BigUint::from(1u64);
                }
                let body = body
                    .build()
                    .map_err(|_| invalid("jetton transfer body could not be built"))?;
                TransferMessage::new(
                    CommonMsgInfo::new_default_internal(
                        &jetton,
                        &BigUint::from(JETTON_MESSAGE_VALUE),
                    ),
                    body.to_arc(),
                )
            }
            None => TransferMessage::new(
                CommonMsgInfo::new_default_internal(&destination, &BigUint::from(transfer.amount)),
                match comment.clone() {
                    Some(comment) => comment.to_arc(),
                    None => empty_cell()?.to_arc(),
                },
            ),
        };
        let message = message
            .build()
            .map_err(|_| invalid("TON transfer message could not be built"))?;
        let external = wallet
            .create_external_msg(
                transfer.now.saturating_add(VALID_FOR),
                transfer.seqno,
                transfer.seqno == 0,
                &[message.to_arc()],
            )
            .map_err(|_| invalid("TON external message could not be built"))?;
        wallet.key_pair.secret_key.zeroize();
        BagOfCells::from_root(external)
            .serialize(true)
            .map_err(|_| invalid("TON external message could not be serialized"))
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
    fn address_is_a_wallet_v4r2_deposit_address() {
        let material = material();
        let address = material.address().unwrap();
        assert_eq!(address.len(), 48);
        assert!(address.starts_with('U'), "{address}");
        let next = material.derive_next().unwrap().address().unwrap();
        assert_eq!(next.len(), 48);
        assert_ne!(next, address);
    }

    #[test]
    fn native_transfer_builds_a_signed_boc() {
        let material = material();
        let bytes = material
            .sign_ton_transfer(&TonTransfer {
                to: "EQCDM_QGggZ3qMa_f3lRPk4_qLDnLTqdi6OkMAV2NB9r5TG3".into(),
                amount: 1_000_000,
                jetton_wallet: None,
                seqno: 3,
                now: 1_760_000_000,
                comment: Some("otc".into()),
            })
            .unwrap();
        let boc = BagOfCells::parse(&bytes).unwrap();
        assert_eq!(boc.num_roots(), 1);
    }

    #[test]
    fn jetton_transfer_builds_a_signed_boc() {
        let material = material();
        let bytes = material
            .sign_ton_transfer(&TonTransfer {
                to: "EQCDM_QGggZ3qMa_f3lRPk4_qLDnLTqdi6OkMAV2NB9r5TG3".into(),
                amount: 29_000_000,
                jetton_wallet: Some("EQCDM_QGggZ3qMa_f3lRPk4_qLDnLTqdi6OkMAV2NB9r5TG3".into()),
                seqno: 0,
                now: 1_760_000_000,
                comment: None,
            })
            .unwrap();
        let boc = BagOfCells::parse(&bytes).unwrap();
        assert_eq!(boc.num_roots(), 1);
    }
}

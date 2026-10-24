use anyhow::{Context, Result, ensure};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize, ser::SerializeStruct};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::domain::PublicKey;

pub const DERIVATION_PATH: &str = "m/44'/396'/0'/0/0";

// Deliberately no Debug: this value contains a signing secret.
pub struct Keypair {
    pub public: PublicKey,
    secret: Zeroizing<[u8; 32]>,
}

impl Keypair {
    pub fn derive(phrase: &str) -> Result<Self> {
        let mnemonic = Mnemonic::parse_in(Language::English, phrase)
            .map_err(|_| anyhow::anyhow!("Invalid English BIP39 phrase"))?;
        ensure!(
            mnemonic.word_count() == 12,
            "The source wallet uses 12-word phrases"
        );
        let seed = Zeroizing::new(mnemonic.to_seed(""));
        let path: DerivationPath = DERIVATION_PATH
            .parse()
            .context("Invalid built-in derivation path")?;
        let derived = XPrv::derive_from_path(seed.as_ref(), &path)
            .map_err(|_| anyhow::anyhow!("BIP32 derivation failed"))?;
        let secret = Zeroizing::new(derived.private_key().to_bytes().into());
        let public = PublicKey::from_hex(&hex::encode(
            SigningKey::from_bytes(&secret).verifying_key().as_bytes(),
        ))?;
        Ok(Self { public, secret })
    }

    pub fn from_json(input: &[u8]) -> Result<Self> {
        #[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            public: String,
            secret: String,
        }
        let wire: Wire =
            serde_json::from_slice(input).map_err(|_| anyhow::anyhow!("Invalid keypair JSON"))?;
        let bytes = Zeroizing::new(
            hex::decode(&wire.secret).map_err(|_| anyhow::anyhow!("Invalid keypair secret"))?,
        );
        let secret: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Keypair secret must contain 32 bytes"))?;
        let secret = Zeroizing::new(secret);
        let public = PublicKey::from_hex(&wire.public)?;
        let actual = PublicKey::from_hex(&hex::encode(
            SigningKey::from_bytes(&secret).verifying_key().as_bytes(),
        ))?;
        ensure!(
            public == actual,
            "Keypair public and secret keys do not match"
        );
        Ok(Self { public, secret })
    }
}

impl Serialize for Keypair {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let secret = Zeroizing::new(hex::encode(self.secret.as_ref()));
        let mut state = serializer.serialize_struct("Keypair", 2)?;
        state.serialize_field("public", self.public.as_hex())?;
        state.serialize_field("secret", secret.as_str())?;
        state.end()
    }
}

pub fn generate_phrase() -> Result<Zeroizing<String>> {
    let mnemonic =
        Mnemonic::generate_in(Language::English, 12).context("Could not generate mnemonic")?;
    Ok(Zeroizing::new(mnemonic.to_string()))
}

//! Temporary custodial wallet material for OTC deals.
//!
//! This is phases B and C of `docs/OTC_TEMP_MULTICHAIN_WALLETS.md`: one random
//! master seed per wallet row, deterministic chain keys, and per-row salt with
//! AES-256-GCM at rest. Transaction assembly, signing of chain transfers, and
//! broadcasting deliberately live outside this module: until a settlement
//! signer exists, a deal must never create an address that holds user funds.
use super::{
    error::{invalid, Result},
    wallet::hex,
};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use bip32::{DerivationPath, XPrv};
use ed25519_dalek::{Signer, SigningKey as EdSigningKey};
use hmac::{Hmac, Mac};
use k256::ecdsa::SigningKey;
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256, Sha512};
use sha3::{Keccak256, Sha3_256};
use std::str::FromStr;
use zeroize::Zeroizing;

const INFO: &[u8] = b"otc-temp-wallet-v1";
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 32;
/// BIP32 non-hardened child indexes stop one below this value.
const MAX_INDEX: u32 = 0x7fff_ffff;

/// Canonical network ids from the temp-wallet spec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Chain {
    Ethereum,
    PolygonPos,
    AvalancheC,
    Tron,
    Solana,
    Near,
    Ton,
    Aptos,
    Everscale,
}

impl Chain {
    pub(crate) fn parse(id: &str) -> Result<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "eth" | "ethereum" => Ok(Self::Ethereum),
            "polygon" | "polygon-pos" => Ok(Self::PolygonPos),
            "avalanche" | "avalanche-c" => Ok(Self::AvalancheC),
            "tron" => Ok(Self::Tron),
            "sol" | "solana" => Ok(Self::Solana),
            "near" => Ok(Self::Near),
            "ton" => Ok(Self::Ton),
            "aptos" => Ok(Self::Aptos),
            "everscale" => Ok(Self::Everscale),
            _ => Err(invalid("unsupported temporary wallet network")),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::PolygonPos => "polygon-pos",
            Self::AvalancheC => "avalanche-c",
            Self::Tron => "tron",
            Self::Solana => "solana",
            Self::Near => "near",
            Self::Ton => "ton",
            Self::Aptos => "aptos",
            Self::Everscale => "everscale",
        }
    }

    /// Polygon and Avalanche reuse the Ethereum secp256k1 address.
    fn is_evm(self) -> bool {
        matches!(self, Self::Ethereum | Self::PolygonPos | Self::AvalancheC)
    }

    fn is_secp256k1(self) -> bool {
        self.is_evm() || self == Self::Tron
    }

    /// BIP44 coin type for secp256k1 families.
    fn coin_type(self) -> Option<u32> {
        match self {
            Self::Ethereum | Self::PolygonPos | Self::AvalancheC => Some(60),
            Self::Tron => Some(195),
            _ => None,
        }
    }
}

/// Which job a wallet performs in a deal (spec §4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Ever,
    Usd,
    UsdIn,
    UsdOut,
}

impl Role {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ever => "ever",
            Self::Usd => "usd",
            Self::UsdIn => "usd_in",
            Self::UsdOut => "usd_out",
        }
    }
}

/// Two wallets when both stablecoin legs share a network, three otherwise.
///
/// Everscale is only ever the EVER leg; it is not a stablecoin network in this
/// product, so a caller that asks for it as a USD destination is rejected.
pub(crate) fn roles(send_chain_usd: &str, get_chain_usd: &str) -> Result<Vec<(Role, Chain)>> {
    let send = Chain::parse(send_chain_usd)?;
    let get = Chain::parse(get_chain_usd)?;
    for chain in [send, get] {
        if chain == Chain::Everscale {
            return Err(invalid(
                "Everscale is not a supported stablecoin temporary wallet network",
            ));
        }
    }
    let mut roles = vec![(Role::Ever, Chain::Everscale)];
    if send == get {
        roles.push((Role::Usd, send));
    } else {
        roles.push((Role::UsdIn, send));
        roles.push((Role::UsdOut, get));
    }
    Ok(roles)
}

/// Key material for one temporary wallet row. Deliberately no `Debug`.
pub(crate) struct WalletMaterial {
    chain: Chain,
    seed: Zeroizing<Vec<u8>>,
    derivation_index: u32,
}

pub(crate) struct WalletSignature {
    pub bytes: Vec<u8>,
    /// Recovery id for secp256k1 chains; `None` for ed25519.
    pub recovery: Option<u8>,
}

impl WalletMaterial {
    /// Fresh random master seed for a new wallet row.
    pub(crate) fn generate(chain: Chain) -> Result<Self> {
        let mut seed = Zeroizing::new(vec![0u8; 32]);
        OsRng.fill_bytes(seed.as_mut_slice());
        Ok(Self {
            chain,
            seed,
            derivation_index: 0,
        })
    }

    /// Build material from a stored seed. Seeds are 16 to 64 bytes; the BIP39
    /// vectors use the standard 64-byte seed.
    pub(crate) fn from_seed(chain: Chain, seed: &[u8], derivation_index: u32) -> Result<Self> {
        if !(16..=64).contains(&seed.len()) || derivation_index > MAX_INDEX {
            return Err(invalid("invalid temporary wallet seed or derivation index"));
        }
        Ok(Self {
            chain,
            seed: Zeroizing::new(seed.to_vec()),
            derivation_index,
        })
    }

    /// Same seed family, next address (`create_based_on` in the spec).
    pub(crate) fn derive_next(&self) -> Result<Self> {
        let index = self
            .derivation_index
            .checked_add(1)
            .filter(|index| *index <= MAX_INDEX)
            .ok_or_else(|| invalid("temporary wallet derivation index exhausted"))?;
        Ok(Self {
            chain: self.chain,
            seed: self.seed.clone(),
            derivation_index: index,
        })
    }

    pub(crate) fn chain(&self) -> Chain {
        self.chain
    }

    pub(crate) fn derivation_index(&self) -> u32 {
        self.derivation_index
    }

    pub(crate) fn seed(&self) -> &[u8] {
        self.seed.as_slice()
    }

    fn secp256k1(&self) -> Result<SigningKey> {
        let coin = self
            .chain
            .coin_type()
            .ok_or_else(|| invalid("temporary wallet network is not secp256k1"))?;
        let path: DerivationPath =
            DerivationPath::from_str(&format!("m/44'/{coin}'/0'/0/{}", self.derivation_index))
                .map_err(|_| invalid("invalid temporary wallet derivation path"))?;
        let derived = XPrv::derive_from_path(self.seed.as_slice(), &path)
            .map_err(|_| invalid("temporary wallet key derivation failed"))?;
        let bytes: [u8; 32] = derived.private_key().to_bytes().into();
        SigningKey::from_bytes((&bytes).into())
            .map_err(|_| invalid("temporary wallet key is invalid"))
    }

    /// ed25519 families get a domain-separated child key so one seed can serve
    /// successive `create_based_on` rows without an HD-ed25519 dependency.
    fn ed25519(&self) -> Result<EdSigningKey> {
        let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(self.seed.as_slice())
            .map_err(|_| invalid("temporary wallet seed is invalid"))?;
        mac.update(INFO);
        mac.update(self.chain.id().as_bytes());
        mac.update(&self.derivation_index.to_be_bytes());
        let derived = mac.finalize().into_bytes();
        let key: [u8; 32] = derived[..32]
            .try_into()
            .map_err(|_| invalid("temporary wallet key derivation failed"))?;
        Ok(EdSigningKey::from_bytes(&key))
    }

    /// Uncompressed secp256k1 public key without the `0x04` prefix, or the raw
    /// 32-byte ed25519 public key.
    pub(crate) fn public_key(&self) -> Result<Vec<u8>> {
        if self.chain.is_secp256k1() {
            let point = self.secp256k1()?.verifying_key().to_encoded_point(false);
            Ok(point.as_bytes()[1..].to_vec())
        } else {
            Ok(self.ed25519()?.verifying_key().to_bytes().to_vec())
        }
    }

    pub(crate) fn address(&self) -> Result<String> {
        match self.chain {
            Chain::Ethereum | Chain::PolygonPos | Chain::AvalancheC => {
                Ok(evm_address(&self.secp256k1()?.verifying_key()))
            }
            Chain::Tron => {
                let point = self.secp256k1()?.verifying_key().to_encoded_point(false);
                Ok(tron_address(&point.as_bytes()[1..]))
            }
            Chain::Solana => Ok(bs58::encode(self.public_key()?).into_string()),
            Chain::Near => Ok(hex(&self.public_key()?)),
            Chain::Aptos => {
                let mut bytes = self.public_key()?;
                bytes.push(0);
                Ok(format!("0x{}", hex(&Sha3_256::digest(&bytes))))
            }
            Chain::Everscale => {
                let key: [u8; 32] = self.ed25519()?.verifying_key().to_bytes();
                super::everwallet::address(&key)
            }
            Chain::Ton => Err(invalid(
                "TON temporary wallet address requires the TON wallet SDK",
            )),
        }
    }

    /// Signs message bytes with the chain's key. secp256k1 chains sign the
    /// keccak digest the way EVM/TRON transactions consume it; ed25519 chains
    /// sign the bytes directly.
    pub(crate) fn sign_message(&self, message: &[u8]) -> Result<WalletSignature> {
        if self.chain.is_secp256k1() {
            self.sign_digest(&Keccak256::digest(message))
        } else {
            let signature = self.ed25519()?.sign(message);
            Ok(WalletSignature {
                bytes: signature.to_bytes().to_vec(),
                recovery: None,
            })
        }
    }

    /// Signs a 32-byte prehash. This is the primitive a transaction builder
    /// consumes; it does not assemble or broadcast a chain transaction.
    pub(crate) fn sign_digest(&self, digest: &[u8]) -> Result<WalletSignature> {
        if !self.chain.is_secp256k1() {
            return Err(invalid("digest signing is only used by secp256k1 chains"));
        }
        let (signature, recovery) = self
            .secp256k1()?
            .sign_prehash_recoverable(digest)
            .map_err(|_| invalid("temporary wallet signing failed"))?;
        Ok(WalletSignature {
            bytes: signature.to_bytes().to_vec(),
            recovery: Some(recovery.to_byte()),
        })
    }
}

fn evm_address(key: &k256::ecdsa::VerifyingKey) -> String {
    let point = key.to_encoded_point(false);
    let hash = Keccak256::digest(&point.as_bytes()[1..]);
    format!("0x{}", hex(&hash[12..]))
}

fn tron_address(public: &[u8]) -> String {
    let hash = Keccak256::digest(public);
    let mut payload = [0u8; 21];
    payload[0] = 0x41;
    payload[1..].copy_from_slice(&hash[12..]);
    let checksum = Sha256::digest(Sha256::digest(payload));
    let mut encoded = payload.to_vec();
    encoded.extend_from_slice(&checksum[..4]);
    bs58::encode(encoded).into_string()
}

/// One encrypted seed with its own salt (spec §5.2).
pub(crate) struct SealedSeed {
    pub salt: [u8; SALT_LEN],
    pub ciphertext: Vec<u8>,
}

fn cipher(secret_key: &str, salt: &[u8]) -> Result<Aes256Gcm> {
    let mut hasher = Sha256::new();
    hasher.update(secret_key.as_bytes());
    hasher.update(salt);
    hasher.update(INFO);
    let key = hasher.finalize();
    Aes256Gcm::new_from_slice(&key).map_err(|_| invalid("temporary wallet cipher is unavailable"))
}

/// Encrypts a seed under `SECRETS_KEY` and a fresh per-row salt.
///
/// The spec names HKDF-SHA-256 as the target KDF and explicitly accepts
/// `SHA-256(SECRETS_KEY || salt)` until HKDF is added; that interim is what this
/// function uses today, with the domain tag mixed in.
pub(crate) fn seal(secret_key: &str, seed: &[u8]) -> Result<SealedSeed> {
    if !(16..=64).contains(&seed.len()) {
        return Err(invalid("invalid temporary wallet seed length"));
    }
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let cipher = cipher(secret_key, &salt)?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), seed)
        .map_err(|_| invalid("temporary wallet encryption failed"))?;
    let mut ciphertext = Vec::with_capacity(NONCE_LEN + encrypted.len());
    ciphertext.extend_from_slice(&nonce);
    ciphertext.extend_from_slice(&encrypted);
    Ok(SealedSeed { salt, ciphertext })
}

/// Decrypts a stored seed; a wrong `SECRETS_KEY` or a tampered blob fails
/// closed instead of returning garbage key material.
pub(crate) fn open(
    secret_key: &str,
    salt: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<[u8; 32]>> {
    if salt.len() < 16 || ciphertext.len() <= NONCE_LEN {
        return Err(invalid("stored temporary wallet material is malformed"));
    }
    let cipher = cipher(secret_key, salt)?;
    let (nonce, body) = ciphertext.split_at(NONCE_LEN);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(Nonce::from_slice(nonce), body)
            .map_err(|_| invalid("temporary wallet key cannot be decrypted"))?,
    );
    let seed: [u8; 32] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| invalid("stored temporary wallet seed has the wrong length"))?;
    Ok(Zeroizing::new(seed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::{Language, Mnemonic};
    use ed25519_dalek::{Signature as EdSignature, Verifier};
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    /// Hardhat account #0 for the canonical BIP39 test mnemonic.
    const EVM_VECTOR: &str = "0x9858effd232b4033e47d90003d41ec34ecaeda94";

    fn vector_material(chain: Chain) -> WalletMaterial {
        let mnemonic = Mnemonic::parse_in(
            Language::English,
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("test mnemonic");
        let seed = mnemonic.to_seed("");
        WalletMaterial::from_seed(chain, &seed, 0).expect("material")
    }

    #[test]
    fn canonical_network_ids_and_aliases_resolve() {
        for (input, expected) in [
            ("eth", Chain::Ethereum),
            ("ethereum", Chain::Ethereum),
            ("polygon", Chain::PolygonPos),
            ("polygon-pos", Chain::PolygonPos),
            ("avalanche", Chain::AvalancheC),
            ("avalanche-c", Chain::AvalancheC),
            ("sol", Chain::Solana),
            ("tron", Chain::Tron),
            ("near", Chain::Near),
            ("ton", Chain::Ton),
            ("aptos", Chain::Aptos),
            ("everscale", Chain::Everscale),
        ] {
            assert_eq!(Chain::parse(input).unwrap(), expected, "{input}");
            assert_eq!(Chain::parse(expected.id()).unwrap(), expected);
        }
        assert!(Chain::parse("cosmos").is_err());
    }

    #[test]
    fn bip44_vector_matches_the_known_evm_address() {
        assert_eq!(
            vector_material(Chain::Ethereum).address().unwrap(),
            EVM_VECTOR
        );
        assert_eq!(
            vector_material(Chain::PolygonPos).address().unwrap(),
            EVM_VECTOR
        );
        assert_eq!(
            vector_material(Chain::AvalancheC).address().unwrap(),
            EVM_VECTOR
        );
    }

    #[test]
    fn tron_address_is_base58check_with_the_0x41_prefix() {
        let address = vector_material(Chain::Tron).address().unwrap();
        assert_eq!(address.len(), 34);
        assert!(address.starts_with('T'));
        let bytes = bs58::decode(&address).into_vec().unwrap();
        assert_eq!(bytes.len(), 25);
        assert_eq!(bytes[0], 0x41);
        let checksum = Sha256::digest(Sha256::digest(&bytes[..21]));
        assert_eq!(&bytes[21..], &checksum[..4]);
        // A second derivation is stable but different.
        let next = vector_material(Chain::Tron).derive_next().unwrap();
        assert_ne!(next.address().unwrap(), address);
    }

    #[test]
    fn ed25519_families_sign_and_verify_with_chain_format_addresses() {
        for chain in [Chain::Solana, Chain::Near, Chain::Aptos, Chain::Everscale] {
            let material = vector_material(chain);
            let address = material.address().unwrap();
            match chain {
                Chain::Solana => {
                    assert_eq!(bs58::decode(&address).into_vec().unwrap().len(), 32)
                }
                Chain::Near => {
                    assert_eq!(address.len(), 64);
                    assert!(address
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
                }
                Chain::Aptos => {
                    assert_eq!(address.len(), 66);
                    assert!(address.starts_with("0x"));
                }
                Chain::Everscale => {
                    assert_eq!(address.len(), 66);
                    assert!(address.starts_with("0:"));
                }
                _ => unreachable!(),
            }
            let signature = material.sign_message(b"otc-temp-wallet").unwrap();
            let verifying = material.ed25519().unwrap().verifying_key();
            verifying
                .verify(
                    b"otc-temp-wallet",
                    &EdSignature::from_slice(&signature.bytes).unwrap(),
                )
                .expect("ed25519 signature verifies");
            assert_ne!(
                material.derive_next().unwrap().address().unwrap(),
                address,
                "{chain:?}"
            );
        }
    }

    #[test]
    fn secp256k1_signatures_recover_the_wallet_address() {
        for chain in [Chain::Ethereum, Chain::Tron] {
            let material = vector_material(chain);
            let digest = Keccak256::digest(b"payload");
            let signature = material.sign_digest(&digest).unwrap();
            let signature = Signature::from_slice(&signature.bytes).unwrap();
            let recovery = RecoveryId::try_from(signature_recovery(&material, &digest)).unwrap();
            let verifying =
                VerifyingKey::recover_from_prehash(&digest, &signature, recovery).unwrap();
            let recovered = if chain == Chain::Tron {
                tron_address(&verifying.to_encoded_point(false).as_bytes()[1..])
            } else {
                evm_address(&verifying)
            };
            assert_eq!(recovered, material.address().unwrap());
        }
    }

    fn signature_recovery(material: &WalletMaterial, digest: &[u8]) -> u8 {
        material
            .sign_digest(digest)
            .unwrap()
            .recovery
            .expect("secp256k1 recovery id")
    }

    #[test]
    fn next_derivation_keeps_the_seed_and_moves_the_address() {
        let material = vector_material(Chain::Ethereum);
        let child = material.derive_next().unwrap();
        assert_eq!(child.seed(), material.seed());
        assert_eq!(child.derivation_index(), 1);
        assert_ne!(child.address().unwrap(), material.address().unwrap());
        assert_eq!(child.chain(), Chain::Ethereum);
    }

    #[test]
    fn salt_and_ciphertext_differ_for_the_same_seed_and_open_fails_closed() {
        let seed = [7u8; 32];
        let first = seal("unit-test-secret", &seed).unwrap();
        let second = seal("unit-test-secret", &seed).unwrap();
        assert_ne!(first.salt, second.salt);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert_eq!(
            &*open("unit-test-secret", &first.salt, &first.ciphertext).unwrap(),
            &seed
        );
        assert!(open("other-secret", &first.salt, &first.ciphertext).is_err());
        let mut tampered = first.ciphertext.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(open("unit-test-secret", &first.salt, &tampered).is_err());
        assert!(open("unit-test-secret", &[0u8; 8], &first.ciphertext).is_err());
        assert!(open("unit-test-secret", &first.salt, &[]).is_err());
    }

    #[test]
    fn ton_needs_the_wallet_sdk_before_it_can_produce_an_address() {
        let material = WalletMaterial::generate(Chain::Ton).unwrap();
        assert!(material.public_key().is_ok());
        assert!(material.address().is_err());
    }

    #[test]
    fn deal_shape_uses_two_or_three_wallets() {
        let same = roles("ethereum", "ethereum").unwrap();
        assert_eq!(
            same,
            vec![(Role::Ever, Chain::Everscale), (Role::Usd, Chain::Ethereum)]
        );
        let different = roles("tron", "solana").unwrap();
        assert_eq!(
            different,
            vec![
                (Role::Ever, Chain::Everscale),
                (Role::UsdIn, Chain::Tron),
                (Role::UsdOut, Chain::Solana),
            ]
        );
        assert_eq!(
            roles("sol", "solana").unwrap(),
            vec![(Role::Ever, Chain::Everscale), (Role::Usd, Chain::Solana)]
        );
        assert!(roles("everscale", "ethereum").is_err());
    }

    #[test]
    fn generated_material_never_debug_prints_the_seed() {
        let material = WalletMaterial::generate(Chain::Solana).unwrap();
        let rendered = format!("{:?}", material.chain());
        assert_eq!(rendered, "Solana");
        assert!(!rendered.contains("seed"));
    }
}

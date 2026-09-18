use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

const NONCE_LEN: usize = 12;
const B64_ERR: &str = "base64 decode failed";

/// AES-256-GCM box for provider credentials and other secrets.
///
/// The key is derived from an application secret (SHA-256 over the configured
/// `SECRETS_KEY`), so any non-empty string works in dev while still requiring a
/// real 256-bit secret in production. Stored secret format:
/// `base64(nonce(12) || ciphertext)`.
#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
}

impl SecretBox {
    pub fn new(secret: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let key: [u8; 32] = hasher.finalize().into();
        let cipher = Aes256Gcm::new_from_slice(&key).expect("32-byte key is always valid");
        Self { cipher }
    }

    /// Encrypt a plaintext secret to `base64(nonce || ciphertext)`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow!("encryption failed"))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(B64.encode(out))
    }

    /// Decrypt `base64(nonce || ciphertext)` back to plaintext.
    pub fn decrypt(&self, encoded: &str) -> Result<String> {
        let raw = B64.decode(encoded).context(B64_ERR)?;
        if raw.len() < NONCE_LEN {
            return Err(anyhow!("secret too short"));
        }
        let (nonce, ciphertext) = raw.split_at(NONCE_LEN);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("decryption failed: key mismatch or corrupted secret"))?;
        Ok(String::from_utf8(plaintext).context("secret is not utf-8")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_secret() {
        let boxed = SecretBox::new("unit-test-secret");
        let enc = boxed.encrypt("sk_live_abc123").unwrap();
        assert_ne!(enc, "sk_live_abc123");
        assert_eq!(boxed.decrypt(&enc).unwrap(), "sk_live_abc123");
    }

    #[test]
    fn distinct_ciphertexts_for_same_plaintext() {
        let boxed = SecretBox::new("unit-test-secret");
        let a = boxed.encrypt("same").unwrap();
        let b = boxed.encrypt("same").unwrap();
        assert_ne!(a, b, "random nonce must produce different ciphertext");
        assert_eq!(boxed.decrypt(&a).unwrap(), "same");
        assert_eq!(boxed.decrypt(&b).unwrap(), "same");
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let en = SecretBox::new("key one").encrypt("top secret").unwrap();
        let de = SecretBox::new("key two").decrypt(&en);
        assert!(de.is_err(), "a different key must fail to decrypt");
    }
}

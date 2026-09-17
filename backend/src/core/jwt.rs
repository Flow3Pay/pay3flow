use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const TTL_SECS: u64 = 60 * 60 * 24 * 7;

#[derive(Clone)]
pub struct Jwt {
    encode_key: EncodingKey,
    decode_key: DecodingKey,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: u64,
}

impl Jwt {
    pub fn new(secret: &str) -> Self {
        Self {
            encode_key: EncodingKey::from_secret(secret.as_bytes()),
            decode_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn sign(&self, sub: &str) -> anyhow::Result<String> {
        let claims = Claims {
            sub: sub.to_string(),
            exp: now_secs() + TTL_SECS,
        };
        Ok(encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &self.encode_key,
        )?)
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<String> {
        let data = decode::<Claims>(token, &self.decode_key, &Validation::new(Algorithm::HS256))?;
        Ok(data.claims.sub)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

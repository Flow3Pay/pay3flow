use super::{
    config::PROFILE,
    error::{invalid, Error, Result},
    Service,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signature as EdSignature, VerifyingKey};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey as EvmKey};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Digest;
use sha3::Keccak256;
use uuid::Uuid;

pub(crate) fn address(chain: &str, value: &str) -> Result<String> {
    let valid = match chain {
        "ethereum" => {
            value.len() == 42
                && value.starts_with("0x")
                && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
        }
        "everscale" => value.split_once(':').is_some_and(|(workchain, hash)| {
            workchain == "0" && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
        }),
        _ => false,
    };
    if !valid {
        return Err(invalid("invalid wallet address or chain"));
    }
    Ok(value.to_ascii_lowercase())
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn unhex(value: &str) -> Result<Vec<u8>> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() % 2 != 0
        || value.len() > 100_000
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(invalid("invalid hex"));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let value = std::str::from_utf8(pair).map_err(|_| invalid("invalid hex"))?;
            u8::from_str_radix(value, 16).map_err(|_| invalid("invalid hex"))
        })
        .collect()
}
pub(crate) fn verify_evm(message: &str, signature: &str, expected: &str) -> Result<()> {
    let bytes = unhex(signature)?;
    if bytes.len() != 65 {
        return Err(Error::Unauthorized);
    }
    let signature = Signature::from_slice(&bytes[..64]).map_err(|_| Error::Unauthorized)?;
    let recovery = RecoveryId::try_from(match bytes[64] {
        27 | 28 => bytes[64] - 27,
        0 | 1 => bytes[64],
        _ => return Err(Error::Unauthorized),
    })
    .map_err(|_| Error::Unauthorized)?;
    let prefixed = format!("\x19Ethereum Signed Message:\n{}{message}", message.len());
    let key = EvmKey::recover_from_digest(
        Keccak256::new_with_prefix(prefixed.as_bytes()),
        &signature,
        recovery,
    )
    .map_err(|_| Error::Unauthorized)?;
    let point = key.to_encoded_point(false);
    let hash = Keccak256::digest(&point.as_bytes()[1..]);
    if format!("0x{}", hex(&hash[12..])) != expected {
        return Err(Error::Unauthorized);
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkRequest {
    pub actor: String,
    pub credential: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChallengeRequest {
    pub chain: String,
    pub address: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProofRequest {
    pub challenge_id: Uuid,
    pub signature: String,
    pub public_key: Option<String>,
}
impl Service {
    pub(crate) async fn link(&self, request: LinkRequest) -> Result<Value> {
        let actor = self.relay_actor_url(&request.actor)?;
        if !request.credential.starts_with("lf-") || request.credential.len() > 512 {
            return Err(Error::Unauthorized);
        }
        let response = self
            .http
            .get(format!("{actor}/outbox"))
            .bearer_auth(&request.credential)
            .send()
            .await?;
        if (!self.cfg.demo
            && response
                .headers()
                .get("x-relay-signatures")
                .and_then(|h| h.to_str().ok())
                != Some("required"))
            || !response.status().is_success()
            || response
                .headers()
                .get("x-relay-owner")
                .and_then(|h| h.to_str().ok())
                != Some(&request.actor)
            || response
                .headers()
                .get("x-relay-profile")
                .and_then(|h| h.to_str().ok())
                != Some(PROFILE)
        {
            return Err(Error::Unauthorized);
        }
        let id = Uuid::new_v4();
        let expires = Utc::now() + Duration::hours(8);
        let encrypted = self.secrets.encrypt(&request.credential)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute("INSERT INTO otc_actor_links(actor,credential) VALUES($1,$2) ON CONFLICT(actor) DO UPDATE SET credential=$2,revoked=FALSE,updated_at=now()",&[&request.actor,&encrypted]).await?;
        tx.execute(
            "INSERT INTO otc_sessions(id,actor,credential,expires_at) VALUES($1,$2,$3,$4)",
            &[&id, &request.actor, &encrypted, &expires],
        )
        .await?;
        tx.commit().await?;
        Ok(
            json!({"token":self.jwt.sign(&format!("otc:{id}"))?,"actor":request.actor,"desk":request.actor==self.cfg.desk_actor,"expires_at":expires}),
        )
    }
    pub(crate) async fn challenge(
        &self,
        session: Uuid,
        actor: &str,
        request: ChallengeRequest,
    ) -> Result<Value> {
        let address = address(&request.chain, &request.address)?;
        let id = Uuid::new_v4();
        let expires = Utc::now() + Duration::minutes(5);
        let message=format!("Pay3flow OTC wallet ownership\nOrigin: {}\nActor: {actor}\nChain: {}\nAddress: {address}\nNonce: {id}\nExpires: {expires}\nThis authorizes no transfer.",self.cfg.origin,request.chain);
        let client = self.pool.get().await?;
        client.execute("INSERT INTO otc_wallet_challenges(id,session_id,chain,address,message,expires_at) VALUES($1,$2,$3,$4,$5,$6)",&[&id,&session,&request.chain,&address,&message,&expires]).await?;
        Ok(
            json!({"id":id,"message":message,"expires_at":expires,"everscale_data":STANDARD.encode(message.as_bytes()),"withSignatureId":false}),
        )
    }
    pub(crate) async fn prove(
        &self,
        session: Uuid,
        actor: &str,
        request: ProofRequest,
    ) -> Result<Value> {
        let client = self.pool.get().await?;
        let row=client.query_opt("SELECT chain,address,message FROM otc_wallet_challenges WHERE id=$1 AND session_id=$2 AND expires_at>now() AND consumed_at IS NULL",&[&request.challenge_id,&session]).await?.ok_or(Error::Unauthorized)?;
        let chain: String = row.get(0);
        let address: String = row.get(1);
        let message: String = row.get(2);
        if chain == "ethereum" {
            verify_evm(&message, &request.signature, &address)?;
            let code = self
                .evm_agree("eth_getCode", json!([address, "finalized"]))
                .await?;
            if code.as_str() != Some("0x") {
                return Err(invalid("Ethereum contract wallets are outside the MVP"));
            }
        } else {
            let public = unhex(request.public_key.as_deref().ok_or(Error::Unauthorized)?)?;
            let public: [u8; 32] = public.try_into().map_err(|_| Error::Unauthorized)?;
            self.verify_ever_key(&address, &public).await?;
            let signature = STANDARD
                .decode(&request.signature)
                .map_err(|_| Error::Unauthorized)?;
            let signature = EdSignature::from_slice(&signature).map_err(|_| Error::Unauthorized)?;
            VerifyingKey::from_bytes(&public)
                .map_err(|_| Error::Unauthorized)?
                .verify_strict(&sha2::Sha256::digest(message.as_bytes()), &signature)
                .map_err(|_| Error::Unauthorized)?;
        }
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        if tx.execute("UPDATE otc_wallet_challenges SET consumed_at=now() WHERE id=$1 AND session_id=$2 AND consumed_at IS NULL AND expires_at>now()",&[&request.challenge_id,&session]).await?!=1 {return Err(Error::Unauthorized);}
        tx.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES($1,$2,$3,now()) ON CONFLICT(actor,chain) DO UPDATE SET address=$3,verified_at=now()",&[&actor,&chain,&address]).await?;
        tx.commit().await?;
        Ok(json!({"chain":chain,"address":address,"verified":true}))
    }
    async fn verify_ever_key(&self, address: &str, key: &[u8; 32]) -> Result<()> {
        let query="query($address:String!){accounts(filter:{id:{eq:$address}},limit:1){id acc_type code_hash code data}}";
        let data = self.ever_agree(query, json!({"address":address})).await?;
        let account = data["accounts"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or(Error::Unauthorized)?;
        let code_hash = account["code_hash"].as_str().ok_or(Error::Unauthorized)?;
        if account["id"].as_str() != Some(address)
            || account["acc_type"].as_i64() != Some(1)
            || !self
                .cfg
                .everwallet_code_hashes
                .iter()
                .any(|h| h.eq_ignore_ascii_case(code_hash))
        {
            return Err(invalid("unsupported or inactive EverWallet account"));
        }
        let code = STANDARD
            .decode(account["code"].as_str().ok_or(Error::Unauthorized)?)
            .map_err(|_| Error::Unauthorized)?;
        let cell = everscale_types::boc::Boc::decode(&code).map_err(|_| Error::Unauthorized)?;
        if hex(&cell.repr_hash().0) != code_hash.to_ascii_lowercase() {
            return Err(Error::Unauthorized);
        }
        let data = STANDARD
            .decode(account["data"].as_str().ok_or(Error::Unauthorized)?)
            .map_err(|_| Error::Unauthorized)?;
        let cell = everscale_types::boc::Boc::decode(&data).map_err(|_| Error::Unauthorized)?;
        let actual = cell
            .as_slice()
            .map_err(|_| Error::Unauthorized)?
            .load_u256()
            .map_err(|_| Error::Unauthorized)?;
        if actual.0 != *key {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wallet_addresses_are_chain_specific() {
        assert!(address("ethereum", "0x123").is_err());
        assert!(address("everscale", &format!("0:{}", "a".repeat(64))).is_ok());
        assert!(address("tron", "x").is_err());
    }
}

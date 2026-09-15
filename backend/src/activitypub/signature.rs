use base64::Engine;
use chrono::{DateTime, Utc};
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use sha2::{Digest, Sha256};

use crate::activitypub::actor::ActorIdentity;
use crate::activitypub::error::ActivityPubError;

const REPLAY_WINDOW_SECS: i64 = 300;
const HEADER_ORDER: [&str; 4] = ["(request-target)", "host", "date", "digest"];

pub fn sign_headers(
    identity: &ActorIdentity,
    method: &str,
    uri: &str,
    host: &str,
    body: &[u8],
    now: DateTime<Utc>,
) -> Result<Vec<(String, String)>, ActivityPubError> {
    let date = now.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let digest = format!("SHA-256={}", sha256_b64(body));
    let signing_string = [
        format!("(request-target): {} {}", method.to_lowercase(), uri),
        format!("host: {host}"),
        format!("date: {date}"),
        format!("digest: {digest}"),
    ]
    .join("\n");

    let signature = base64::engine::general_purpose::STANDARD
        .encode(identity.sign_bytes(signing_string.as_bytes()));

    let signature_header = format!(
        "keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date digest\",signature=\"{signature}\"",
        identity.public_key_id()
    );

    Ok(vec![
        ("Host".to_string(), host.to_string()),
        ("Date".to_string(), date),
        ("Digest".to_string(), digest),
        ("Signature".to_string(), signature_header),
    ])
}

/// Verify an HTTP Signature. `uri` must be the raw request-target as received
/// (path + optional query). Returns the signing actor's keyId on success.
pub fn verify(
    headers: &[(&str, &str)],
    method: &str,
    uri: &str,
    body: &[u8],
    public_key_pem: &str,
    now: DateTime<Utc>,
) -> Result<String, ActivityPubError> {
    let sig = parse_signature_header(headers)?;

    // digest verification is independent of the signature headers order
    if let Some((dname, dvalue)) = headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("digest")) {
        let expected = format!("SHA-256={}", sha256_b64(body));
        if !dvalue.eq_ignore_ascii_case(&expected) {
            return Err(ActivityPubError::Signature("digest mismatch".into()));
        }
        let _ = dname;
    } else {
        return Err(ActivityPubError::Signature("missing digest header".into()));
    }

    // replay window on Date
    if let Some((_, dvalue)) = headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("date")) {
        let date = DateTime::parse_from_rfc2822(dvalue)
            .map_err(|e| ActivityPubError::Signature(format!("bad date header: {e}")))?;
        let age = (now - date.with_timezone(&Utc)).num_seconds().abs();
        if age > REPLAY_WINDOW_SECS {
            return Err(ActivityPubError::Signature(format!(
                "date outside replay window ({age}s)"
            )));
        }
    } else if sig.headers.iter().any(|h| h == "date") {
        return Err(ActivityPubError::Signature("missing date header".into()));
    }

    let lines = sig
        .headers
        .iter()
        .map(|h| {
            let value = if h == "(request-target)" {
                format!("{} {}", method.to_lowercase(), uri)
            } else {
                headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(h))
                    .map(|(_, v)| v.to_string())
                    .ok_or_else(|| {
                        ActivityPubError::Signature(format!("missing header {h} in signature"))
                    })?
            };
            Ok(format!("{h}: {value}"))
        })
        .collect::<Result<Vec<_>, ActivityPubError>>()?;
    let signing_string = lines.join("\n");

    let raw_sig = base64::engine::general_purpose::STANDARD
        .decode(&sig.signature)
        .map_err(|e| ActivityPubError::Signature(format!("bad base64 signature: {e}")))?;
    let pkcs1_sig = Signature::try_from(raw_sig.as_slice())
        .map_err(|_| ActivityPubError::Signature("invalid pkcs1 signature length".into()))?;
    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
        .map_err(|e| ActivityPubError::Signature(format!("bad public key pem: {e}")))?;
    let verifying_key = VerifyingKey::<Sha256>::new(public_key);
    verifying_key
        .verify(signing_string.as_bytes(), &pkcs1_sig)
        .map_err(|_| ActivityPubError::Signature("rsa-sha256 signature invalid".into()))?;

    Ok(sig.key_id)
}

fn sha256_b64(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
}

struct ParsedSignature {
    key_id: String,
    algorithm: String,
    headers: Vec<String>,
    signature: String,
}

fn parse_signature_header(
    headers: &[(&str, &str)],
) -> Result<ParsedSignature, ActivityPubError> {
    let (_, raw) = headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("signature"))
        .ok_or_else(|| ActivityPubError::Signature("missing signature header".into()))?;

    let mut parsed = ParsedSignature {
        key_id: String::new(),
        algorithm: String::new(),
        headers: HEADER_ORDER.iter().map(|s| s.to_string()).collect(),
        signature: String::new(),
    };
    for part in raw.split(',') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim_matches('"');
        match key.trim() {
            "keyId" => parsed.key_id = value.to_string(),
            "algorithm" => parsed.algorithm = value.to_string(),
            "headers" => {
                parsed.headers = value
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect()
            }
            "signature" => parsed.signature = value.to_string(),
            _ => {}
        }
    }
    if parsed.key_id.is_empty() || parsed.signature.is_empty() {
        return Err(ActivityPubError::Signature(
            "signature header lacks keyId/signature".into(),
        ));
    }
    if !parsed.algorithm.is_empty()
        && !matches!(parsed.algorithm.as_str(), "rsa-sha256" | "hs2019")
    {
        return Err(ActivityPubError::Signature(format!(
            "unsupported algorithm {}",
            parsed.algorithm
        )));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn roundtrip_sign_and_verify() {
        let identity = ActorIdentity::load_or_create(
            "C:\\Users\\pasaz\\AppData\\Local\\Temp\\opencode\\ap_sig_test_key.pem",
            "https://pay3flow.local",
            "pay3flow",
        )
        .unwrap();
        let now = Utc::now();
        let body = br#"{"type":"Follow"}"#;
        let headers = sign_headers(&identity, "POST", "/inbox/actra", "localhost:7277", body, now)
            .unwrap();
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let key_id = verify(
            &pairs,
            "POST",
            "/inbox/actra",
            body,
            identity.public_key_pem(),
            now,
        )
        .unwrap();
        assert_eq!(key_id, identity.public_key_id());
    }

    #[test]
    fn replay_window_rejects_stale_date() {
        let identity = ActorIdentity::load_or_create(
            "C:\\Users\\pasaz\\AppData\\Local\\Temp\\opencode\\ap_sig_test_key2.pem",
            "https://pay3flow.local",
            "pay3flow",
        )
        .unwrap();
        let now = Utc::now();
        let stale = now - Duration::minutes(10);
        let body = b"{}";
        let headers =
            sign_headers(&identity, "POST", "/x", "h", body, stale).unwrap();
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let err = verify(&pairs, "POST", "/x", body, identity.public_key_pem(), now)
            .unwrap_err();
        assert!(err.to_string().contains("replay"));
    }

    #[test]
    fn digest_mismatch_is_rejected() {
        let identity = ActorIdentity::load_or_create(
            "C:\\Users\\pasaz\\AppData\\Local\\Temp\\opencode\\ap_sig_test_key3.pem",
            "https://pay3flow.local",
            "pay3flow",
        )
        .unwrap();
        let now = Utc::now();
        let body = b"original";
        let headers = sign_headers(&identity, "POST", "/x", "h", body, now).unwrap();
        let pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let err = verify(
            &pairs,
            "POST",
            "/x",
            b"tampered",
            identity.public_key_pem(),
            now,
        )
        .unwrap_err();
        assert!(err.to_string().contains("digest"));
    }
}
use std::path::Path;

use rand::rngs::OsRng;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::signature::{SignatureEncoding, Signer};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use sha2::Sha256;

use crate::activitypub::context::SEC_CONTEXT;
use crate::activitypub::error::ActivityPubError;
use crate::activitypub::model::{ActorDocument, PublicKey, AS_PUBLIC};

pub const KEY_BITS: usize = 2048;

/// The service's federated identity: RSA keypair + canonical IRIs.
#[derive(Clone)]
pub struct ActorIdentity {
    pub actor_id: String,
    pub handle: String,
    pub domain: String,
    pub inbox: String,
    pub outbox: String,
    private_key: SigningKey<Sha256>,
    public_key_pem: String,
}

impl ActorIdentity {
    /// Load the RSA key from `key_path` or generate + persist a fresh one.
    pub fn load_or_create(
        key_path: &str,
        origin: &str,
        handle: &str,
    ) -> Result<Self, ActivityPubError> {
        let path = Path::new(key_path);
        let private_key = if path.exists() {
            let pem = std::fs::read_to_string(path)
                .map_err(|e| ActivityPubError::Key(format!("read {key_path}: {e}")))?;
            RsaPrivateKey::from_pkcs8_pem(&pem)
                .map_err(|e| ActivityPubError::Key(format!("parse pkcs8 pem: {e}")))?
        } else {
            let key = RsaPrivateKey::new(&mut OsRng, KEY_BITS)
                .map_err(|e| ActivityPubError::Key(format!("generate rsa key: {e}")))?;
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        ActivityPubError::Key(format!("create {}: {e}", parent.display()))
                    })?;
                }
            }
            let pem = key
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| ActivityPubError::Key(format!("serialize private key: {e}")))?;
            std::fs::write(key_path, pem.as_bytes())
                .map_err(|e| ActivityPubError::Key(format!("write {key_path}: {e}")))?;
            key
        };

        let public = RsaPublicKey::from(&private_key);
        let public_key_pem = public
            .to_public_key_pem(LineEnding::LF)
            .map_err(|e| ActivityPubError::Key(format!("serialize public key: {e}")))?
            .to_string();

        let actor_id = format!("{origin}/actor/{handle}");
        // Shared inbox: served as `POST /inbox` (also advertised by fmatch as
        // the candidate inbox, so a ticket reaches the same handler).
        let inbox = format!("{origin}/inbox");
        let outbox = format!("{origin}/outbox");

        Ok(Self {
            actor_id,
            handle: handle.to_string(),
            domain: origin
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .trim_end_matches('/')
                .to_string(),
            inbox,
            outbox,
            private_key: SigningKey::<Sha256>::new(private_key),
            public_key_pem,
        })
    }

    pub fn public_key_id(&self) -> String {
        format!("{}#main-key", self.actor_id)
    }

    pub fn public_key_pem(&self) -> &str {
        &self.public_key_pem
    }

    /// The marketplace resource this actor's capability advertises.
    pub fn capability_resource(&self) -> String {
        self.inbox.trim_end_matches("/inbox").to_string() + "/marketplace/resources/acquiring"
    }

    pub fn sign_bytes(&self, message: &[u8]) -> Vec<u8> {
        self.private_key.sign(message).to_vec()
    }

    /// ActivityStreams actor document (fmatch fetches this during Follow).
    pub fn to_document(&self) -> Value {
        let doc = ActorDocument {
            context: json!(["https://www.w3.org/ns/activitystreams", SEC_CONTEXT]),
            id: self.actor_id.clone(),
            atype: vec!["Service".into(), "Application".into()],
            preferred_username: self.handle.clone(),
            name: format!("pay3flow — {}", self.handle),
            summary: "Cross-border payments via acquiring federation.".into(),
            inbox: self.inbox.clone(),
            outbox: self.outbox.clone(),
            url: self.actor_id.clone(),
            endpoints: json!({"sharedInbox": self.inbox}),
            public_key: Some(PublicKey {
                id: self.public_key_id(),
                owner: self.actor_id.clone(),
                public_key_pem: self.public_key_pem.clone(),
            }),
            attachment: vec![
                json!({
                    "type": "Service",
                    "resourceConformsTo": self.capability_resource(),
                    "action": "deliverService",
                    "purpose": "offer",
                    "inbox": self.inbox,
                }),
                json!({ "type": "PropertyValue", "name": "audience", "value": AS_PUBLIC }),
            ],
        };
        serde_json::to_value(doc).unwrap_or_default()
    }

    /// RFC 7033 webfinger response.
    pub fn webfinger(&self, resource: &str) -> Value {
        json!({
            "subject": format!("acct:{}@{}", self.handle, self.domain),
            "aliases": [self.actor_id],
            "links": [
                {
                    "rel": "self",
                    "type": "application/activity+json",
                    "href": self.actor_id,
                },
                {
                    "rel": "http://webfinger.net/rel/profile-page",
                    "type": "text/html",
                    "href": resource,
                }
            ]
        })
    }

    /// Fetch a remote actor document (used when resolving public keys).
    pub async fn fetch_remote_actor(
        &self,
        http: &reqwest::Client,
        iri: &str,
    ) -> Result<ActorDocument, ActivityPubError> {
        let res = http
            .get(iri)
            .header("Accept", "application/activity+json")
            .send()
            .await
            .map_err(|e| ActivityPubError::Http(format!("fetch actor {iri}: {e}")))?;
        if !res.status().is_success() {
            return Err(ActivityPubError::Http(format!(
                "fetch actor {iri}: http {}",
                res.status()
            )));
        }
        res.json()
            .await
            .map_err(|e| ActivityPubError::Http(format!("parse actor {iri}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_key(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("pay3flow-{name}-{}.pem", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn document_and_webfinger_shape() {
        let key_path = temp_key("document");
        let id =
            ActorIdentity::load_or_create(&key_path, "https://pay3flow.local", "pay3flow").unwrap();
        let doc = id.to_document();
        assert_eq!(doc["id"], json!("https://pay3flow.local/actor/pay3flow"));
        assert_eq!(doc["preferredUsername"], json!("pay3flow"));
        assert!(doc["publicKey"]["publicKeyPem"]
            .as_str()
            .unwrap()
            .contains("PUBLIC KEY"));
        assert_eq!(doc["inbox"], json!("https://pay3flow.local/inbox"));
        assert_eq!(
            doc["endpoints"]["sharedInbox"],
            json!("https://pay3flow.local/inbox")
        );

        let cap = &doc["attachment"][0];
        assert_eq!(
            cap["resourceConformsTo"],
            json!("https://pay3flow.local/marketplace/resources/acquiring")
        );
        assert_eq!(cap["action"], json!("deliverService"));
        assert_eq!(cap["purpose"], json!("offer"));

        let wf = id.webfinger("https://pay3flow.local/actor/pay3flow");
        assert_eq!(wf["subject"], json!("acct:pay3flow@pay3flow.local"));
        assert_eq!(wf["links"][0]["rel"], json!("self"));
        std::fs::remove_file(key_path).unwrap();
    }

    #[test]
    fn key_is_persisted_and_reloaded_identically() {
        let key_path = temp_key("persist");
        let a = ActorIdentity::load_or_create(&key_path, "https://a.local", "svc").unwrap();
        let b = ActorIdentity::load_or_create(&key_path, "https://a.local", "svc").unwrap();
        assert_eq!(a.public_key_pem, b.public_key_pem);
        let msg = b"hello";
        assert_eq!(a.sign_bytes(msg), b.sign_bytes(msg));
        std::fs::remove_file(key_path).unwrap();
    }
}

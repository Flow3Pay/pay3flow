//! Wallet sign-in keeps relay identity and delegation on the server.
use super::{
    config::PROFILE,
    error::{invalid, Error, Result},
    wallet::{address, ChallengeRequest, ProofRequest},
    Service,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::GenericClient;
use serde_json::{json, Value};
use uuid::Uuid;

// Serialize account creation and binding for a wallet, including historical accounts.
pub(crate) async fn lock_wallet(
    client: &impl GenericClient,
    chain: &str,
    address: &str,
) -> Result<()> {
    client
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("otc-login:{chain}:{address}")],
        )
        .await?;
    Ok(())
}

pub(crate) async fn wallet_owner(
    client: &impl GenericClient,
    chain: &str,
    address: &str,
) -> Result<Option<String>> {
    let rows = client.query(
        "SELECT actor FROM otc_login_wallets WHERE chain=$1 AND address=$2 UNION SELECT actor FROM otc_wallets WHERE chain=$1 AND lower(address)=$2",
        &[&chain, &address],
    ).await?;
    match rows.as_slice() {
        [] => Ok(None),
        [row] => Ok(Some(row.get(0))),
        _ => Err(Error::Conflict(
            "Wallet has conflicting account mappings; contact support.".into(),
        )),
    }
}

pub(crate) async fn attach_wallet(
    client: &impl GenericClient,
    actor: &str,
    chain: &str,
    address: &str,
) -> Result<()> {
    lock_wallet(client, chain, address).await?;
    if wallet_owner(client, chain, address)
        .await?
        .is_some_and(|owner| owner != actor)
    {
        return Err(Error::Conflict(
            "Wallet already belongs to another OTC account.".into(),
        ));
    }
    client.execute("INSERT INTO otc_login_wallets(chain,address,actor) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&chain, &address, &actor]).await?;
    Ok(())
}

impl Service {
    pub(crate) fn wallet_login_available(&self) -> bool {
        !self.cfg.relay_origin.trim().is_empty()
            && (self.cfg.ethereum_rpc.len() >= 2 || self.cfg.everscale_graphql.len() >= 2)
    }
    pub(crate) async fn login_challenge(&self, request: ChallengeRequest) -> Result<Value> {
        if !self.wallet_login_available() {
            return Err(invalid(
                "Wallet sign-in is not configured. Contact support.",
            ));
        }
        let address = address(&request.chain, &request.address)?;
        let id = Uuid::new_v4();
        let expires = Utc::now() + Duration::minutes(5);
        let message = format!("Pay3Flow OTC sign-in\nOrigin: {}\nChain: {}\nAddress: {address}\nNonce: {id}\nExpires: {expires}\nSign in to your OTC account. This authorizes no transfer.", self.cfg.origin, request.chain);
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        lock_wallet(&tx, &request.chain, &address).await?;
        let count: i64 = tx.query_one("SELECT count(*) FROM otc_login_challenges WHERE chain=$1 AND address=$2 AND expires_at>now()", &[&request.chain, &address]).await?.get(0);
        if count >= 10 {
            return Err(invalid(
                "Too many sign-in requests. Try again in five minutes.",
            ));
        }
        tx.execute("INSERT INTO otc_login_challenges(id,chain,address,message,expires_at) VALUES($1,$2,$3,$4,$5)", &[&id, &request.chain, &address, &message, &expires]).await?;
        tx.commit().await?;
        Ok(
            json!({"id":id,"message":message,"expires_at":expires,"everscale_data":STANDARD.encode(message.as_bytes()),"withSignatureId":false}),
        )
    }

    pub(crate) async fn login_prove(&self, request: ProofRequest) -> Result<Value> {
        if !self.wallet_login_available() {
            return Err(invalid(
                "Wallet sign-in is not configured. Contact support.",
            ));
        }
        let row = self.pool.get().await?.query_opt("SELECT chain,address,message FROM otc_login_challenges WHERE id=$1 AND expires_at>now() AND consumed_at IS NULL", &[&request.challenge_id]).await?.ok_or(Error::Unauthorized)?;
        let chain: String = row.get(0);
        let address: String = row.get(1);
        let message: String = row.get(2);
        self.verify_wallet_signature(&chain, &address, &message, &request)
            .await?;

        // Wallet accounts exist independently of relay availability. Reserve a stable
        // handle locally; federation provisions it only when a trade needs delegation.
        let actor = {
            let mut client = self.pool.get().await?;
            let tx = client.transaction().await?;
            lock_wallet(&tx, &chain, &address).await?;
            let actor = match wallet_owner(&tx, &chain, &address).await? {
                Some(actor) => actor,
                None => {
                    // Inventory wallets cannot acquire desk privileges through public signup.
                    if address == self.cfg.usdt_wallet || address == self.cfg.ever_wallet {
                        return Err(invalid(
                            "The desk account must be onboarded by an operator.",
                        ));
                    }
                    let actor = format!(
                        "{}/actors/otc-{}",
                        self.cfg.relay_origin.trim_end_matches('/'),
                        Uuid::new_v4().simple()
                    );
                    self.relay_actor_url(&actor)?;
                    let encrypted = self.secrets.encrypt("")?;
                    tx.execute("INSERT INTO otc_actor_links(actor,credential,managed,provisioned) VALUES($1,$2,TRUE,FALSE)", &[&actor, &encrypted]).await?;
                    actor
                }
            };
            attach_wallet(&tx, &actor, &chain, &address).await?;
            tx.commit().await?;
            actor
        };

        let id = Uuid::new_v4();
        let expires = Utc::now() + Duration::hours(8);
        let token = self.jwt.sign(&format!("otc:{id}"))?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Both replay races and expiry during provisioning must fail before issuing access.
        if tx.execute("UPDATE otc_login_challenges SET consumed_at=now() WHERE id=$1 AND consumed_at IS NULL AND expires_at>now()", &[&request.challenge_id]).await? != 1 {
            return Err(Error::Unauthorized);
        }
        attach_wallet(&tx, &actor, &chain, &address).await?;
        let credential: String = tx
            .query_opt(
                "SELECT credential FROM otc_actor_links WHERE actor=$1 AND NOT revoked",
                &[&actor],
            )
            .await?
            .ok_or(Error::Unauthorized)?
            .get(0);
        tx.execute("INSERT INTO otc_sessions(id,actor,credential,expires_at,login_chain,login_address) VALUES($1,$2,$3,$4,$5,$6)", &[&id,&actor,&credential,&expires,&chain,&address]).await?;
        tx.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES($1,$2,$3,now()) ON CONFLICT(actor,chain) DO UPDATE SET address=$3,verified_at=now()", &[&actor,&chain,&address]).await?;
        tx.commit().await?;
        drop(client);
        self.session_info(&token).await
    }

    // The relay's signup transport creates an account-scoped federation key.
    // A dedicated advisory lock serializes delegation creation for each account.
    async fn create_login_delegation(&self, actor: &str) -> Result<String> {
        let response = self
            .http
            .post(format!(
                "{}/api/auth/api-key/generate",
                self.cfg.relay_origin.trim_end_matches('/')
            ))
            .json(&json!({"label":format!("Pay3Flow OTC {actor}")}))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(invalid("Wallet account creation failed. Please try again."));
        }
        let body: Value = response.json().await?;
        let credential = body["apiKey"].as_str().ok_or(Error::Unauthorized)?;
        if body["created"] != true
            || body["oratorLinked"] != true
            || !credential.starts_with("lf-")
            || credential.len() < 16
            || credential.len() > 512
            || !credential
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::Unauthorized);
        }
        Ok(credential.to_owned())
    }

    pub(crate) async fn provision_login_actor(&self, actor: &str) -> Result<()> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("otc-relay:{actor}")],
        )
        .await?;
        let row = tx.query_opt("SELECT credential,managed,provisioned FROM otc_actor_links WHERE actor=$1 AND NOT revoked", &[&actor]).await?.ok_or_else(|| invalid("Account relay access must be restored by an operator."))?;
        if row.get::<_, bool>(2) {
            return Ok(());
        }
        if !row.get::<_, bool>(1) {
            return Err(Error::Unauthorized);
        }
        let mut credential = self.secrets.decrypt(&row.get::<_, String>(0))?;
        if credential.is_empty() {
            credential = self.create_login_delegation(actor).await?;
            let encrypted = self.secrets.encrypt(&credential)?;
            tx.execute("UPDATE otc_actor_links SET credential=$2,updated_at=now() WHERE actor=$1 AND NOT revoked", &[&actor, &encrypted]).await?;
        }
        // Persist delegation before claiming so network failures retry with the same key.
        tx.commit().await?;
        drop(client);
        let url = self.relay_actor_url(actor)?;
        let handle = url.rsplit('/').next().ok_or(Error::Unauthorized)?;
        let response = self.http.post(format!("{}/actors", self.cfg.relay_origin.trim_end_matches('/')))
            .bearer_auth(&credential)
            .json(&json!({"@context":"https://www.w3.org/ns/activitystreams","type":"Create","object":{"type":"Service","preferredUsername":handle}}))
            .send().await?;
        if !response.status().is_success() {
            return Err(invalid("Wallet account creation failed. Please try again."));
        }
        let body: Value = response.json().await?;
        if body["id"] != actor {
            return Err(Error::Unauthorized);
        }
        // Verify ownership and the same relay contract required for existing LF sessions.
        let response = self
            .http
            .get(format!("{url}/outbox"))
            .bearer_auth(&credential)
            .send()
            .await?;
        if !response.status().is_success()
            || response
                .headers()
                .get("x-relay-owner")
                .and_then(|v| v.to_str().ok())
                != Some(actor)
            || response
                .headers()
                .get("x-relay-profile")
                .and_then(|v| v.to_str().ok())
                != Some(PROFILE)
            || (!self.cfg.demo
                && response
                    .headers()
                    .get("x-relay-signatures")
                    .and_then(|v| v.to_str().ok())
                    != Some("required"))
        {
            return Err(Error::Unauthorized);
        }
        self.pool.get().await?.execute("UPDATE otc_actor_links SET provisioned=TRUE,updated_at=now() WHERE actor=$1 AND NOT revoked", &[&actor]).await?;
        Ok(())
    }

    pub(crate) async fn session_info(&self, token: &str) -> Result<Value> {
        let (id, actor) = self.session(token).await?;
        let client = self.pool.get().await?;
        let row = client.query_opt("SELECT expires_at,login_chain,login_address FROM otc_sessions WHERE id=$1 AND NOT revoked AND expires_at>now()", &[&id]).await?.ok_or(Error::Unauthorized)?;
        let wallets: Vec<Value> = client.query("SELECT chain,address,verified_at FROM otc_wallets WHERE actor=$1 AND verified_at>now()-interval '24 hours' ORDER BY chain", &[&actor]).await?.iter().map(|r| json!({"chain":r.get::<_,String>(0),"address":r.get::<_,String>(1),"verified_at":r.get::<_,DateTime<Utc>>(2)})).collect();
        let login_chain: Option<String> = row.get(1);
        let login_address: Option<String> = row.get(2);
        let signing_wallet = login_chain
            .zip(login_address)
            .map(|(chain, address)| json!({"chain":chain,"address":address}));
        Ok(
            json!({"token":token,"actor":actor,"desk":actor==self.cfg.desk_actor,"expires_at":row.get::<_,DateTime<Utc>>(0),"signing_wallet":signing_wallet,"wallets":wallets}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey as EverKey};
    use everscale_types::{boc::Boc, cell::CellBuilder};
    use k256::ecdsa::SigningKey;
    use sha2::Digest;
    use sha3::Keccak256;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    fn evm_address(key: &SigningKey) -> String {
        let point = key.verifying_key().to_encoded_point(false);
        format!(
            "0x{}",
            super::super::wallet::hex(&Keccak256::digest(&point.as_bytes()[1..])[12..])
        )
    }
    fn evm_proof(challenge: &Value, key: &SigningKey) -> ProofRequest {
        let message = challenge["message"].as_str().unwrap();
        let prefix = format!("\x19Ethereum Signed Message:\n{}{message}", message.len());
        let (signature, recovery) = key
            .sign_digest_recoverable(Keccak256::new_with_prefix(prefix))
            .unwrap();
        let mut bytes = signature.to_bytes().to_vec();
        bytes.push(recovery.to_byte() + 27);
        ProofRequest {
            challenge_id: Uuid::parse_str(challenge["id"].as_str().unwrap()).unwrap(),
            signature: super::super::wallet::hex(&bytes),
            public_key: None,
        }
    }
    fn ever_proof(challenge: &Value, key: &EverKey) -> ProofRequest {
        ProofRequest {
            challenge_id: Uuid::parse_str(challenge["id"].as_str().unwrap()).unwrap(),
            signature: STANDARD.encode(
                key.sign(&sha2::Sha256::digest(
                    challenge["message"].as_str().unwrap().as_bytes(),
                ))
                .to_bytes(),
            ),
            public_key: Some(super::super::wallet::hex(key.verifying_key().as_bytes())),
        }
    }
    async fn challenge(service: &Service, chain: &str, address: &str) -> Value {
        service
            .login_challenge(ChallengeRequest {
                chain: chain.into(),
                address: address.into(),
            })
            .await
            .unwrap()
    }

    struct Fixture {
        service: Service,
        task: tokio::task::JoinHandle<()>,
        claims: Arc<Mutex<Vec<String>>>,
        keys: Arc<Mutex<std::collections::HashMap<String, String>>>,
        linked: Arc<AtomicBool>,
        fail_claim: Arc<AtomicBool>,
        ever_key: EverKey,
        ever_address: String,
    }
    async fn fixture() -> Fixture {
        let (mut service, old_task, _) = super::super::tests::fixture().await;
        old_task.abort();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let claims = Arc::new(Mutex::new(Vec::new()));
        let fail_claim = Arc::new(AtomicBool::new(false));
        let keys = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let linked = Arc::new(AtomicBool::new(true));
        let ever_key = EverKey::from_bytes(&[11; 32]);
        let ever_address = format!("0:{}", "e".repeat(64));
        let code = CellBuilder::new().build().unwrap();
        let mut data = CellBuilder::new();
        data.store_raw(ever_key.verifying_key().as_bytes(), 256)
            .unwrap();
        let data = data.build().unwrap();
        let account = json!({"id":ever_address,"acc_type":1,"code_hash":super::super::wallet::hex(&code.repr_hash().0),"code":STANDARD.encode(Boc::encode(code)),"data":STANDARD.encode(Boc::encode(data))});
        let cfg = Arc::make_mut(&mut service.cfg);
        cfg.relay_origin = base.clone();
        cfg.desk_actor = format!("{base}/actors/desk");
        cfg.ethereum_rpc = vec![base.clone(), base.clone()];
        cfg.everscale_graphql = vec![base.clone(), base.clone()];
        cfg.everwallet_code_hashes = vec![account["code_hash"].as_str().unwrap().into()];
        let claim_base = base.clone();
        let outbox_base = base;
        let claim_log = claims.clone();
        let fail = fail_claim.clone();
        let generated_keys = keys.clone();
        let claim_keys = keys.clone();
        let outbox_keys = keys.clone();
        let generation_linked = linked.clone();
        let app = axum::Router::new()
            .route("/api/auth/api-key/generate", axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let keys = generated_keys.clone();
                    let linked = generation_linked.clone();
                    async move {
                        assert!(!headers.contains_key("authorization"));
                        assert!(!headers.contains_key("cookie"));
                        let actor = body["label"].as_str().unwrap().strip_prefix("Pay3Flow OTC ").unwrap().to_owned();
                        let credential = format!("lf-{}", Uuid::new_v4().simple());
                        keys.lock().unwrap().insert(credential.clone(), actor);
                        axum::Json(json!({"created":true,"apiKey":credential,"oratorLinked":linked.load(Ordering::Relaxed)}))
                    }
                }))
            .route(
                "/",
                axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                    let account = account.clone();
                    async move {
                        if body.get("query").is_some() {
                            axum::Json(json!({"data":{"accounts":[account]}}))
                        } else {
                            axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":"0x"}))
                        }
                    }
                }),
            )
            .route(
                "/actors",
                axum::routing::post(
                    move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                        let base = claim_base.clone();
                        let claims = claim_log.clone();
                        let fail = fail.clone();
                        let keys = claim_keys.clone();
                        async move {
                            assert_eq!(body["type"], "Create");
                            assert_eq!(body["object"]["type"], "Service");
                            let actor = format!(
                                "{base}/actors/{}",
                                body["object"]["preferredUsername"].as_str().unwrap()
                            );
                            let bearer = headers["authorization"].to_str().unwrap().strip_prefix("Bearer ").unwrap();
                            assert_eq!(keys.lock().unwrap().get(bearer), Some(&actor));
                            claims.lock().unwrap().push(actor.clone());
                            let status = if fail.load(Ordering::Relaxed) {
                                axum::http::StatusCode::SERVICE_UNAVAILABLE
                            } else {
                                axum::http::StatusCode::CREATED
                            };
                            (status, axum::Json(json!({"id":actor})))
                        }
                    },
                ),
            )
            .route(
                "/actors/:handle/outbox",
                axum::routing::get(
                    move |headers: axum::http::HeaderMap, axum::extract::Path(handle): axum::extract::Path<String>| {
                        let actor = format!("{outbox_base}/actors/{handle}");
                        let keys = outbox_keys.clone();
                        async move {
                            let bearer = headers["authorization"].to_str().unwrap().strip_prefix("Bearer ").unwrap();
                            assert_eq!(keys.lock().unwrap().get(bearer), Some(&actor));
                            (
                                [
                                    ("x-relay-owner", actor),
                                    ("x-relay-profile", PROFILE.into()),
                                    ("x-relay-signatures", "required".into()),
                                ],
                                axum::Json(json!({"orderedItems":[]})),
                            )
                        }
                    },
                ),
            );
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Fixture {
            service,
            task,
            claims,
            keys,
            linked,
            fail_claim,
            ever_key,
            ever_address,
        }
    }

    #[tokio::test]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn either_wallet_restores_one_account_and_proofs_cannot_be_replayed() {
        let mut f = fixture().await;
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let address = evm_address(&key);
        let c = challenge(&f.service, "ethereum", &address).await;
        assert!(matches!(
            f.service
                .login_prove(evm_proof(
                    &c,
                    &SigningKey::from_bytes((&[9; 32]).into()).unwrap()
                ))
                .await,
            Err(Error::Unauthorized)
        ));
        let session = f.service.login_prove(evm_proof(&c, &key)).await.unwrap();
        assert_eq!(session["wallets"].as_array().unwrap().len(), 1);
        assert!(!session["desk"].as_bool().unwrap());
        assert!(matches!(
            f.service.login_prove(evm_proof(&c, &key)).await,
            Err(Error::Unauthorized)
        ));
        let token = session["token"].as_str().unwrap();
        let (id, actor) = f.service.session(token).await.unwrap();
        let binding = f
            .service
            .challenge(
                id,
                &actor,
                ChallengeRequest {
                    chain: "everscale".into(),
                    address: f.ever_address.clone(),
                },
            )
            .await
            .unwrap();
        f.service
            .prove(id, &actor, ever_proof(&binding, &f.ever_key))
            .await
            .unwrap();
        Arc::make_mut(&mut f.service.cfg).enabled = false;
        assert!(matches!(f.service.ready().await, Err(Error::Disabled)));
        let c = challenge(&f.service, "everscale", &f.ever_address).await;
        let same = f
            .service
            .login_prove(ever_proof(&c, &f.ever_key))
            .await
            .unwrap();
        assert_eq!(same["actor"], session["actor"]);
        assert_eq!(same["wallets"].as_array().unwrap().len(), 2);
        assert!(f.claims.lock().unwrap().is_empty());
        let c = challenge(&f.service, "ethereum", &address).await;
        let (first, second) = tokio::join!(
            f.service.login_prove(evm_proof(&c, &key)),
            f.service.login_prove(evm_proof(&c, &key))
        );
        assert_ne!(
            first.is_ok(),
            second.is_ok(),
            "only one racing proof can issue a session"
        );
        let client = f.service.pool.get().await.unwrap();
        client
            .execute("UPDATE otc_sessions SET revoked=TRUE WHERE id=$1", &[&id])
            .await
            .unwrap();
        assert!(matches!(
            f.service.session_info(token).await,
            Err(Error::Unauthorized)
        ));
        let credential = f.service.credential(&actor).await.unwrap();
        assert_eq!(f.keys.lock().unwrap().get(&credential), Some(&actor));
        assert!(!session.to_string().contains(&credential));
        drop(client);
        super::super::tests::cleanup(f.service, f.task).await;
    }

    #[tokio::test]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn provisioning_retries_keep_the_handle_and_expired_proofs_issue_no_session() {
        let f = fixture().await;
        let c = challenge(&f.service, "everscale", &f.ever_address).await;
        let mut invalid_signature = ever_proof(&c, &f.ever_key);
        invalid_signature.signature = STANDARD.encode([0u8; 64]);
        assert!(matches!(
            f.service.login_prove(invalid_signature).await,
            Err(Error::Unauthorized)
        ));
        assert!(
            f.claims.lock().unwrap().is_empty(),
            "invalid signatures must not provision an account"
        );
        assert!(
            f.keys.lock().unwrap().is_empty(),
            "invalid signatures must not generate keys"
        );
        f.fail_claim.store(true, Ordering::Relaxed);
        let session = f
            .service
            .login_prove(ever_proof(&c, &f.ever_key))
            .await
            .unwrap();
        let actor = session["actor"].as_str().unwrap();
        assert!(
            f.keys.lock().unwrap().is_empty(),
            "sign-in must not need relay access"
        );
        assert!(matches!(
            f.service.credential(actor).await,
            Err(Error::Invalid(_))
        ));
        f.fail_claim.store(false, Ordering::Relaxed);
        f.service.credential(actor).await.unwrap();
        let claims = f.claims.lock().unwrap().clone();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0], claims[1]);
        let c = challenge(&f.service, "everscale", &f.ever_address).await;
        let client = f.service.pool.get().await.unwrap();
        client
            .execute(
                "UPDATE otc_login_challenges SET expires_at=now()-interval '1 second' WHERE id=$1",
                &[&Uuid::parse_str(c["id"].as_str().unwrap()).unwrap()],
            )
            .await
            .unwrap();
        assert!(matches!(
            f.service.login_prove(ever_proof(&c, &f.ever_key)).await,
            Err(Error::Unauthorized)
        ));
        let token = session["token"].as_str().unwrap();
        let (id, _) = f.service.session(token).await.unwrap();
        client
            .execute(
                "UPDATE otc_sessions SET expires_at=now()-interval '1 second' WHERE id=$1",
                &[&id],
            )
            .await
            .unwrap();
        assert!(matches!(
            f.service.session_info(token).await,
            Err(Error::Unauthorized)
        ));
        drop(client);
        super::super::tests::cleanup(f.service, f.task).await;
    }

    #[tokio::test]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn historical_conflicts_are_not_merged_and_bound_wallets_cannot_be_stolen() {
        let f = fixture().await;
        let mut client = f.service.pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        attach_wallet(&tx, "account-a", "everscale", &f.ever_address)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let tx = client.transaction().await.unwrap();
        assert!(matches!(
            attach_wallet(&tx, "account-b", "everscale", &f.ever_address).await,
            Err(Error::Conflict(_))
        ));
        tx.rollback().await.unwrap();
        for actor in ["account-a", "account-b"] {
            client.execute("INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES($1,'everscale',$2,now())", &[&actor,&f.ever_address]).await.unwrap();
        }
        let c = challenge(&f.service, "everscale", &f.ever_address).await;
        assert!(matches!(
            f.service.login_prove(ever_proof(&c, &f.ever_key)).await,
            Err(Error::Conflict(_))
        ));
        assert!(f.claims.lock().unwrap().is_empty());
        drop(client);
        super::super::tests::cleanup(f.service, f.task).await;
    }

    #[tokio::test]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn concurrent_signup_uses_one_identity_and_inventory_wallets_cannot_signup() {
        let mut f = fixture().await;
        let key = SigningKey::from_bytes((&[19; 32]).into()).unwrap();
        let address = evm_address(&key);
        let a = challenge(&f.service, "ethereum", &address).await;
        let b = challenge(&f.service, "ethereum", &address).await;
        let (a, b) = tokio::join!(
            f.service.login_prove(evm_proof(&a, &key)),
            f.service.login_prove(evm_proof(&b, &key))
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(
            a["actor"], b["actor"],
            "concurrent signup must restore one account"
        );
        let count: i64 = f.service.pool.get().await.unwrap().query_one("SELECT count(DISTINCT actor) FROM otc_login_wallets WHERE chain='ethereum' AND address=$1", &[&address]).await.unwrap().get(0);
        assert_eq!(count, 1);
        let desk_key = SigningKey::from_bytes((&[23; 32]).into()).unwrap();
        let desk_address = evm_address(&desk_key);
        Arc::make_mut(&mut f.service.cfg).usdt_wallet = desk_address.clone();
        let c = challenge(&f.service, "ethereum", &desk_address).await;
        assert!(matches!(
            f.service.login_prove(evm_proof(&c, &desk_key)).await,
            Err(Error::Invalid(_))
        ));
        super::super::tests::cleanup(f.service, f.task).await;
    }

    #[tokio::test]
    #[ignore = "requires OTC_TEST_DATABASE_URL"]
    async fn signup_requires_linked_generated_keys_and_isolates_account_delegations() {
        let f = fixture().await;
        let first_key = SigningKey::from_bytes((&[21; 32]).into()).unwrap();
        let second_key = SigningKey::from_bytes((&[22; 32]).into()).unwrap();
        let first_address = evm_address(&first_key);
        let second_address = evm_address(&second_key);
        let first_challenge = challenge(&f.service, "ethereum", &first_address).await;
        f.linked.store(false, Ordering::Relaxed);
        let first = f
            .service
            .login_prove(evm_proof(&first_challenge, &first_key))
            .await
            .unwrap();
        let first_actor = first["actor"].as_str().unwrap();
        assert!(matches!(
            f.service.credential(first_actor).await,
            Err(Error::Unauthorized)
        ));
        assert!(f.claims.lock().unwrap().is_empty());
        let client = f.service.pool.get().await.unwrap();
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM otc_login_wallets", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        drop(client);
        f.linked.store(true, Ordering::Relaxed);
        let second_challenge = challenge(&f.service, "ethereum", &second_address).await;
        let second = f
            .service
            .login_prove(evm_proof(&second_challenge, &second_key))
            .await
            .unwrap();
        let first_actor = first["actor"].as_str().unwrap();
        let second_actor = second["actor"].as_str().unwrap();
        let first_credential = f.service.credential(first_actor).await.unwrap();
        let second_credential = f.service.credential(second_actor).await.unwrap();
        assert_ne!(first_credential, second_credential);
        let generated = f.keys.lock().unwrap().len();
        let retry_challenge = challenge(&f.service, "ethereum", &first_address).await;
        let restored = f
            .service
            .login_prove(evm_proof(&retry_challenge, &first_key))
            .await
            .unwrap();
        assert_eq!(restored["actor"], first["actor"]);
        assert_eq!(
            f.service.credential(first_actor).await.unwrap(),
            first_credential
        );
        assert_eq!(
            f.keys.lock().unwrap().len(),
            generated,
            "returning wallets must reuse their key"
        );
        let client = f.service.pool.get().await.unwrap();
        let encrypted: String = client
            .query_one(
                "SELECT credential FROM otc_actor_links WHERE actor=$1",
                &[&first_actor],
            )
            .await
            .unwrap()
            .get(0);
        assert_ne!(encrypted, first_credential);
        assert!(!first.to_string().contains(&first_credential));
        drop(client);
        super::super::tests::cleanup(f.service, f.task).await;
    }

    #[test]
    fn login_payloads_reject_credentials_and_extra_actor_fields() {
        assert!(serde_json::from_value::<ChallengeRequest>(
            json!({"chain":"ethereum","address":"0x0123456789012345678901234567890123456789"})
        )
        .is_ok());
        for field in ["actor", "credential"] {
            let mut payload =
                json!({"challenge_id":"4b416df7-a5e8-4e75-89be-a70586b39592","signature":"0x1234"});
            payload[field] = json!("unexpected");
            assert!(
                serde_json::from_value::<ProofRequest>(payload).is_err(),
                "reject legacy {field} in a wallet proof"
            );
        }
    }
}

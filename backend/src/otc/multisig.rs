//! Public agreement bridge to the Rust evernode CLI. Signing stays in EVER Wallet.
use super::{
    error::{invalid, Error, Result},
    Service,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use evernode_multisig::{
    backend::{Assets, Backend},
    domain::Contract,
    otc::{BrowserOperation, Trade},
};
use everscale_types::{boc::Boc, cell::CellBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewRequest {
    pub trade: Trade,
    pub terms_hash: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrepareRequest {
    pub trade: Trade,
    pub terms_hash: String,
    pub operation: BrowserOperation,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReferenceRequest {
    pub transaction_hash: String,
}

fn backend(executable: String, url: Option<String>) -> Backend {
    Backend {
        executable: executable.into(),
        config: None,
        url,
        signature_id: None,
        timeout: Duration::from_secs(10),
    }
}
// Hold the permit inside the blocking task, including after HTTP cancellation.
async fn bounded<T: Send + 'static>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T> {
    static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = LIMIT
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| invalid("Multisig verification is busy; retry shortly"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    })
    .await
    .map_err(|_| invalid("Multisig verifier stopped"))?
    .map_err(|error| Error::Invalid(error.to_string()))
}

impl Service {
    pub(super) fn multisig_available(&self) -> bool {
        !self.cfg.demo
            && self.cfg.everscale_global_id == 42
            && self.cfg.everscale_lag_seconds > 0
            && self.cfg.everscale_lag_seconds <= 600
            && self
                .cfg
                .everscale_multisig_backend
                .as_deref()
                .is_some_and(|p| Path::new(p).is_absolute())
            && self.cfg.everscale_graphql.len() >= 2
            && self.cfg.everscale_graphql[0] != self.cfg.everscale_graphql[1]
            && self
                .cfg
                .everscale_graphql
                .iter()
                .all(|url| url.starts_with("https://"))
            && !self.cfg.everwallet_code_hashes.is_empty()
            && self.wallet_login_available()
    }
    async fn multisig_head(&self) -> Result<()> {
        if !self.multisig_available() {
            return Err(Error::Disabled);
        }
        let data = self.ever_agree("query{blocks(filter:{workchain_id:{eq:-1}},orderBy:[{path:\"seq_no\",direction:DESC}],limit:1){id gen_utime global_id}}", json!({})).await?;
        let blocks = data["blocks"]
            .as_array()
            .ok_or_else(|| invalid("Missing Everscale head"))?;
        if blocks.len() != 1 || blocks[0]["global_id"] != 42 {
            return Err(invalid("Wrong Everscale network"));
        }
        let at = blocks[0]["gen_utime"]
            .as_i64()
            .ok_or_else(|| invalid("Missing Everscale head time"))?;
        let age = i128::from(Utc::now().timestamp()) - i128::from(at);
        if age < -30 || age > i128::from(self.cfg.everscale_lag_seconds) {
            return Err(invalid("Everscale head is stale or in the future"));
        }
        Ok(())
    }
    async fn multisig_verify(
        &self,
        request: ReviewRequest,
        operation: Option<BrowserOperation>,
    ) -> Result<Value> {
        self.multisig_head().await?;
        let value = serde_json::to_value(&request.trade)?;
        if value["terms"]["network_id"] != 42 {
            return Err(invalid("Agreement must use Everscale mainnet (42)"));
        }
        let executable = self
            .cfg
            .everscale_multisig_backend
            .clone()
            .ok_or(Error::Disabled)?;
        let endpoints = self.cfg.everscale_graphql[..2].to_vec();
        bounded(move || {
            let mut reviews = Vec::new();
            for endpoint in endpoints {
                reviews.push(request.trade.browser_review(
                    &backend(executable.clone(), Some(endpoint)),
                    &request.terms_hash,
                    operation.as_ref(),
                )?);
            }
            anyhow::ensure!(
                reviews[0] == reviews[1],
                "Everscale providers disagree about the multisig state"
            );
            Ok(reviews.remove(0))
        })
        .await
    }
    pub(super) async fn multisig_review(&self, request: ReviewRequest) -> Result<Value> {
        let mut review = self.multisig_verify(request, None).await?;
        let address = review["trade"]["wallet_address"]
            .as_str()
            .ok_or_else(|| invalid("Missing multisig address"))?;
        let pending = self.pool.get().await?.query_opt("SELECT id,terms_hash,created_at FROM otc_ever_calls WHERE address=$1 AND state='unknown'", &[&address]).await?;
        review["handoff"] = pending.map(|row| json!({"id":row.get::<_,Uuid>(0),"terms_hash":row.get::<_,String>(1),"created_at":row.get::<_,DateTime<Utc>>(2)})).unwrap_or(Value::Null);
        Ok(review)
    }
    pub(super) async fn multisig_prepare(
        &self,
        actor: &str,
        key: &str,
        request: PrepareRequest,
    ) -> Result<Value> {
        self.multisig_head().await?;
        let wallet = self.wallet(actor, "everscale").await?;
        let public = super::wallet::unhex(request.operation.public_key.as_hex())?;
        self.verify_ever_key(
            &wallet,
            &public.try_into().map_err(|_| Error::Unauthorized)?,
        )
        .await?;
        let identity = serde_json::to_value(&request)?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        // Serialize all preparations, including an HTTP retry and another actor.
        let address = serde_json::to_value(&request.trade)?["wallet_address"]
            .as_str()
            .ok_or_else(|| invalid("Missing wallet address"))?
            .to_owned();
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            &[&format!("ever-multisig:{address}")],
        )
        .await?;
        let paused: bool = tx
            .query_one(
                "SELECT signing_paused FROM otc_controls WHERE singleton=TRUE",
                &[],
            )
            .await?
            .get(0);
        if paused {
            return Err(Error::Disabled);
        }
        if let Some(row) = tx.query_opt("SELECT id,instructions,state,reference FROM otc_ever_calls WHERE actor=$1 AND idempotency_key=$2", &[&actor,&key]).await? {
            let saved: Value = row.get(1);
            if saved["request"] != identity { return Err(Error::Conflict("Idempotency key already has different terms".into())); }
            if row.get::<_,String>(2) == "unknown" {
                let fresh = self.multisig_verify(ReviewRequest { trade: request.trade.clone(), terms_hash: request.terms_hash.clone() }, Some(request.operation.clone())).await?;
                if fresh["call"] != saved["review"]["call"] { return Err(Error::Conflict("Original wallet call changed; signing remains blocked".into())); }
            }
            return Ok(json!({"id":row.get::<_,Uuid>(0),"instructions":saved,"state":row.get::<_,String>(2),"reference":row.get::<_,Option<String>>(3),"resumed":true}));
        }
        if tx
            .query_opt(
                "SELECT id FROM otc_ever_calls WHERE address=$1 AND state='unknown'",
                &[&address],
            )
            .await?
            .is_some()
        {
            return Err(Error::Conflict(
                "An earlier wallet handoff is unresolved. Reconcile it before signing again."
                    .into(),
            ));
        }
        let review = self
            .multisig_verify(
                ReviewRequest {
                    trade: request.trade.clone(),
                    terms_hash: request.terms_hash.clone(),
                },
                Some(request.operation.clone()),
            )
            .await?;
        let hash = review["terms_hash"]
            .as_str()
            .ok_or_else(|| invalid("Missing agreement hash"))?;
        let manifest = serde_json::to_value(&request.trade)?;
        tx.execute("INSERT INTO otc_ever_agreements(address,terms_hash,manifest) VALUES($1,$2,$3) ON CONFLICT(address) DO NOTHING", &[&address,&hash,&manifest]).await?;
        let saved: String = tx
            .query_one(
                "SELECT terms_hash FROM otc_ever_agreements WHERE address=$1",
                &[&address],
            )
            .await?
            .get(0);
        if saved != hash {
            return Err(Error::Conflict(
                "This wallet is already bound to another agreement".into(),
            ));
        }
        let id = Uuid::new_v4();
        let instructions = json!({"request":identity,"review":review});
        tx.execute("INSERT INTO otc_ever_calls(id,address,actor,public_key,terms_hash,instructions,idempotency_key) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&id,&address,&actor,&request.operation.public_key.as_hex(),&hash,&instructions,&key]).await?;
        tx.commit().await?;
        Ok(
            json!({"id":id,"instructions":instructions,"state":"unknown","reference":null,"resumed":false}),
        )
    }
    pub(super) async fn multisig_call(&self, id: Uuid) -> Result<Value> {
        let row = self.pool.get().await?.query_opt("SELECT state,reference,evidence,address,terms_hash FROM otc_ever_calls WHERE id=$1", &[&id]).await?.ok_or(Error::NotFound)?;
        // Agreement, receipts and call ids contain public chain data only.
        Ok(
            json!({"id":id,"state":row.get::<_,String>(0),"reference":row.get::<_,Option<String>>(1),"evidence":row.get::<_,Option<Value>>(2),"address":row.get::<_,String>(3),"terms_hash":row.get::<_,String>(4),"settlement_delivery_verified":false}),
        )
    }
    pub(super) async fn multisig_reference(
        &self,
        id: Uuid,
        request: ReferenceRequest,
    ) -> Result<Value> {
        self.multisig_head().await?;
        if request.transaction_hash.len() != 64
            || !request
                .transaction_hash
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("Expected a 64-digit Everscale transaction hash"));
        }
        let hash = request.transaction_hash.to_ascii_lowercase();
        let row = self.pool.get().await?.query_opt("SELECT address,instructions,created_at,state,reference FROM otc_ever_calls WHERE id=$1", &[&id]).await?.ok_or(Error::NotFound)?;
        let address: String = row.get(0);
        let instructions: Value = row.get(1);
        let created: DateTime<Utc> = row.get(2);
        if row.get::<_, String>(3) != "unknown" {
            if row.get::<_, Option<String>>(4).as_deref() != Some(&hash) {
                return Err(Error::Conflict(
                    "Call already reconciled with another receipt".into(),
                ));
            }
            return self.multisig_call(id).await;
        }
        let data = self.ever_agree("query($id:String!){transactions(filter:{id:{eq:$id}},limit:2){id account_addr status aborted now in_message{id dst msg_type body} compute{success exit_code} action{success result_code}}}", json!({"id":hash})).await?;
        let rows = data["transactions"]
            .as_array()
            .ok_or_else(|| invalid("Missing transaction evidence"))?;
        if rows.len() != 1 {
            return Err(invalid(
                "Original transaction has no unique finalized receipt yet",
            ));
        }
        let receipt = rows[0].clone();
        let body = receipt["in_message"]["body"]
            .as_str()
            .filter(|s| s.len() <= 65536)
            .ok_or_else(|| invalid("Missing inbound message body"))?
            .to_owned();
        verify_external_signature(
            &body,
            instructions["review"]["call"]["publicKey"]
                .as_str()
                .ok_or_else(|| invalid("Missing saved signer"))?,
        )?;
        let executable = self
            .cfg
            .everscale_multisig_backend
            .clone()
            .ok_or(Error::Disabled)?;
        let decoded = bounded(move || {
            let assets = Assets::new(Contract::Safe)?;
            backend(executable, None).run(&[
                "decode".into(),
                "body".into(),
                body,
                "--abi".into(),
                assets.abi.to_string_lossy().into_owned(),
            ])
        })
        .await?;
        let state = receipt_state(
            &receipt,
            &decoded,
            &instructions["review"]["call"],
            &hash,
            &address,
            created.timestamp_millis(),
        )?;
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        let locked = tx
            .query_one(
                "SELECT state,reference FROM otc_ever_calls WHERE id=$1 FOR UPDATE",
                &[&id],
            )
            .await?;
        if locked.get::<_, String>(0) != "unknown"
            && locked.get::<_, Option<String>>(1).as_deref() != Some(&hash)
        {
            return Err(Error::Conflict(
                "Call already reconciled with another receipt".into(),
            ));
        }
        if tx
            .query_opt(
                "SELECT id FROM otc_ever_calls WHERE reference=$1 AND id<>$2",
                &[&hash, &id],
            )
            .await?
            .is_some()
        {
            return Err(Error::Conflict(
                "Receipt already belongs to another handoff".into(),
            ));
        }
        tx.execute("UPDATE otc_ever_calls SET state=$2,reference=$3,evidence=$4 WHERE id=$1 AND state='unknown'", &[&id,&state,&hash,&receipt]).await?;
        tx.commit().await?;
        self.multisig_call(id).await
    }
}

// SafeMultisig's bundled ABI is v2.0: the unsigned remainder cell hash is
// signed with the mainnet global ID prefix. v2.3 destination hashing is not used.
fn verify_external_signature(body: &str, public_key: &str) -> Result<()> {
    let bytes = STANDARD
        .decode(body)
        .map_err(|_| invalid("Invalid inbound body"))?;
    let cell = Boc::decode(bytes).map_err(|_| invalid("Invalid inbound BOC"))?;
    let mut slice = cell.as_slice().map_err(|_| invalid("Invalid body cell"))?;
    if !slice
        .load_bit()
        .map_err(|_| invalid("Missing signature flag"))?
    {
        return Err(invalid(
            "Unsigned receipt cannot reconcile a wallet handoff",
        ));
    }
    let mut signature = [0u8; 64];
    slice
        .load_raw(&mut signature, 512)
        .map_err(|_| invalid("Invalid message signature"))?;
    let payload = CellBuilder::build_from(slice).map_err(|_| invalid("Invalid signed payload"))?;
    let public: [u8; 32] = super::wallet::unhex(public_key)?
        .try_into()
        .map_err(|_| Error::Unauthorized)?;
    let mut signed = Vec::with_capacity(36);
    signed.extend_from_slice(&42i32.to_be_bytes());
    signed.extend_from_slice(&payload.repr_hash().0);
    VerifyingKey::from_bytes(&public)
        .map_err(|_| Error::Unauthorized)?
        .verify_strict(&signed, &Signature::from_bytes(&signature))
        .map_err(|_| invalid("Inbound message signature does not match this owner and network"))?;
    Ok(())
}

fn receipt_state(
    receipt: &Value,
    decoded: &Value,
    call: &Value,
    hash: &str,
    address: &str,
    created_ms: i64,
) -> Result<&'static str> {
    let method = call["payload"]["method"]
        .as_str()
        .ok_or_else(|| invalid("Missing saved method"))?;
    let mut params = decoded["BodyCall"][method].clone();
    if params["payload"] == "te6ccgEBAQEAAgAAAA==" {
        params["payload"] = json!("");
    }
    if receipt["id"] != hash
        || receipt["status"] != 3
        || receipt["account_addr"] != address
        || receipt["in_message"]["dst"] != address
        || receipt["in_message"]["msg_type"] != 1
        || receipt["now"]
            .as_i64()
            .is_none_or(|t| (t as i128) * 1000 < created_ms as i128 - 1000)
        || decoded["Header"]["pubkey"] != call["publicKey"]
        || decoded["Header"]["time"]
            .as_u64()
            .is_none_or(|t| (t as i128) < created_ms as i128)
        || params != call["payload"]["params"]
    {
        return Err(invalid(
            "Receipt does not match the saved wallet, signer, call and handoff time",
        ));
    }
    if receipt["aborted"] == true {
        return Ok("failed");
    }
    if receipt["aborted"] == false
        && receipt["compute"]["success"] == true
        && matches!(receipt["compute"]["exit_code"].as_i64(), Some(0 | 1))
        && (receipt["action"].is_null()
            || (receipt["action"]["success"] == true && receipt["action"]["result_code"] == 0))
    {
        Ok("accepted")
    } else {
        Err(invalid(
            "Transaction outcome is inconclusive; another send remains blocked",
        ))
    }
}

#[cfg(test)]
mod tests;

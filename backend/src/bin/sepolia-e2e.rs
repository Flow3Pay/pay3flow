//! Sepolia end-to-end test for the USD temp-wallet flow.
//!
//! Tests the chain:
//!   funder → temp_usd (temp wallet) → payee
//!
//! Treasury funds gas for the temp wallet. The backend settlement worker
//! handles all signing; this script seeds the DB rows, funds the temp
//! address, and watches until the worker delivers USDT to the payee.
//!
//! Required env vars:
//!   SECRETS_KEY     – same value as the backend uses
//!   FUNDER_KEY      – hex private key (holds Sepolia USDT + ETH)
//!   PAYEE_ADDRESS   – 0x address that should receive the net USDT
//!
//! Optional:
//!   DATABASE_URL    – default: postgres://pay3flow:pay3flow@localhost:5435/pay3flow
//!   SEPOLIA_RPC     – two comma-separated URLs
//!                     (default: publicnode + tenderly)
//!   USDT_CONTRACT   – default: Aave faucet 0xaa8e23fb…
//!   AMOUNT_USDT     – test amount, default "10"

use anyhow::{bail, Context, Result};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use serde_json::{json, Value};
use std::{env, str::FromStr};
use tokio_postgres::NoTls;
use uuid::Uuid;

// alloy types used directly
use alloy::{
    consensus::{SignableTransaction, TxEip1559, TxEnvelope},
    eips::eip2718::Encodable2718,
    network::TxSignerSync,
    primitives::{Address, U256},
    signers::local::PrivateKeySigner,
};

fn signer_from_env() -> anyhow::Result<PrivateKeySigner> {
    // Accept either FUNDER_KEY (hex private key) or MNEMONIC (BIP39 phrase,
    // derives m/44'/60'/0'/0/0).
    if let Ok(hex) = env::var("FUNDER_KEY") {
        let bytes = hex::decode(hex.trim_start_matches("0x"))?;
        return Ok(PrivateKeySigner::from_slice(&bytes)?);
    }
    if let Ok(phrase) = env::var("MNEMONIC") {
        let mn = Mnemonic::parse_in(Language::English, phrase.trim())
            .map_err(|e| anyhow::anyhow!("invalid mnemonic: {e}"))?;
        let seed = mn.to_seed("");
        let path = DerivationPath::from_str("m/44'/60'/0'/0/0")?;
        let derived = XPrv::derive_from_path(&seed, &path)
            .map_err(|e| anyhow::anyhow!("key derivation: {e}"))?;
        let key_bytes: [u8; 32] = derived.private_key().to_bytes().into();
        return Ok(PrivateKeySigner::from_slice(&key_bytes)?);
    }
    anyhow::bail!("set FUNDER_KEY (hex private key) or MNEMONIC (BIP39 phrase)")
}

// temp_wallet primitives — inline to avoid pub(crate) visibility issues
mod wallet {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    use bip32::{DerivationPath, XPrv};
    use rand::{rngs::OsRng, RngCore};
    use sha2::{Digest, Sha256};
    use sha3::Keccak256;
    use std::str::FromStr;
    use zeroize::Zeroizing;

    const INFO: &[u8] = b"otc-temp-wallet-v1";
    const NONCE_LEN: usize = 12;
    pub const SALT_LEN: usize = 32;

    pub struct WalletMaterial {
        seed: Zeroizing<Vec<u8>>,
        pub derivation_index: u32,
    }
    pub struct SealedSeed {
        pub salt: [u8; SALT_LEN],
        pub ciphertext: Vec<u8>,
    }

    impl WalletMaterial {
        pub fn generate() -> anyhow::Result<Self> {
            let mut seed = Zeroizing::new(vec![0u8; 32]);
            OsRng.fill_bytes(seed.as_mut_slice());
            Ok(Self {
                seed,
                derivation_index: 0,
            })
        }

        pub fn seed(&self) -> &[u8] {
            &self.seed
        }

        pub fn address(&self) -> anyhow::Result<String> {
            let path =
                DerivationPath::from_str(&format!("m/44'/60'/0'/0/{}", self.derivation_index))?;
            let derived = XPrv::derive_from_path(self.seed.as_slice(), &path)
                .map_err(|e| anyhow::anyhow!("key derivation: {e}"))?;
            let bytes: [u8; 32] = derived.private_key().to_bytes().into();
            let key = k256::ecdsa::SigningKey::from_bytes((&bytes).into())?;
            let point = key.verifying_key().to_encoded_point(false);
            let hash = Keccak256::digest(&point.as_bytes()[1..]);
            Ok(format!("0x{}", hex::encode(&hash[12..])))
        }
    }

    pub fn seal(secret_key: &str, seed: &[u8]) -> anyhow::Result<SealedSeed> {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let aes = cipher(secret_key, &salt)?;
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let encrypted = aes
            .encrypt(nonce, seed)
            .map_err(|_| anyhow::anyhow!("encryption failed"))?;
        let mut ciphertext = Vec::with_capacity(NONCE_LEN + encrypted.len());
        ciphertext.extend_from_slice(&nonce_bytes);
        ciphertext.extend_from_slice(&encrypted);
        Ok(SealedSeed { salt, ciphertext })
    }

    fn cipher(secret_key: &str, salt: &[u8]) -> anyhow::Result<Aes256Gcm> {
        let mut hasher = Sha256::new();
        hasher.update(secret_key.as_bytes());
        hasher.update(salt);
        hasher.update(INFO);
        let key = hasher.finalize();
        Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("cipher unavailable"))
    }
}

const DEFAULT_DB: &str = "postgres://pay3flow:pay3flow@localhost:5435/pay3flow";
const DEFAULT_RPC: &str =
    "https://ethereum-sepolia-rpc.publicnode.com,https://sepolia.gateway.tenderly.co";
const DEFAULT_USDT: &str = "0xaa8e23fb1079ea71e0a56f48a2aa51851d8433d0";
const CHAIN_ID: u64 = 11_155_111;

// ── tiny amount helpers (no dep on crate internals) ───────────────────────────

fn units(s: &str, decimals: u32) -> Result<u128> {
    let s = s.trim();
    let (int_part, frac_part) = if let Some((i, f)) = s.split_once('.') {
        (i, f)
    } else {
        (s, "")
    };
    let scale = 10u128.pow(decimals);
    let int_val: u128 = if int_part.is_empty() {
        0
    } else {
        int_part.parse()?
    };
    let frac_padded = format!("{:0<width$}", frac_part, width = decimals as usize);
    let frac_val: u128 = if frac_padded.is_empty() {
        0
    } else {
        frac_padded[..decimals as usize].parse()?
    };
    Ok(int_val * scale + frac_val)
}

fn decimal(v: u128, decimals: u32) -> String {
    let scale = 10u128.pow(decimals);
    format!(
        "{}.{:0>width$}",
        v / scale,
        v % scale,
        width = decimals as usize
    )
}

// ── RPC helpers ───────────────────────────────────────────────────────────────

async fn rpc_call(http: &reqwest::Client, url: &str, method: &str, params: Value) -> Result<Value> {
    let resp = http
        .post(url)
        .json(&json!({"jsonrpc":"2.0","method":method,"params":params,"id":1}))
        .send()
        .await?
        .json::<Value>()
        .await?;
    if let Some(e) = resp.get("error") {
        bail!("RPC {method} error from {url}: {e}");
    }
    Ok(resp["result"].clone())
}

fn parse_hex_u128(v: &Value) -> Result<u128> {
    let s = v.as_str().context("expected hex string")?;
    u128::from_str_radix(s.trim_start_matches("0x"), 16).context("hex parse")
}

async fn erc20_balance(
    http: &reqwest::Client,
    rpc_url: &str,
    token: &str,
    holder: &str,
) -> Result<u128> {
    let data = format!(
        "0x70a08231{:0>64}",
        holder.trim_start_matches("0x").to_ascii_lowercase()
    );
    parse_hex_u128(
        &rpc_call(
            http,
            rpc_url,
            "eth_call",
            json!([{"to":token,"data":data},"latest"]),
        )
        .await?,
    )
}

async fn eth_balance(http: &reqwest::Client, rpc_url: &str, addr: &str) -> Result<u128> {
    parse_hex_u128(&rpc_call(http, rpc_url, "eth_getBalance", json!([addr, "latest"])).await?)
}

async fn eth_nonce(http: &reqwest::Client, rpc_url: &str, addr: &str) -> Result<u64> {
    Ok(parse_hex_u128(
        &rpc_call(
            http,
            rpc_url,
            "eth_getTransactionCount",
            json!([addr, "pending"]),
        )
        .await?,
    )? as u64)
}

async fn suggest_fees(http: &reqwest::Client, rpc_url: &str) -> Result<(u128, u128)> {
    let gas_price = parse_hex_u128(&rpc_call(http, rpc_url, "eth_gasPrice", json!([])).await?)?;
    let priority = 1_500_000_000u128; // 1.5 gwei tip
    let max_fee = gas_price.saturating_mul(2).saturating_add(priority);
    Ok((max_fee, priority))
}

fn build_erc20_calldata(to: &str, amount: u128) -> Vec<u8> {
    let selector = hex::decode("a9059cbb").unwrap();
    let to_bytes = hex::decode(to.trim_start_matches("0x")).unwrap();
    let mut data = selector;
    // pad address to 32 bytes
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(&to_bytes);
    // pad amount to 32 bytes
    let amount_bytes = amount.to_be_bytes(); // 16 bytes
    data.extend_from_slice(&[0u8; 16]);
    data.extend_from_slice(&amount_bytes);
    data
}

async fn send_erc20(
    http: &reqwest::Client,
    rpc_url: &str,
    signer: &PrivateKeySigner,
    token: &str,
    to: &str,
    amount: u128,
    nonce: u64,
) -> Result<String> {
    let (max_fee, priority) = suggest_fees(http, rpc_url).await?;
    let calldata = build_erc20_calldata(to, amount);
    let token_addr = Address::from_str(token)?;
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce,
        gas_limit: 200_000,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: priority,
        to: alloy::primitives::TxKind::Call(token_addr),
        value: U256::ZERO,
        access_list: Default::default(),
        input: calldata.into(),
    };
    let sig = signer.sign_transaction_sync(&mut tx)?;
    let envelope = TxEnvelope::from(tx.into_signed(sig));
    let mut raw = Vec::new();
    envelope.encode_2718(&mut raw);
    let result = rpc_call(
        http,
        rpc_url,
        "eth_sendRawTransaction",
        json!([format!("0x{}", hex::encode(&raw))]),
    )
    .await?;
    Ok(result.as_str().unwrap_or("").to_string())
}

async fn wait_mined(http: &reqwest::Client, rpc_url: &str, hash: &str) -> Result<()> {
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        let r = rpc_call(http, rpc_url, "eth_getTransactionReceipt", json!([hash])).await?;
        if !r.is_null() {
            if parse_hex_u128(&r["status"])? == 1 {
                return Ok(());
            }
            bail!("tx {hash} reverted");
        }
    }
    bail!("tx {hash} not mined after 4 min")
}

// ── DB seeding ────────────────────────────────────────────────────────────────

// ── Main ──────────────────────────────────────────────────────────────────────

async fn cmd_mint_usdt() -> Result<()> {
    let rpc = env::var("SEPOLIA_RPC")
        .unwrap_or_else(|_| DEFAULT_RPC.split(',').next().unwrap().to_string());
    let rpc = rpc.split(',').next().unwrap().trim().to_string();
    let usdt = env::var("USDT_CONTRACT")
        .unwrap_or_else(|_| DEFAULT_USDT.into())
        .to_ascii_lowercase();
    let amount: u128 = env::var("AMOUNT_USDT")
        .ok()
        .and_then(|s| units(&s, 6).ok())
        .unwrap_or(10_000_000_000); // 10,000 USDT default for faucet
    let signer = signer_from_env()?;
    let addr   = format!("{:#x}", signer.address());
    let http   = reqwest::Client::new();

    println!("Minting {} USDT to {addr} via Aave Sepolia faucet…", decimal(amount, 6));

    // Aave faucet: mint(address token, address to, uint256 amount) → 0xc6c3bbe6
    let token_padded = usdt.trim_start_matches("0x").to_ascii_lowercase();
    let to_padded    = addr.trim_start_matches("0x").to_ascii_lowercase();
    let amount_hex   = format!("{amount:064x}");
    let calldata_hex = format!("c6c3bbe6{token_padded:0>64}{to_padded:0>64}{amount_hex}");
    let calldata     = hex::decode(&calldata_hex)?;

    let faucet = "0xC959483DBa39aa9E78757139af0e9a2EDEb3f42D";
    let (max_fee, priority) = suggest_fees(&http, &rpc).await?;
    let nonce = eth_nonce(&http, &rpc, &addr).await?;
    let faucet_addr = Address::from_str(faucet)?;
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce,
        gas_limit: 300_000,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: priority,
        to: alloy::primitives::TxKind::Call(faucet_addr),
        value: U256::ZERO,
        access_list: Default::default(),
        input: calldata.into(),
    };
    let sig = signer.sign_transaction_sync(&mut tx)?;
    let envelope = TxEnvelope::from(tx.into_signed(sig));
    let mut raw = Vec::new();
    envelope.encode_2718(&mut raw);

    let hash = rpc_call(&http, &rpc, "eth_sendRawTransaction",
        json!([format!("0x{}", hex::encode(&raw))])).await?;
    let hash = hash.as_str().unwrap_or("").to_string();
    println!("tx: {hash}");
    println!("waiting for confirmation…");
    wait_mined(&http, &rpc, &hash).await?;

    let bal = erc20_balance(&http, &rpc, &usdt, &addr).await?;
    println!("✓ minted — {addr} now has {} USDT", decimal(bal, 6));
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // mint-usdt subcommand: derive key from MNEMONIC and mint via Aave faucet
    if env::args().any(|a| a == "--mint-usdt") {
        return cmd_mint_usdt().await;
    }

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DB.into());
    let secrets_key = env::var("SECRETS_KEY").context("SECRETS_KEY required")?;
    let payee = env::var("PAYEE_ADDRESS")
        .context("PAYEE_ADDRESS required")?
        .to_ascii_lowercase();
    let rpc_urls: Vec<String> = env::var("SEPOLIA_RPC")
        .unwrap_or_else(|_| DEFAULT_RPC.into())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    if rpc_urls.len() < 2 {
        bail!("SEPOLIA_RPC must have two URLs");
    }
    let usdt = env::var("USDT_CONTRACT")
        .unwrap_or_else(|_| DEFAULT_USDT.into())
        .to_ascii_lowercase();
    let amount_str = env::var("AMOUNT_USDT").unwrap_or_else(|_| "10".into());

    let gross = units(&amount_str, 6)?;
    // fee: 0.25% with $5 floor
    let fee = (gross * 25 / 10_000).max(5_000_000);
    let net = gross.saturating_sub(fee);

    let funder_signer = signer_from_env()?;
    let funder_addr = format!("{:#x}", funder_signer.address());

    let http = reqwest::Client::new();

    // connect DB
    let (db, conn) = tokio_postgres::connect(&db_url, NoTls)
        .await
        .context("DB connect")?;
    tokio::spawn(async move {
        let _ = conn.await;
    });

    println!("=== Sepolia USD temp-wallet e2e ===");
    println!("funder : {funder_addr}");
    println!("payee  : {payee}");
    println!(
        "amount : {amount_str} USDT  |  net: {} USDT  |  fee: {} USDT",
        decimal(net, 6),
        decimal(fee, 6)
    );

    // ── [1] preflight ──────────────────────────────────────────────────────────
    println!("\n[1] Checking funder balances…");
    let funder_usdt = erc20_balance(&http, &rpc_urls[0], &usdt, &funder_addr).await?;
    let funder_eth = eth_balance(&http, &rpc_urls[0], &funder_addr).await?;
    println!(
        "    USDT: {}  ETH: {}",
        decimal(funder_usdt, 6),
        decimal(funder_eth, 18)
    );
    if funder_usdt < gross {
        bail!(
            "insufficient USDT: need {}, have {}",
            decimal(gross, 6),
            decimal(funder_usdt, 6)
        );
    }
    if funder_eth < 3_000_000_000_000_000 {
        bail!("insufficient ETH for gas (need ≥0.003)");
    }

    // ── [2] check treasury ─────────────────────────────────────────────────────
    println!("\n[2] Checking treasury…");
    let treasury_row = db.query_opt(
        "SELECT address FROM otc_temp_wallets WHERE role='treasury' AND chain='ethereum' AND status='active'",
        &[],
    ).await?;
    let treasury_addr = treasury_row
        .as_ref()
        .map(|r| r.get::<_, String>("address"))
        .context("treasury wallet not found — start the backend first to provision it")?;
    let treasury_eth  = eth_balance(&http, &rpc_urls[0], &treasury_addr).await?;
    let treasury_usdt = erc20_balance(&http, &rpc_urls[0], &usdt, &treasury_addr).await?;
    println!(
        "    treasury: {treasury_addr}  ETH: {}  USDT: {}",
        decimal(treasury_eth, 18), decimal(treasury_usdt, 6)
    );
    if treasury_eth < 1_000_000_000_000_000 {
        bail!("treasury has less than 0.001 ETH — fund it before running the test");
    }
    // Treasury needs USDT to front same-network payouts. Top it up if needed.
    if treasury_usdt < gross {
        let needed = gross.saturating_sub(treasury_usdt);
        println!("    treasury USDT low — sending {} USDT from funder…", decimal(needed, 6));
        let nonce = eth_nonce(&http, &rpc_urls[0], &funder_addr).await?;
        let htx = send_erc20(&http, &rpc_urls[0], &funder_signer, &usdt, &treasury_addr, needed, nonce).await?;
        println!("    tx: {htx}");
        wait_mined(&http, &rpc_urls[0], &htx).await?;
        println!("    confirmed ✓  treasury now has enough USDT");
    }

    // ── [3] generate temp wallets ──────────────────────────────────────────────
    println!("\n[3] Generating temp wallets…");
    let usd_in_mat = wallet::WalletMaterial::generate()?;
    let (usd_in_id, usd_in_addr) = {
        let addr = usd_in_mat.address()?;
        println!("    usd_in : {addr}");
        (Uuid::new_v4(), addr)
    };

    // ── [4] seed DB ────────────────────────────────────────────────────────────
    println!("\n[4] Seeding DB…");
    let offer_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    let fake_ever = format!("0:{}", "a".repeat(64));

    db.execute(
        "INSERT INTO otc_public_offers
           (id, actor, seller_ever, body, idempotency_key, expires_at)
         VALUES ($1,'https://e2e.local/actors/seller',$2,
                 $3,$4,now()+interval '1 day')",
        &[
            &offer_id,
            &fake_ever,
            &json!({"direction":"sell","ever_amount":"1","usdt_amount":amount_str}),
            &format!("e2e-offer-{offer_id}"),
        ],
    )
    .await
    .context("insert offer")?;

    let trade_body = json!({
        "usdt_units": gross.to_string(),
        "usdt_payee": payee,
        "usdt_payer": funder_addr,
        "stablecoin_network": "sepolia",
        "stablecoin_contract": usdt,
        "temp_wallet": { "token": usdt, "asset_id": null },
        "fee_policy": {
            "net_units": net.to_string(),
            "fee_units": fee.to_string(),
            "rate": "0.0025",
            "floor_units": "5000000"
        }
    });
    db.execute(
        "INSERT INTO otc_p2p_trades
           (id, offer_id, owner_actor, taker_actor, body, state,
            idempotency_key, pay_by, payout_by)
         VALUES ($1,$2,
                 'https://e2e.local/actors/seller',
                 'https://e2e.local/actors/buyer',
                 $3,'delivery_pending',$4,
                 now()+interval '1 hour',
                 now()+interval '2 hours')",
        &[
            &trade_id,
            &offer_id,
            &trade_body,
            &format!("e2e-trade-{trade_id}"),
        ],
    )
    .await
    .context("insert trade")?;
    println!("    trade_id: {trade_id}");

    // insert usd temp wallet
    let sealed = wallet::seal(&secrets_key, usd_in_mat.seed())?;
    db.execute(
        "INSERT INTO otc_temp_wallets
           (id, trade_id, role, chain, address, derivation_index, salt, key_ciphertext)
         VALUES ($1,$2,'usd','sepolia',$3,$4,$5,$6)",
        &[
            &usd_in_id,
            &trade_id,
            &usd_in_addr,
            &(usd_in_mat.derivation_index as i32),
            &sealed.salt.as_slice(),
            &sealed.ciphertext.as_slice(),
        ],
    )
    .await
    .context("insert temp wallet")?;
    println!("    usd wallet inserted: {usd_in_addr}");

    // insert settlement row
    db.execute(
        "INSERT INTO otc_temp_settlements (trade_id, wallet_id) VALUES ($1,$2)",
        &[&trade_id, &usd_in_id],
    )
    .await
    .context("insert settlement")?;
    println!("    settlement row inserted");

    // ── [5] fund usd_in with USDT ──────────────────────────────────────────────
    println!(
        "\n[5] Sending {} USDT to temp wallet {usd_in_addr}…",
        decimal(gross, 6)
    );
    let nonce = eth_nonce(&http, &rpc_urls[0], &funder_addr).await?;
    let hash = send_erc20(
        &http,
        &rpc_urls[0],
        &funder_signer,
        &usdt,
        &usd_in_addr,
        gross,
        nonce,
    )
    .await?;
    println!("    tx: {hash}");
    println!("    waiting for confirmation…");
    wait_mined(&http, &rpc_urls[0], &hash).await?;
    println!("    confirmed ✓");

    // Give the node a moment to process state, then check both RPCs.
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Verify balance landed — check second RPC to avoid stale cache.
    let bal = erc20_balance(&http, &rpc_urls[1], &usdt, &usd_in_addr).await?;
    println!("    usd_in USDT balance: {}", decimal(bal, 6));
    if bal < gross {
        bail!("USDT did not land on usd_in");
    }

    // ── [6] wait for settlement worker ────────────────────────────────────────
    println!("\n[6] Watching settlement worker (up to 10 min)…");
    let payee_before = erc20_balance(&http, &rpc_urls[0], &usdt, &payee).await?;
    println!("    payee USDT before: {}", decimal(payee_before, 6));

    for tick in 1u32..=60 {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let row = db
            .query_one(
                "SELECT state, last_error, reference, fee_reference, attempts
             FROM otc_temp_settlements WHERE trade_id=$1",
                &[&trade_id],
            )
            .await
            .context("query settlement")?;
        let state: String = row.get("state");
        let error: Option<String> = row.get("last_error");
        let reference: Option<String> = row.get("reference");
        let attempts: i32 = row.get("attempts");
        println!(
            "    [{tick:02}] state={state:<20} attempts={attempts:>2}  ref={}  err={}",
            reference.as_deref().unwrap_or("-"),
            error.as_deref().unwrap_or("-"),
        );
        match state.as_str() {
            "settled" => {
                println!("\n    ✓ reached 'settled'");
                break;
            }
            "failed" | "stalled" => bail!(
                "settlement terminal: {state} — {}",
                error.as_deref().unwrap_or("?")
            ),
            _ => {}
        }
        if tick == 60 {
            bail!("timed out after 10 min");
        }
    }

    // ── [7] verify delivery ────────────────────────────────────────────────────
    println!("\n[7] Verifying delivery…");
    let payee_after = erc20_balance(&http, &rpc_urls[0], &usdt, &payee).await?;
    let delivered = payee_after.saturating_sub(payee_before);
    println!(
        "    payee USDT: {} → {} (Δ {})",
        decimal(payee_before, 6),
        decimal(payee_after, 6),
        decimal(delivered, 6)
    );

    let usd_in_final = erc20_balance(&http, &rpc_urls[0], &usdt, &usd_in_addr).await?;
    println!("    usd_in remaining: {}", decimal(usd_in_final, 6));

    if delivered != net {
        bail!(
            "wrong amount: expected {} USDT net, delivered {}",
            decimal(net, 6),
            decimal(delivered, 6)
        );
    }
    println!("\n✓ All checks passed. USD temp-wallet flow works on Sepolia.");
    Ok(())
}

//! Independent-key 2-of-3 OTC wallets using the bundled SafeMultisig contract.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    backend::{Assets, Backend},
    crypto::{self, Keypair},
    domain::{self, Address, Contract, PublicKey, Quorum},
    storage::{self, Store},
    wallet::{self, MessageOptions},
};

// Cell representation hash of the code in the bundled SafeMultisigWallet.tvc.
pub const SAFE_CODE_HASH: &str = "80d6c47c4a25543c9b397b71716f3fae1e2c5d247174c52e2c19bd896442b105";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    pub public_key: PublicKey,
    pub address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Amount(u128);

impl Amount {
    pub fn new(nanotokens: u128) -> Result<Self> {
        ensure!(
            nanotokens >= 1_000_000,
            "Safe multisig transfers require at least 0.001 EVER"
        );
        Ok(Self(nanotokens))
    }
}
impl TryFrom<String> for Amount {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        ensure!(
            !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
            "OTC amount must be an integer nanotoken string"
        );
        Self::new(value.parse().context("OTC amount exceeds uint128")?)
    }
}
impl From<Amount> for String {
    fn from(amount: Amount) -> Self {
        amount.0.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawTerms")]
pub struct Terms {
    trade_id: String,
    network_id: i32,
    buyer: Participant,
    seller: Participant,
    arbitrator: PublicKey,
    deployment_key: PublicKey,
    amount: Amount,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTerms {
    trade_id: String,
    network_id: i32,
    buyer: Participant,
    seller: Participant,
    arbitrator: PublicKey,
    deployment_key: PublicKey,
    amount: Amount,
}

impl TryFrom<RawTerms> for Terms {
    type Error = anyhow::Error;
    fn try_from(raw: RawTerms) -> Result<Self> {
        Self::new(
            raw.trade_id,
            raw.network_id,
            raw.buyer,
            raw.seller,
            raw.arbitrator,
            raw.deployment_key,
            raw.amount,
        )
    }
}

impl Terms {
    pub fn new(
        trade_id: String,
        network_id: i32,
        buyer: Participant,
        seller: Participant,
        arbitrator: PublicKey,
        deployment_key: PublicKey,
        amount: Amount,
    ) -> Result<Self> {
        domain::validate_name(&trade_id)?;
        for key in [
            &buyer.public_key,
            &seller.public_key,
            &arbitrator,
            &deployment_key,
        ] {
            validate_key(key)?;
        }
        let owners = [&buyer.public_key, &seller.public_key, &arbitrator];
        ensure!(
            owners
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 3,
            "OTC requires three distinct buyer, seller and arbitrator public keys"
        );
        ensure!(
            buyer.address != seller.address,
            "OTC release and refund addresses must differ"
        );
        for address in [&buyer.address, &seller.address] {
            ensure!(
                address
                    .to_string()
                    .split_once(':')
                    .is_some_and(|(_, hash)| hash.bytes().any(|b| b != b'0')),
                "OTC recipient must not be the zero address"
            );
        }
        Ok(Self {
            trade_id,
            network_id,
            buyer,
            seller,
            arbitrator,
            deployment_key,
            amount,
        })
    }
}

fn validate_key(key: &PublicKey) -> Result<()> {
    let bytes: [u8; 32] = hex::decode(key.as_hex())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid Ed25519 public key"))?;
    let verifying = VerifyingKey::from_bytes(&bytes).context("Invalid Ed25519 public key")?;
    ensure!(
        !verifying.is_weak(),
        "Weak Ed25519 public keys are not valid OTC participants"
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawTrade")]
pub struct Trade {
    schema_version: u8,
    terms: Terms,
    wallet_address: Address,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTrade {
    schema_version: u8,
    terms: Terms,
    wallet_address: Address,
}

impl TryFrom<RawTrade> for Trade {
    type Error = anyhow::Error;
    fn try_from(raw: RawTrade) -> Result<Self> {
        ensure!(raw.schema_version == 1, "Unsupported OTC schema version");
        raw.wallet_address.workchain()?;
        ensure!(
            raw.wallet_address != raw.terms.buyer.address
                && raw.wallet_address != raw.terms.seller.address,
            "An OTC recipient cannot be the multisig itself"
        );
        Ok(Self {
            schema_version: 1,
            terms: raw.terms,
            wallet_address: raw.wallet_address,
        })
    }
}

impl Trade {
    pub fn id(&self) -> &str {
        &self.terms.trade_id
    }
    pub fn terms_hash(&self) -> Result<String> {
        let mut hash = Sha256::new();
        hash.update(b"evernode-otc/v1\0");
        hash.update(serde_json::to_vec(self)?);
        Ok(hex::encode(hash.finalize()))
    }
    pub fn check_hash(&self, expected: &str) -> Result<()> {
        ensure!(
            self.terms_hash()? == expected.to_ascii_lowercase(),
            "OTC terms hash differs from the independently reviewed agreement"
        );
        Ok(())
    }
    pub fn summary(&self) -> Result<Value> {
        Ok(
            json!({"trade":self,"terms_hash":self.terms_hash()?,"asset":"native EVER","required_signatures":2,"custodians":3}),
        )
    }
    fn wallet(&self) -> Result<wallet::Manifest> {
        Ok(wallet::Manifest {
            schema_version: 1,
            name: self.id().to_owned(),
            mode: wallet::Mode::New,
            contract: Contract::Safe,
            address: self.wallet_address.clone(),
            quorum: Quorum::new(3, 2)?,
            public_keys: vec![
                self.terms.buyer.public_key.clone(),
                self.terms.seller.public_key.clone(),
                self.terms.arbitrator.clone(),
            ],
        })
    }
    fn params(&self, action: Action) -> Value {
        let destination = match action {
            Action::Release => &self.terms.buyer.address,
            Action::Refund => &self.terms.seller.address,
        };
        domain::transfer(destination, self.terms.amount.0, true, "")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Release,
    Refund,
}

pub fn keygen(output: &Path, seed_output: &Path) -> Result<Value> {
    ensure!(
        output != seed_output,
        "Key and seed output paths must differ"
    );
    for file in [output, seed_output] {
        storage::no_symlinks(file)?;
        ensure!(
            !file.try_exists()?,
            "Refusing to overwrite an existing key or seed backup"
        );
        storage::private_metadata(
            file.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
            true,
        )?;
    }
    let phrase = crypto::generate_phrase()?;
    let key = Keypair::derive(&phrase)?;
    let backup = Zeroizing::new(format!("{}\n", phrase.as_str()));
    storage::write_new(seed_output, backup.as_bytes())?;
    storage::write_json(output, &key)?;
    Ok(json!({"public_key":key.public,"key_file":output,"seed_backup":seed_output}))
}

fn ensure_unused(store: &Store, trade: &Trade) -> Result<()> {
    for item in fs::read_dir(store.root())? {
        let item = item?;
        if item.file_type()?.is_dir() && item.path().join("trade.json").try_exists()? {
            let existing: Trade = storage::read_json(&item.path().join("trade.json"))?;
            ensure!(
                existing.wallet_address != trade.wallet_address,
                "This multisig address is already assigned to an OTC trade; use a fresh deployment public key"
            );
        }
    }
    Ok(())
}

pub fn create(store: &Store, backend: &Backend, terms: Terms, workchain: i8) -> Result<Trade> {
    let assets = Assets::new(Contract::Safe)?;
    let address = backend.public_address(&assets, &terms.deployment_key, workchain)?;
    let trade = Trade::try_from(RawTrade {
        schema_version: 1,
        terms,
        wallet_address: address,
    })?;
    save(store, &trade)?;
    Ok(trade)
}

fn save(store: &Store, trade: &Trade) -> Result<()> {
    ensure_unused(store, trade)?;
    let directory = store.begin(trade.id())?;
    storage::write_json(&directory.join("trade.json"), trade)
}

pub fn import(store: &Store, backend: &Backend, file: &Path, expected_hash: &str) -> Result<Trade> {
    let trade: Trade = storage::read_json(file)?;
    trade.check_hash(expected_hash)?;
    check_address(backend, &trade)?;
    save(store, &trade)?;
    Ok(trade)
}

pub fn load(store: &Store, name: &str) -> Result<(PathBuf, Trade)> {
    let directory = store.directory(name)?;
    storage::private_metadata(&directory, true)?;
    let trade: Trade = storage::read_json(&directory.join("trade.json"))?;
    ensure!(
        trade.id() == name,
        "OTC trade ID does not match its directory"
    );
    Ok((directory, trade))
}

fn check_address(backend: &Backend, trade: &Trade) -> Result<()> {
    ensure!(
        backend.public_address(
            &Assets::new(Contract::Safe)?,
            &trade.terms.deployment_key,
            trade.wallet_address.workchain()?
        )? == trade.wallet_address,
        "OTC wallet address differs from its deployment public key"
    );
    Ok(())
}

fn inspect(
    backend: &Backend,
    directory: &Path,
    trade: &Trade,
    pending: bool,
) -> Result<wallet::Inspection> {
    backend.check_network(trade.terms.network_id)?;
    check_address(backend, trade)?;
    let state = wallet::inspect(
        backend,
        &Assets::new(Contract::Safe)?,
        directory,
        &trade.wallet()?,
        pending,
    )?;
    ensure!(
        state.account["address"]
            .as_str()
            .context("Missing OTC account address")?
            .parse::<Address>()?
            == trade.wallet_address,
        "Backend returned a different OTC wallet"
    );
    let code_hash = state.account["code_hash"]
        .as_str()
        .context("Backend returned no OTC wallet code hash")?;
    ensure!(
        code_hash
            .trim_start_matches("0x")
            .eq_ignore_ascii_case(SAFE_CODE_HASH),
        "OTC wallet does not use the pinned SafeMultisig code"
    );
    Ok(state)
}

pub fn status(backend: &Backend, directory: &Path, trade: &Trade) -> Result<Value> {
    let state = inspect(backend, directory, trade, true)?;
    Ok(
        json!({"trade":trade,"terms_hash":trade.terms_hash()?,"verified":true,"balance_nanotokens":state.account["balance"],"transactions":state.transactions,"broadcast_pending_reconciliation":directory.join("broadcast.json").try_exists()?}),
    )
}

fn read_signer(path: &Path) -> Result<Keypair> {
    storage::private_metadata(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
        true,
    )?;
    Keypair::from_json(&storage::read_private(path)?)
}

fn signing_context(
    backend: &mut Backend,
    directory: &Path,
    trade: &Trade,
    hash: &str,
) -> Result<()> {
    trade.check_hash(hash)?;
    ensure!(
        !directory.join("broadcast.json").try_exists()?,
        "A previous broadcast is unresolved; run otc reconcile before signing again"
    );
    let id = trade.terms.network_id.to_string();
    if let Some(value) = &backend.signature_id {
        ensure!(
            value == &id || value == "online",
            "Signature ID differs from the OTC network identity"
        );
    } else {
        backend.signature_id = Some(id);
    }
    Ok(())
}

fn encode_only(options: &MessageOptions) -> MessageOptions {
    MessageOptions {
        output: options.output.clone(),
        broadcast: false,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Broadcast {
    terms_hash: String,
    message_id: String,
    file: PathBuf,
    operation: String,
}

fn broadcast(
    backend: &Backend,
    directory: &Path,
    trade: &Trade,
    operation: &str,
    mut encoded: Value,
    yes: bool,
) -> Result<Value> {
    if yes {
        let marker = Broadcast {
            terms_hash: trade.terms_hash()?,
            message_id: encoded["message_id"]
                .as_str()
                .context("Missing encoded message ID")?
                .to_owned(),
            file: PathBuf::from(
                encoded["file"]
                    .as_str()
                    .context("Missing encoded message file")?,
            ),
            operation: operation.to_owned(),
        };
        storage::write_json(&directory.join("broadcast.json"), &marker)?;
        backend.broadcast(&marker.file)?;
        encoded["status"] = json!("broadcast");
        encoded["next"] = json!("Run otc reconcile before this participant signs again");
    }
    encoded["terms_hash"] = json!(trade.terms_hash()?);
    encoded["operation"] = json!(operation);
    Ok(encoded)
}

pub fn deploy(
    backend: &mut Backend,
    directory: &Path,
    trade: &Trade,
    signer: &Path,
    hash: &str,
    options: MessageOptions,
) -> Result<Value> {
    signing_context(backend, directory, trade, hash)?;
    ensure!(
        read_signer(signer)?.public == trade.terms.deployment_key,
        "Deployment requires this trade's deployment signing key"
    );
    backend.check_network(trade.terms.network_id)?;
    let encoded = wallet::deploy_signed(
        backend,
        directory,
        &trade.wallet()?,
        signer,
        encode_only(&options),
    )?;
    broadcast(
        backend,
        directory,
        trade,
        "deploy",
        encoded,
        options.broadcast,
    )
}

pub struct Signing<'a> {
    pub key: &'a Path,
    pub terms_hash: &'a str,
    pub options: MessageOptions,
}

pub fn propose(
    backend: &mut Backend,
    directory: &Path,
    trade: &Trade,
    action: Action,
    signing: Signing<'_>,
) -> Result<Value> {
    signing_context(backend, directory, trade, signing.terms_hash)?;
    let key = read_signer(signing.key)?;
    let wallet = trade.wallet()?;
    ensure!(
        wallet.public_keys.contains(&key.public),
        "Signer is not an OTC buyer, seller or arbitrator"
    );
    let state = inspect(backend, directory, trade, true)?;
    ensure!(
        state.transactions.is_empty(),
        "An OTC transaction is already pending; review and confirm its ID"
    );
    ensure!(
        wallet::balance(&state.account)? > trade.terms.amount.0,
        "OTC amount must leave a balance for network fees"
    );
    let params = trade.params(action);
    let encoded = wallet::encode_call(
        backend,
        &Assets::new(Contract::Safe)?,
        directory,
        &wallet,
        signing.key,
        ("submitTransaction", &params),
        encode_only(&signing.options),
    )?;
    broadcast(
        backend,
        directory,
        trade,
        "propose",
        encoded,
        signing.options.broadcast,
    )
}

pub fn confirm(
    backend: &mut Backend,
    directory: &Path,
    trade: &Trade,
    action: Action,
    id: u64,
    signing: Signing<'_>,
) -> Result<Value> {
    signing_context(backend, directory, trade, signing.terms_hash)?;
    let params = domain::confirmation(id)?;
    let key = read_signer(signing.key)?;
    let state = inspect(backend, directory, trade, true)?;
    let index = state
        .owners
        .iter()
        .find_map(|(index, owner)| (owner == &key.public).then_some(*index))
        .context("Signer is not an OTC buyer, seller or arbitrator")?;
    let matches = state
        .transactions
        .iter()
        .map(|t| Ok((domain::abi_u64(&t["id"])? == id, t)))
        .collect::<Result<Vec<_>>>()?;
    let matches: Vec<_> = matches
        .into_iter()
        .filter_map(|(same, tx)| same.then_some(tx))
        .collect();
    ensure!(
        matches.len() == 1,
        "Transaction ID must match exactly one pending OTC transaction"
    );
    let tx = matches[0];
    check_transaction(trade, action, tx)?;
    let mask = domain::abi_u64(&tx["confirmationsMask"])?;
    ensure!(
        mask <= 7 && mask.count_ones() == 1 && domain::abi_u64(&tx["signsReceived"])? == 1,
        "Invalid OTC confirmation state"
    );
    ensure!(
        mask & (1u64 << index) == 0,
        "This OTC participant has already signed the transaction"
    );
    let encoded = wallet::encode_call(
        backend,
        &Assets::new(Contract::Safe)?,
        directory,
        &trade.wallet()?,
        signing.key,
        ("confirmTransaction", &params),
        encode_only(&signing.options),
    )?;
    broadcast(
        backend,
        directory,
        trade,
        "confirm",
        encoded,
        signing.options.broadcast,
    )
}

fn check_transaction(trade: &Trade, action: Action, tx: &Value) -> Result<()> {
    let expected = trade.params(action);
    let amount: Amount =
        serde_json::from_value(tx["value"].clone()).context("Invalid OTC transaction amount")?;
    ensure!(
        tx["dest"]
            .as_str()
            .context("Missing OTC destination")?
            .parse::<Address>()?
            == serde_json::from_value::<Address>(expected["dest"].clone())?
            && amount.0 == trade.terms.amount.0,
        "Pending transaction destination or amount differs from OTC terms"
    );
    ensure!(
        domain::abi_u64(&tx["signsRequired"])? == 2
            && domain::abi_u64(&tx["sendFlags"])? == 3
            && tx["bounce"] == true
            && matches!(
                tx["payload"].as_str(),
                Some("") | Some("te6ccgEBAQEAAgAAAA==")
            ),
        "Pending transaction is not the reviewed fixed-value OTC transfer"
    );
    Ok(())
}

/// A public-key-only request for an EVER Wallet external message.
/// No signing material or deployment operation is accepted by this API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserOperation {
    pub public_key: PublicKey,
    pub action: Action,
    pub transaction_id: Option<String>,
}

impl Trade {
    /// Check a deployed agreement and optionally build a reviewed, unsigned call.
    ///
    /// # Errors
    /// Returns an error for changed terms, network, code, owners, threshold,
    /// pending transfers, insufficient balance, or an invalid second approval.
    pub fn browser_review(
        &self,
        backend: &Backend,
        expected_hash: &str,
        operation: Option<&BrowserOperation>,
    ) -> Result<Value> {
        self.check_hash(expected_hash)?;
        let directory = tempfile::tempdir()?;
        let state = inspect(backend, directory.path(), self, true)?;
        browser_state(self, &state, operation)
    }
}

fn browser_state(
    trade: &Trade,
    state: &wallet::Inspection,
    operation: Option<&BrowserOperation>,
) -> Result<Value> {
    let call = if let Some(operation) = operation {
        let index = state
            .owners
            .iter()
            .find_map(|(index, key)| (key == &operation.public_key).then_some(*index))
            .context("Connected public key is not an OTC custodian")?;
        let (method, params) = if let Some(id) = &operation.transaction_id {
            ensure!(
                !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()),
                "Invalid transaction ID"
            );
            let id: u64 = id.parse().context("Transaction ID exceeds uint64")?;
            let matches = state
                .transactions
                .iter()
                .filter(|tx| domain::abi_u64(&tx["id"]).ok() == Some(id))
                .collect::<Vec<_>>();
            ensure!(
                matches.len() == 1,
                "Transaction ID must match exactly one pending transaction"
            );
            let tx = matches[0];
            check_transaction(trade, operation.action, tx)?;
            let mask = domain::abi_u64(&tx["confirmationsMask"])?;
            ensure!(
                mask <= 7 && mask.count_ones() == 1 && domain::abi_u64(&tx["signsReceived"])? == 1,
                "Invalid OTC confirmation state"
            );
            ensure!(
                mask & (1u64 << index) == 0,
                "This participant already approved the transaction"
            );
            ("confirmTransaction", domain::confirmation(id)?)
        } else {
            ensure!(
                state.transactions.is_empty(),
                "Review the existing pending transaction before proposing another"
            );
            ensure!(
                wallet::balance(&state.account)? > trade.terms.amount.0,
                "The wallet must retain a balance for network fees"
            );
            ("submitTransaction", trade.params(operation.action))
        };
        json!({"publicKey":operation.public_key.as_hex(), "recipient":trade.wallet_address,
            "payload":{"abi":include_str!("../assets/SafeMultisigWallet.abi.json"),"method":method,"params":params}})
    } else {
        Value::Null
    };
    let transactions = state
        .transactions
        .iter()
        .map(|tx| {
            let action = if check_transaction(trade, Action::Release, tx).is_ok() {
                Some("release")
            } else if check_transaction(trade, Action::Refund, tx).is_ok() {
                Some("refund")
            } else {
                None
            };
            Ok(
                json!({"id":domain::abi_u64(&tx["id"])?.to_string(),"action":action,
            "destination":tx["dest"],"amount":tx["value"],
            "confirmations_mask":domain::abi_u64(&tx["confirmationsMask"])?.to_string()}),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"trade":trade,"terms_hash":trade.terms_hash()?,"verified":true,
        "balance_nanotokens":wallet::balance(&state.account)?.to_string(),
        "owners":state.owners.iter().map(|(index,key)|json!({"index":index,"public_key":key.as_hex()})).collect::<Vec<_>>(),
        "transactions":transactions,"call":call,"settlement_delivery_verified":false}),
    )
}

pub fn reconcile(backend: &Backend, directory: &Path, trade: &Trade) -> Result<Value> {
    let marker_path = directory.join("broadcast.json");
    let marker: Broadcast = storage::read_json(&marker_path)?;
    trade.check_hash(&marker.terms_hash)?;
    ensure!(
        marker.message_id.len() == 64 && marker.message_id.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid saved broadcast message ID"
    );
    backend.check_network(trade.terms.network_id)?;
    let filter =
        json!({"in_msg":{"eq":marker.message_id},"account_addr":{"eq":trade.wallet_address}});
    let rows = backend.run(&["query-raw".into(), "transactions".into(), "id status aborted account_addr in_message{id dst} compute{success exit_code} action{success result_code}".into(), "--filter".into(), filter.to_string(), "--limit".into(), "2".into()])?;
    let rows = rows
        .as_array()
        .context("Expected OTC transaction evidence array")?;
    ensure!(
        rows.len() == 1,
        "Broadcast has no unique confirmed receipt yet; signing remains blocked"
    );
    let tx = &rows[0];
    ensure!(
        tx["status"] == 3
            && tx["account_addr"].as_str() == Some(&trade.wallet_address.to_string())
            && tx["in_message"]["id"].as_str() == Some(&marker.message_id)
            && tx["in_message"]["dst"].as_str() == Some(&trade.wallet_address.to_string()),
        "Broadcast receipt does not match the saved message or is not finalized"
    );
    let success = tx["aborted"] == false
        && tx["compute"]["success"] == true
        && matches!(domain::abi_u64(&tx["compute"]["exit_code"])?, 0 | 1)
        && (tx["action"].is_null() || tx["action"]["success"] == true);
    // Only a finalized aborted receipt conclusively permits a failed-message retry.
    ensure!(
        success || tx["aborted"] == true,
        "Broadcast outcome is inconclusive; signing remains blocked"
    );
    let result = json!({"operation":marker.operation,"message_id":marker.message_id,"status":if success {"accepted"} else {"failed"},"receipt":tx,"settlement_delivery_verified":false});
    let archive = directory.join(format!("broadcast-{}.json", marker.message_id));
    if archive.try_exists()? {
        ensure!(
            storage::read_json::<Value>(&archive)? == result,
            "Archived broadcast evidence differs"
        );
    } else {
        storage::write_json(&archive, &result)?;
    }
    fs::remove_file(marker_path)?;
    fs::File::open(directory)?.sync_all()?;
    Ok(result)
}

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{
    backend::{self, Assets, Backend},
    crypto::{self, Keypair},
    domain::{self, Address, Contract, PublicKey, Quorum},
    storage::{self, Store},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    New,
    Import,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(try_from = "RawManifest")]
pub struct Manifest {
    pub schema_version: u8,
    pub name: String,
    pub mode: Mode,
    pub contract: Contract,
    pub address: Address,
    pub quorum: Quorum,
    pub public_keys: Vec<PublicKey>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: u8,
    name: String,
    mode: Mode,
    contract: Contract,
    address: Address,
    quorum: Quorum,
    public_keys: Vec<PublicKey>,
}

impl TryFrom<RawManifest> for Manifest {
    type Error = anyhow::Error;
    fn try_from(raw: RawManifest) -> Result<Self> {
        ensure!(raw.schema_version == 1, "Unsupported wallet schema version");
        domain::validate_name(&raw.name)?;
        domain::constructor(&raw.public_keys, raw.quorum)?;
        raw.address.workchain()?;
        Ok(Self {
            schema_version: 1,
            name: raw.name,
            mode: raw.mode,
            contract: raw.contract,
            address: raw.address,
            quorum: raw.quorum,
            public_keys: raw.public_keys,
        })
    }
}

pub struct Preparation {
    pub name: String,
    pub contract: Contract,
    pub quorum: Quorum,
    pub workchain: i8,
}

pub enum SeedSource {
    Generate,
    Recover {
        address: Address,
        files: Vec<PathBuf>,
    },
}

pub fn prepare(
    store: &Store,
    backend: &Backend,
    options: Preparation,
    source: SeedSource,
) -> Result<Manifest> {
    domain::validate_name(&options.name)?;
    let (mode, expected, phrases) = match source {
        SeedSource::Generate => (
            Mode::New,
            None,
            (0..options.quorum.custodians())
                .map(|_| crypto::generate_phrase())
                .collect::<Result<Vec<_>>>()?,
        ),
        SeedSource::Recover { address, files } => {
            backend.require_network()?;
            ensure!(
                files.len() == usize::from(options.quorum.custodians()),
                "Seed file count must match custodian count"
            );
            let phrases = files
                .iter()
                .map(|path| {
                    let bytes = storage::read_private(path)?;
                    let text = std::str::from_utf8(&bytes)
                        .context("Seed file must contain UTF-8 text")?
                        .trim()
                        .trim_matches('"');
                    ensure!(
                        !text.contains(['\n', '\r']),
                        "Seed file must contain a single phrase"
                    );
                    Ok(Zeroizing::new(text.to_owned()))
                })
                .collect::<Result<Vec<_>>>()?;
            (Mode::Import, Some(address), phrases)
        }
    };
    let keypairs = phrases
        .iter()
        .map(|p| Keypair::derive(p))
        .collect::<Result<Vec<_>>>()?;
    let public_keys: Vec<_> = keypairs.iter().map(|key| key.public.clone()).collect();
    domain::constructor(&public_keys, options.quorum)?;
    // Once generated, retain private backups on backend failure. Publish manifest last.
    let directory = store.begin(&options.name)?;
    let backups = directory.join(format!("MSKeys_{}", options.name));
    storage::create_directory(&backups)?;
    for (i, (phrase, key)) in phrases.iter().zip(&keypairs).enumerate() {
        let backup = Zeroizing::new(format!("{}\n", phrase.as_str()));
        storage::write_new(
            &backups.join(format!("{}_seed_{}.txt", options.name, i + 1)),
            backup.as_bytes(),
        )?;
        storage::write_json(
            &backups.join(format!("{}_{}.keys.json", options.name, i + 1)),
            key,
        )?;
    }
    let assets = Assets::new(options.contract)?;
    let address = match expected {
        Some(address) => address,
        None => backend.address(
            &assets,
            &backups.join(format!("{}_1.keys.json", options.name)),
            options.workchain,
        )?,
    };
    let manifest = Manifest::try_from(RawManifest {
        schema_version: 1,
        name: options.name,
        mode,
        contract: options.contract,
        address,
        quorum: options.quorum,
        public_keys,
    })?;
    if mode == Mode::Import {
        inspect(backend, &assets, &directory, &manifest, false)?;
    }
    for (i, key) in keypairs.iter().enumerate() {
        storage::write_json(&key_path(&directory, &manifest.name, i + 1), key)?;
    }
    storage::write_new(
        &directory.join(format!("{}.addr", manifest.name)),
        format!("{}\n", manifest.address).as_bytes(),
    )?;
    storage::write_json(&directory.join("wallet.json"), &manifest)?;
    Ok(manifest)
}

fn key_path(directory: &Path, name: &str, custodian: usize) -> PathBuf {
    directory.join(format!("{name}_{custodian}.keys.json"))
}

pub fn load(store: &Store, name: &str) -> Result<(PathBuf, Manifest)> {
    let directory = store.directory(name)?;
    storage::private_metadata(&directory, true)?;
    let manifest: Manifest = storage::read_json(&directory.join("wallet.json"))?;
    ensure!(
        manifest.name == name,
        "Wallet manifest name does not match its directory"
    );
    let address_file = storage::read_private(&directory.join(format!("{name}.addr")))?;
    let address: Address = std::str::from_utf8(&address_file)
        .context("Invalid address file")?
        .trim()
        .parse()?;
    ensure!(
        address == manifest.address,
        "Wallet address file does not match its manifest"
    );
    for (i, expected) in manifest.public_keys.iter().enumerate() {
        let bytes = storage::read_private(&key_path(&directory, name, i + 1))?;
        let key = Keypair::from_json(&bytes)?;
        ensure!(
            &key.public == expected,
            "Custodian {} key does not match the manifest",
            i + 1
        );
    }
    Ok((directory, manifest))
}

pub(crate) struct Inspection {
    pub(crate) owners: Vec<(u8, PublicKey)>,
    pub(crate) account: Value,
    pub(crate) transactions: Vec<Value>,
}

pub(crate) fn inspect(
    backend: &Backend,
    assets: &Assets,
    directory: &Path,
    wallet: &Manifest,
    pending: bool,
) -> Result<Inspection> {
    let snapshot = tempfile::NamedTempFile::new_in(directory)?;
    let account = backend.snapshot(&wallet.address, snapshot.path())?;
    ensure!(
        account.get("acc_type").and_then(Value::as_str) == Some("Active"),
        "Wallet account is not active"
    );
    if let Some(address) = account.get("address") {
        ensure!(
            address
                .as_str()
                .context("Invalid account address")?
                .parse::<Address>()?
                == wallet.address,
            "Backend returned another account"
        );
    }
    backend::ensure_boc(snapshot.as_file())?;
    let custodians = backend.getter(assets, snapshot.path(), "getCustodians")?;
    let parameters = backend.getter(assets, snapshot.path(), "getParameters")?;
    let owners = custodians
        .get("custodians")
        .and_then(Value::as_array)
        .context("Backend returned no custodian list")?
        .iter()
        .map(|item| {
            let index = domain::abi_u64(&item["index"])?;
            ensure!(index < 31, "Invalid on-chain custodian index");
            Ok((index as u8, PublicKey::from_chain(&item["pubkey"])?))
        })
        .collect::<Result<Vec<_>>>()?;
    let quorum = Quorum::new(
        owners.len() as u64,
        domain::abi_u64(&parameters["requiredTxnConfirms"])?,
    )?;
    ensure!(
        owners.iter().map(|(i, _)| *i).collect::<BTreeSet<_>>()
            == (0..quorum.custodians()).collect(),
        "Invalid on-chain custodian indices"
    );
    let remote_keys: Vec<_> = owners.iter().map(|(_, key)| key.clone()).collect();
    domain::verify_owners(&wallet.public_keys, wallet.quorum, &remote_keys, quorum)?;
    let transactions = if pending {
        backend
            .getter(assets, snapshot.path(), "getTransactions")?
            .get("transactions")
            .and_then(Value::as_array)
            .context("Backend returned no transaction list")?
            .to_owned()
    } else {
        Vec::new()
    };
    Ok(Inspection {
        owners,
        account,
        transactions,
    })
}

pub fn verify(backend: &Backend, directory: &Path, wallet: &Manifest) -> Result<Value> {
    inspect(
        backend,
        &Assets::new(wallet.contract)?,
        directory,
        wallet,
        false,
    )?;
    Ok(
        json!({"address": wallet.address, "custodians": wallet.quorum.custodians(), "required_signatures": wallet.quorum.required(), "public_keys": wallet.public_keys, "verified": true}),
    )
}

pub fn transactions(backend: &Backend, directory: &Path, wallet: &Manifest) -> Result<Value> {
    let state = inspect(
        backend,
        &Assets::new(wallet.contract)?,
        directory,
        wallet,
        true,
    )?;
    Ok(json!({"address": wallet.address, "transactions": state.transactions}))
}

pub(crate) fn balance(account: &Value) -> Result<u128> {
    match account.get("balance") {
        Some(Value::String(s)) if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) => {
            s.parse().context("Balance exceeds uint128")
        }
        Some(Value::Number(n)) if n.is_u64() => {
            Ok(u128::from(n.as_u64().context("Invalid balance")?))
        }
        _ => anyhow::bail!("Backend must report balance as an integer in nanotokens"),
    }
}

pub struct MessageOptions {
    pub output: Option<PathBuf>,
    pub broadcast: bool,
}

pub(crate) fn finish(
    backend: &Backend,
    file: tempfile::NamedTempFile,
    result: &Value,
    options: MessageOptions,
) -> Result<Value> {
    storage::private_metadata(file.path(), false)?;
    backend::ensure_boc(file.as_file())?;
    let id = result
        .get("MessageId")
        .and_then(Value::as_str)
        .context("Backend returned no message ID")?;
    ensure!(
        id.len() == 64 && id.bytes().all(|c| c.is_ascii_hexdigit()),
        "Backend returned an invalid message ID"
    );
    file.as_file().sync_all()?;
    let (file, path) = match options.output {
        Some(path) => {
            storage::no_symlinks(&path)?;
            (
                file.persist_noclobber(&path)
                    .map_err(|e| e.error)
                    .context("Cannot save BOC; existing output files are never overwritten")?,
                path,
            )
        }
        None => file
            .keep()
            .map_err(|e| e.error)
            .context("Cannot retain BOC")?,
    };
    file.sync_all()?;
    if options.broadcast {
        backend.broadcast(&path)?;
    }
    Ok(
        json!({"status": if options.broadcast { "broadcast" } else { "encoded" }, "message_id": id, "file": path}),
    )
}

pub fn deploy(
    backend: &Backend,
    directory: &Path,
    wallet: &Manifest,
    options: MessageOptions,
) -> Result<Value> {
    deploy_signed(
        backend,
        directory,
        wallet,
        &key_path(directory, &wallet.name, 1),
        options,
    )
}

pub(crate) fn deploy_signed(
    backend: &Backend,
    directory: &Path,
    wallet: &Manifest,
    signer: &Path,
    options: MessageOptions,
) -> Result<Value> {
    ensure!(
        wallet.mode == Mode::New,
        "Recovered wallets cannot be deployed"
    );
    backend.require_network()?;
    let assets = Assets::new(wallet.contract)?;
    ensure!(
        backend.address(&assets, signer, wallet.address.workchain()?)? == wallet.address,
        "Deployment key or contract does not match the prepared wallet address"
    );
    let account = backend.run(&["account".into(), wallet.address.to_string()])?;
    let status = account
        .get("acc_type")
        .and_then(Value::as_str)
        .context("Backend returned no account status")?;
    ensure!(
        matches!(status, "Uninit" | "Uninitialized"),
        "Wallet must be funded and uninitialized before deployment (status {status})"
    );
    ensure!(
        balance(&account)? >= 900_000_000,
        "Wallet requires at least 0.9 EVER before deployment"
    );
    let file = tempfile::Builder::new()
        .prefix("deploy-")
        .suffix(".boc")
        .tempfile_in(directory)?;
    let params = domain::constructor(&wallet.public_keys, wallet.quorum)?;
    let mut args = vec![
        "deploy_message".into(),
        backend::path(&assets.tvc),
        params.to_string(),
        "--abi".into(),
        backend::path(&assets.abi),
        "--sign".into(),
        backend::path(signer),
        "--wc".into(),
        wallet.address.workchain()?.to_string(),
        "--raw".into(),
        "--output".into(),
        backend::path(file.path()),
    ];
    backend.signature_args(&mut args);
    let result = backend.run(&args)?;
    finish(backend, file, &result, options)
}

pub struct Transfer {
    pub dest: Address,
    pub amount: u128,
    pub bounce: bool,
    pub payload: String,
    pub custodian: u8,
}

fn signer(directory: &Path, wallet: &Manifest, custodian: u8) -> Result<PathBuf> {
    ensure!(
        (1..=wallet.quorum.custodians()).contains(&custodian),
        "Custodian must be in 1..={}",
        wallet.quorum.custodians()
    );
    Ok(key_path(directory, &wallet.name, usize::from(custodian)))
}

pub fn submit(
    backend: &Backend,
    directory: &Path,
    wallet: &Manifest,
    transfer: Transfer,
    options: MessageOptions,
) -> Result<Value> {
    ensure!(transfer.amount > 0, "Amount must be positive");
    let key = signer(directory, wallet, transfer.custodian)?;
    let assets = Assets::new(wallet.contract)?;
    let state = inspect(backend, &assets, directory, wallet, false)?;
    ensure!(
        balance(&state.account)? > transfer.amount,
        "Transfer must leave a balance for fees"
    );
    let params = domain::transfer(
        &transfer.dest,
        transfer.amount,
        transfer.bounce,
        &transfer.payload,
    );
    encode_call(
        backend,
        &assets,
        directory,
        wallet,
        &key,
        ("submitTransaction", &params),
        options,
    )
}

pub fn confirm(
    backend: &Backend,
    directory: &Path,
    wallet: &Manifest,
    id: u64,
    custodian: u8,
    options: MessageOptions,
) -> Result<Value> {
    let params = domain::confirmation(id)?;
    let key = signer(directory, wallet, custodian)?;
    let assets = Assets::new(wallet.contract)?;
    let state = inspect(backend, &assets, directory, wallet, true)?;
    let matching = state
        .transactions
        .iter()
        .map(|t| Ok((domain::abi_u64(&t["id"])? == id, t)))
        .collect::<Result<Vec<_>>>()?;
    let matching: Vec<_> = matching
        .into_iter()
        .filter_map(|(same, t)| same.then_some(t))
        .collect();
    ensure!(
        matching.len() == 1,
        "Transaction ID must match exactly one pending transaction"
    );
    let owner = &wallet.public_keys[usize::from(custodian - 1)];
    let index = state
        .owners
        .iter()
        .find_map(|(index, key)| (key == owner).then_some(*index))
        .context("Signer is not an on-chain custodian")?;
    let mask = domain::abi_u64(&matching[0]["confirmationsMask"])?;
    ensure!(
        mask <= u64::from(u32::MAX),
        "Invalid transaction confirmation mask"
    );
    ensure!(
        mask & (1u64 << index) == 0,
        "Selected custodian has already confirmed this transaction"
    );
    encode_call(
        backend,
        &assets,
        directory,
        wallet,
        &key,
        ("confirmTransaction", &params),
        options,
    )
}

pub(crate) fn encode_call(
    backend: &Backend,
    assets: &Assets,
    directory: &Path,
    wallet: &Manifest,
    key: &Path,
    (method, params): (&str, &Value),
    options: MessageOptions,
) -> Result<Value> {
    let file = tempfile::Builder::new()
        .prefix(method)
        .suffix(".boc")
        .tempfile_in(directory)?;
    let mut args = vec![
        "message".into(),
        wallet.address.to_string(),
        method.into(),
        params.to_string(),
        "--abi".into(),
        backend::path(&assets.abi),
        "--sign".into(),
        backend::path(key),
        "--raw".into(),
        "--output".into(),
        backend::path(file.path()),
        "--lifetime".into(),
        "600".into(),
    ];
    backend.signature_args(&mut args);
    let result = backend.run(&args)?;
    finish(backend, file, &result, options)
}

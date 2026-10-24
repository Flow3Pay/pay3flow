use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use wait_timeout::ChildExt;

use crate::{
    domain::{Address, Contract},
    storage,
};

pub struct Backend {
    pub executable: PathBuf,
    pub config: Option<PathBuf>,
    pub url: Option<String>,
    pub signature_id: Option<String>,
    pub timeout: Duration,
}

impl Backend {
    pub fn require_network(&self) -> Result<()> {
        ensure!(
            self.url.as_ref().is_some_and(|s| !s.trim().is_empty()),
            "Pass --url (or EVERNODE_URL) explicitly for network operations"
        );
        Ok(())
    }

    pub fn run(&self, args: &[String]) -> Result<Value> {
        let label = args.first().context("Missing backend command")?;
        let mut stdout = tempfile::tempfile().context("Cannot create backend output buffer")?;
        let stderr = tempfile::tempfile().context("Cannot create backend error buffer")?;
        let mut command = Command::new(&self.executable);
        command.arg("-j");
        if let Some(config) = &self.config {
            command.arg("--config").arg(config);
        }
        if let Some(url) = &self.url {
            command.arg("--url").arg(url);
        }
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout.try_clone()?)
            .stderr(stderr);
        let mut child = command.spawn().with_context(|| {
            format!(
                "Cannot start TVM backend {}; install ever-cli or set --backend",
                self.executable.display()
            )
        })?;
        let status = match child.wait_timeout(self.timeout) {
            Ok(Some(status)) => status,
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Backend {label} timed out; check chain state before retrying a broadcast");
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("Cannot wait for TVM backend");
            }
        };
        // Do not echo argv or backend output: either may contain private data.
        ensure!(
            status.success(),
            "Backend {label} failed ({status}); backend output suppressed"
        );
        ensure!(
            stdout.metadata()?.len() <= 4 * 1024 * 1024,
            "Backend {label} returned excessive output"
        );
        stdout.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        stdout.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(serde_json::json!({}));
        }
        let value: Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("Backend {label} returned invalid JSON"))?;
        ensure!(
            value.is_object() || (label == "query-raw" && value.is_array()),
            "Backend {label} returned an unexpected JSON shape"
        );
        ensure!(
            value.get("error").is_none(),
            "Backend {label} returned an error; backend output suppressed"
        );
        Ok(value)
    }

    pub fn address(&self, assets: &Assets, key: &Path, wc: i8) -> Result<Address> {
        let value = self.run(&[
            "genaddr".into(),
            path(&assets.tvc),
            "--abi".into(),
            path(&assets.abi),
            "--setkey".into(),
            path(key),
            "--wc".into(),
            wc.to_string(),
        ])?;
        let address: Address = value
            .get("raw_address")
            .and_then(Value::as_str)
            .context("Backend genaddr returned no raw_address")?
            .parse()?;
        ensure!(
            address.workchain()? == wc,
            "Backend generated an address in a different workchain"
        );
        Ok(address)
    }

    pub fn public_address(
        &self,
        assets: &Assets,
        key: &crate::domain::PublicKey,
        wc: i8,
    ) -> Result<Address> {
        let directory = tempfile::tempdir()?;
        let card = directory.path().join("public.json");
        // genaddr reads only public from the backend KeyPair wire shape.
        // Empty secret deliberately cannot sign and is never passed to --sign.
        storage::write_json(
            &card,
            &serde_json::json!({"public":key.as_hex(),"secret":""}),
        )?;
        self.address(assets, &card, wc)
    }

    pub fn check_network(&self, expected: i32) -> Result<()> {
        self.require_network()?;
        let value = self.run(&[
            "query-raw".into(),
            "blocks".into(),
            "global_id".into(),
            "--filter".into(),
            r#"{"workchain_id":{"eq":-1}}"#.into(),
            "--order".into(),
            r#"[{"path":"seq_no","direction":"DESC"}]"#.into(),
            "--limit".into(),
            "1".into(),
        ])?;
        let rows = value
            .as_array()
            .context("Backend returned no network identity")?;
        ensure!(
            rows.len() == 1 && rows[0]["global_id"].as_i64() == Some(i64::from(expected)),
            "Endpoint network identity differs from OTC terms"
        );
        Ok(())
    }

    pub fn snapshot(&self, address: &Address, file: &Path) -> Result<Value> {
        self.require_network()?;
        self.run(&[
            "account".into(),
            address.to_string(),
            "--dumpboc".into(),
            path(file),
        ])
    }

    pub fn getter(&self, assets: &Assets, boc: &Path, method: &str) -> Result<Value> {
        self.run(&[
            "run".into(),
            "--boc".into(),
            path(boc),
            method.into(),
            "{}".into(),
            "--abi".into(),
            path(&assets.abi),
        ])
    }

    pub fn signature_args(&self, args: &mut Vec<String>) {
        if let Some(id) = &self.signature_id {
            args.extend(["--signature_id".into(), id.to_owned()]);
        }
    }

    pub fn broadcast(&self, file: &Path) -> Result<Value> {
        self.require_network()?;
        storage::private_metadata(file, false)?;
        self.run(&["sendfile".into(), path(file)])
    }
}

pub fn path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

// Assets live in a private temporary directory for the duration of an operation.
pub struct Assets {
    _directory: tempfile::TempDir,
    pub abi: PathBuf,
    pub tvc: PathBuf,
}

impl Assets {
    pub fn new(contract: Contract) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let abi = directory.path().join("wallet.abi.json");
        let tvc = directory.path().join("wallet.tvc");
        let (abi_bytes, tvc_bytes) = contract.assets();
        storage::write_new(&abi, abi_bytes.as_bytes())?;
        storage::write_new(&tvc, tvc_bytes)?;
        Ok(Self {
            _directory: directory,
            abi,
            tvc,
        })
    }
}

pub fn ensure_boc(file: &File) -> Result<()> {
    ensure!(
        file.metadata()?.len() > 4,
        "Backend did not write a message BOC"
    );
    let mut copy = file.try_clone()?;
    copy.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 4];
    copy.read_exact(&mut magic)?;
    ensure!(
        matches!(
            magic,
            [0xb5, 0xee, 0x9c, 0x72] | [0x68, 0xff, 0x65, 0xf3] | [0xac, 0xc3, 0xa7, 0x28]
        ),
        "Backend returned invalid BOC magic"
    );
    Ok(())
}

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

use crate::domain::validate_name;

const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024;

pub fn no_symlinks(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        ensure!(
            !matches!(component, Component::ParentDir),
            "Parent traversal is not allowed in storage paths"
        );
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => ensure!(
                !metadata.file_type().is_symlink(),
                "Symlink paths are not allowed: {}",
                current.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| format!("Cannot inspect {}", current.display()));
            }
        }
    }
    Ok(())
}

pub fn private_metadata(path: &Path, directory: bool) -> Result<()> {
    no_symlinks(path)?;
    let m =
        fs::symlink_metadata(path).with_context(|| format!("Cannot inspect {}", path.display()))?;
    ensure!(
        m.uid() == rustix::process::geteuid().as_raw(),
        "Storage must belong to the current user: {}",
        path.display()
    );
    ensure!(
        m.mode() & 0o077 == 0,
        "Storage must not be accessible to group or other users: {}",
        path.display()
    );
    ensure!(
        if directory {
            m.is_dir()
        } else {
            m.is_file() && m.mode() & 0o111 == 0 && m.nlink() == 1
        },
        "Expected a private {}: {}",
        if directory {
            "directory"
        } else {
            "regular file without hard links"
        },
        path.display()
    );
    Ok(())
}

pub fn create_directory(path: &Path) -> Result<()> {
    no_symlinks(path)?;
    let mut missing = Vec::new();
    let mut ancestor = path;
    while !ancestor.as_os_str().is_empty() && !ancestor.try_exists()? {
        missing.push(ancestor.to_path_buf());
        ancestor = ancestor.parent().unwrap_or(Path::new("."));
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .with_context(|| format!("Cannot create {}", path.display()))?;
    private_metadata(path, true)?;
    for directory in missing {
        File::open(&directory)?.sync_all()?;
        sync_parent(&directory)?;
    }
    Ok(())
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    no_symlinks(path)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("Refusing to overwrite or cannot create {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_parent(path)?;
    Ok(())
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes =
        Zeroizing::new(serde_json::to_vec_pretty(value).context("Cannot encode wallet data")?);
    bytes.push(b'\n');
    write_new(path, &bytes)
}

pub fn read_private(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    private_metadata(path, false)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())?)
        .open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_FILE_SIZE,
        "Wallet file is too large: {}",
        path.display()
    );
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_FILE_SIZE + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE_SIZE,
        "Wallet file is too large"
    );
    Ok(bytes)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read_private(path)?;
    // Do not attach serde's potentially secret input to an error chain.
    serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid wallet metadata in {}", path.display()))
}

pub struct Store {
    root: PathBuf,
    _lock: File,
}

impl Store {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub fn open(root: &Path) -> Result<Self> {
        create_directory(root)?;
        let root = root.canonicalize()?;
        let path = root.join(".lock");
        no_symlinks(&path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())?)
            .open(&path)?;
        private_metadata(&path, false)?;
        lock.try_lock_exclusive()
            .context("Another wallet operation is running in this store")?;
        Ok(Self { root, _lock: lock })
    }

    pub fn directory(&self, name: &str) -> Result<PathBuf> {
        validate_name(name)?;
        Ok(self.root.join(name))
    }

    pub fn begin(&self, name: &str) -> Result<PathBuf> {
        let path = self.directory(name)?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .with_context(|| {
                format!(
                    "Wallet directory already exists or cannot be created: {}",
                    path.display()
                )
            })?;
        sync_parent(&path)?;
        Ok(path)
    }
}

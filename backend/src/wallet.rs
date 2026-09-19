//! Self-hosted EVM wallet support.
//!
//! The keystore stays on the Pay3Flow host. The RPC endpoint only receives
//! already signed transactions and never sees the private key.

use std::path::{Path, PathBuf};

use alloy::{
    network::TransactionBuilder,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::{LocalSigner, PrivateKeySigner},
    sol,
};
use anyhow::{bail, Context, Result};
use serde::Serialize;

sol! {
    #[sol(rpc)]
    contract Erc20 {
        function transfer(address to, uint256 amount) returns (bool);
    }
}

#[derive(Clone)]
pub struct WalletService {
    keystore_path: PathBuf,
    password: String,
    rpc_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletInfo {
    pub address: String,
    pub rpc_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletBalance {
    pub address: String,
    pub wei: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferResult {
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    pub value_wei: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NativeTransferRequest {
    pub to: String,
    /// Amount in wei. Decimal fractions are intentionally rejected.
    pub value_wei: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenTransferRequest {
    pub token_contract: String,
    pub to: String,
    pub amount: String,
}

impl WalletService {
    pub fn new(
        keystore_path: impl Into<PathBuf>,
        password: String,
        rpc_url: String,
    ) -> Result<Self> {
        let keystore_path = keystore_path.into();
        if keystore_path.as_os_str().is_empty() {
            bail!("WALLET_KEYSTORE_PATH is empty")
        }
        if password.is_empty() {
            bail!("WALLET_KEYSTORE_PASSWORD is empty")
        }
        if rpc_url.is_empty() {
            bail!("WALLET_RPC_URL is empty")
        }
        if !keystore_path.is_file() {
            bail!(
                "wallet keystore does not exist: {}",
                keystore_path.display()
            )
        }
        Ok(Self {
            keystore_path,
            password,
            rpc_url,
        })
    }

    pub fn load_from_config(
        keystore_path: String,
        password: String,
        rpc_url: String,
    ) -> Result<Option<Self>> {
        if keystore_path.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(Self::new(keystore_path, password, rpc_url)?))
    }

    fn signer(&self) -> Result<PrivateKeySigner> {
        LocalSigner::decrypt_keystore(&self.keystore_path, &self.password).with_context(|| {
            format!(
                "failed to decrypt wallet keystore at {}",
                self.keystore_path.display()
            )
        })
    }

    async fn provider(&self) -> Result<impl Provider> {
        let signer = self.signer()?;
        Ok(ProviderBuilder::new()
            .wallet(signer)
            .connect(&self.rpc_url)
            .await
            .with_context(|| format!("failed to connect to wallet RPC {}", self.rpc_url))?)
    }

    pub fn info(&self) -> Result<WalletInfo> {
        Ok(WalletInfo {
            address: self.signer()?.address().to_string(),
            rpc_url: self.rpc_url.clone(),
        })
    }

    pub async fn balance(&self) -> Result<WalletBalance> {
        let signer = self.signer()?;
        let address = signer.address();
        let provider = ProviderBuilder::new()
            .connect(&self.rpc_url)
            .await
            .with_context(|| format!("failed to connect to wallet RPC {}", self.rpc_url))?;
        let balance = provider.get_balance(address).await?;
        Ok(WalletBalance {
            address: address.to_string(),
            wei: balance.to_string(),
        })
    }

    pub async fn transfer_native(&self, request: NativeTransferRequest) -> Result<TransferResult> {
        let to: Address = request
            .to
            .parse()
            .context("invalid EVM recipient address")?;
        let value = U256::from_str_radix(&request.value_wei, 10)
            .context("value_wei must be a non-negative decimal integer")?;
        let signer = self.signer()?;
        let from = signer.address();
        let provider = self.provider().await?;
        let tx = TransactionRequest::default().with_to(to).with_value(value);
        let tx_hash = provider.send_transaction(tx).await?.watch().await?;
        Ok(TransferResult {
            tx_hash: tx_hash.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            value_wei: value.to_string(),
        })
    }

    pub async fn transfer_token(&self, request: TokenTransferRequest) -> Result<TransferResult> {
        let token: Address = request
            .token_contract
            .parse()
            .context("invalid ERC-20 token contract address")?;
        let to: Address = request
            .to
            .parse()
            .context("invalid EVM recipient address")?;
        let amount = U256::from_str_radix(&request.amount, 10)
            .context("amount must be a non-negative decimal integer")?;
        let signer = self.signer()?;
        let from = signer.address();
        let provider = self.provider().await?;
        let receipt = Erc20::new(token, provider)
            .transfer(to, amount)
            .send()
            .await?
            .get_receipt()
            .await?;
        Ok(TransferResult {
            tx_hash: receipt.transaction_hash.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            value_wei: amount.to_string(),
        })
    }
}

/// Creates an encrypted keystore for local setup. This is deliberately a
/// separate operation; the HTTP server never creates or replaces a wallet.
pub fn create_keystore(directory: &Path, password: &str) -> Result<(String, String)> {
    std::fs::create_dir_all(directory).with_context(|| {
        format!(
            "failed to create keystore directory {}",
            directory.display()
        )
    })?;
    let mut rng = rand::thread_rng();
    let mut private_key = [0_u8; 32];
    rand::RngCore::fill_bytes(&mut rng, &mut private_key);
    let (wallet, file_name) =
        LocalSigner::encrypt_keystore(directory, &mut rng, private_key, password, None)?;
    Ok((
        wallet.address().to_string(),
        directory.join(file_name).display().to_string(),
    ))
}

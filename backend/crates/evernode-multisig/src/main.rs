use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use evernode_multisig::{
    backend::Backend,
    domain::{self, Address, Contract, Quorum},
    storage::Store,
    wallet::{self, MessageOptions, Preparation, SeedSource},
};
use serde_json::{Value, json};

mod otc_cli;

#[derive(Parser)]
#[command(
    name = "evernode-rs",
    version,
    about = "Everscale OTC and multisig wallet workflows in Rust"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = ".evernode/wallets",
        env = "EVERNODE_WALLET_DIR"
    )]
    wallet_dir: PathBuf,
    /// Compatible ever-cli or tonos-cli executable.
    #[arg(
        long,
        global = true,
        default_value = "ever-cli",
        env = "EVERNODE_BACKEND"
    )]
    backend: PathBuf,
    /// Explicit network endpoint; required for chain reads and broadcasts.
    #[arg(long, global = true, env = "EVERNODE_URL")]
    url: Option<String>,
    /// Backend configuration file (passed through without modification).
    #[arg(long, global = true)]
    backend_config: Option<PathBuf>,
    /// Replay protection ID, or 'online' to fetch it from the network.
    #[arg(long, global = true, allow_hyphen_values = true)]
    signature_id: Option<String>,
    #[arg(long, global = true, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout: u64,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Independent buyer, seller and arbitrator OTC multisig with a fixed 2-of-3 threshold.
    Otc {
        #[command(subcommand)]
        command: otc_cli::Command,
    },
    /// Create, recover, deploy and operate Safe or SetCode multisig wallets.
    #[command(alias = "multisig")]
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
}

#[derive(Args)]
struct Name {
    #[arg(short = 'n', long)]
    name: String,
}

#[derive(Args)]
struct Setup {
    #[command(flatten)]
    name: Name,
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=31))]
    custodians: u8,
    #[arg(long)]
    required_signatures: u8,
    #[arg(long, value_enum, default_value = "safe")]
    contract: Contract,
}

#[derive(Args)]
struct Encoding {
    /// Save the signed raw BOC here; existing files are never overwritten.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Broadcast the encoded message. Without this flag only a BOC is saved.
    #[arg(long)]
    yes: bool,
}

impl From<Encoding> for MessageOptions {
    fn from(value: Encoding) -> Self {
        Self {
            output: value.output,
            broadcast: value.yes,
        }
    }
}

impl From<&Encoding> for MessageOptions {
    fn from(value: &Encoding) -> Self {
        Self {
            output: value.output.clone(),
            broadcast: value.yes,
        }
    }
}

#[derive(Subcommand)]
enum WalletCommand {
    /// Generate native BIP39/Ed25519 keys and calculate the deployment address.
    Create {
        #[command(flatten)]
        setup: Setup,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        workchain: i8,
    },
    /// Recover from private seed files and verify the existing wallet on-chain.
    #[command(alias = "import")]
    Recover {
        #[command(flatten)]
        setup: Setup,
        #[arg(long, allow_hyphen_values = true)]
        address: Address,
        /// Repeat once per custodian, in the desired local signing order.
        #[arg(long, required = true)]
        seed_file: Vec<PathBuf>,
    },
    /// Show public local wallet metadata and backup location.
    Status {
        #[command(flatten)]
        name: Name,
    },
    /// Compare local keys and threshold with a single on-chain state snapshot.
    Verify {
        #[command(flatten)]
        name: Name,
    },
    /// Encode deployment; --yes also broadcasts it to the selected network.
    Deploy {
        #[command(flatten)]
        name: Name,
        #[command(flatten)]
        encoding: Encoding,
    },
    /// List pending multisig transactions from a verified account snapshot.
    Transactions {
        #[command(flatten)]
        name: Name,
    },
    /// Encode submitTransaction; other custodians confirm the resulting ID.
    Submit {
        #[command(flatten)]
        name: Name,
        #[arg(long, allow_hyphen_values = true)]
        dest: Address,
        /// EVER, with at most 9 decimal places (for example 1.000000001).
        #[arg(long, value_parser = parse_amount)]
        amount: u128,
        /// 1-based local key file number.
        #[arg(long, default_value_t = 1)]
        custodian: u8,
        /// Set false when funding an address that has no deployed contract.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        bounce: bool,
        /// ABI cell payload; empty for a simple transfer.
        #[arg(long, default_value = "")]
        payload: String,
        #[command(flatten)]
        encoding: Encoding,
    },
    /// Encode one confirmation after checking the actual on-chain signing mask.
    Confirm {
        #[command(flatten)]
        name: Name,
        #[arg(long)]
        transaction_id: u64,
        #[arg(long)]
        custodian: u8,
        #[command(flatten)]
        encoding: Encoding,
    },
}

fn parse_amount(value: &str) -> std::result::Result<u128, String> {
    domain::nanotokens(value).map_err(|e| e.to_string())
}

fn run(cli: Cli) -> Result<Value> {
    let backend = Backend {
        executable: cli.backend,
        config: cli.backend_config,
        url: cli.url,
        signature_id: cli.signature_id,
        timeout: Duration::from_secs(cli.timeout),
    };
    match cli.command {
        Command::Wallet { command } => run_wallet(command, backend, &cli.wallet_dir),
        Command::Otc { command } => otc_cli::run(command, backend, &cli.wallet_dir),
    }
}

fn run_wallet(
    command: WalletCommand,
    backend: Backend,
    wallet_dir: &std::path::Path,
) -> Result<Value> {
    let store = Store::open(wallet_dir)?;
    match command {
        WalletCommand::Create { setup, workchain } => {
            let options = preparation(setup, workchain)?;
            let wallet = wallet::prepare(&store, &backend, options, SeedSource::Generate)?;
            summary(&store, &wallet)
        }
        WalletCommand::Recover {
            setup,
            address,
            seed_file,
        } => {
            let options = preparation(setup, address.workchain()?)?;
            let wallet = wallet::prepare(
                &store,
                &backend,
                options,
                SeedSource::Recover {
                    address,
                    files: seed_file,
                },
            )?;
            summary(&store, &wallet)
        }
        WalletCommand::Status { name } => {
            let (_, wallet) = wallet::load(&store, &name.name)?;
            summary(&store, &wallet)
        }
        WalletCommand::Verify { name } => {
            let (directory, wallet) = wallet::load(&store, &name.name)?;
            wallet::verify(&backend, &directory, &wallet)
        }
        WalletCommand::Deploy { name, encoding } => {
            let (directory, wallet) = wallet::load(&store, &name.name)?;
            wallet::deploy(&backend, &directory, &wallet, encoding.into())
        }
        WalletCommand::Transactions { name } => {
            let (directory, wallet) = wallet::load(&store, &name.name)?;
            wallet::transactions(&backend, &directory, &wallet)
        }
        WalletCommand::Submit {
            name,
            dest,
            amount,
            custodian,
            bounce,
            payload,
            encoding,
        } => {
            let (directory, wallet) = wallet::load(&store, &name.name)?;
            wallet::submit(
                &backend,
                &directory,
                &wallet,
                wallet::Transfer {
                    dest,
                    amount,
                    custodian,
                    bounce,
                    payload,
                },
                encoding.into(),
            )
        }
        WalletCommand::Confirm {
            name,
            transaction_id,
            custodian,
            encoding,
        } => {
            let (directory, wallet) = wallet::load(&store, &name.name)?;
            wallet::confirm(
                &backend,
                &directory,
                &wallet,
                transaction_id,
                custodian,
                encoding.into(),
            )
        }
    }
}

fn preparation(setup: Setup, workchain: i8) -> Result<Preparation> {
    Ok(Preparation {
        name: setup.name.name,
        contract: setup.contract,
        quorum: Quorum::new(
            u64::from(setup.custodians),
            u64::from(setup.required_signatures),
        )?,
        workchain,
    })
}

fn summary(store: &Store, wallet: &wallet::Manifest) -> Result<Value> {
    let directory = store.directory(&wallet.name)?;
    Ok(
        json!({"wallet": wallet, "directory": directory, "seed_backup_directory": directory.join(format!("MSKeys_{}", wallet.name)), "next": if wallet.mode == wallet::Mode::New { "Back up seed files, fund the address, then run wallet deploy" } else { "Wallet recovered and on-chain custodians verified" }}),
    )
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(value) => {
            println!("{value:#}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

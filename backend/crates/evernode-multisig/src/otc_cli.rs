use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Subcommand};
use evernode_multisig::{
    backend::Backend,
    domain::{Address, PublicKey},
    otc::{self, Action, Amount, Participant, Terms},
    storage::{self, Store},
};
use serde_json::Value;

use super::{Encoding, Name, parse_amount};

#[derive(Args)]
pub struct Signing {
    /// Only this participant's private key file; it is never copied into the trade.
    #[arg(long)]
    signer: PathBuf,
    /// Hash independently reviewed with the other participants.
    #[arg(long)]
    terms_hash: String,
    #[command(flatten)]
    encoding: Encoding,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate one participant or fresh deployment key on its owner's machine.
    Keygen {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        seed_output: PathBuf,
    },
    /// Prepare a native EVER OTC multisig from three independently supplied public keys.
    Create {
        #[command(flatten)]
        name: Name,
        #[arg(long)]
        buyer_pubkey: PublicKey,
        #[arg(long, allow_hyphen_values = true)]
        buyer_address: Address,
        #[arg(long)]
        seller_pubkey: PublicKey,
        #[arg(long, allow_hyphen_values = true)]
        seller_address: Address,
        #[arg(long)]
        arbitrator_pubkey: PublicKey,
        /// Fresh per-trade deployment key, shared only as a public key.
        #[arg(long)]
        deployment_pubkey: PublicKey,
        /// Native EVER; release sends this amount to buyer, refund sends it to seller.
        #[arg(long, value_parser = parse_amount)]
        amount: u128,
        #[arg(long, default_value_t = 42, allow_hyphen_values = true)]
        network_id: i32,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        workchain: i8,
    },
    /// Import a public trade agreement; no seed phrases or signing keys are needed.
    Import {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        terms_hash: String,
    },
    /// Export the trade terms and wallet identity for the other participants.
    Export {
        #[command(flatten)]
        name: Name,
        #[arg(long)]
        output: PathBuf,
    },
    /// Show local terms, wallet address and review hash.
    Status {
        #[command(flatten)]
        name: Name,
    },
    /// Check network, pinned Safe code, three owners, threshold and pending transactions.
    #[command(alias = "transactions", alias = "pending")]
    Verify {
        #[command(flatten)]
        name: Name,
    },
    /// Encode deployment using only the trade's separate deployment signing key.
    Deploy {
        #[command(flatten)]
        name: Name,
        #[command(flatten)]
        signing: Signing,
    },
    /// Propose the agreed release to buyer or refund to seller; a second owner must sign.
    Propose {
        #[command(flatten)]
        name: Name,
        #[arg(long, value_enum)]
        action: Action,
        #[command(flatten)]
        signing: Signing,
    },
    /// Confirm an exact transaction after checking destination, amount, flags and payload.
    Confirm {
        #[command(flatten)]
        name: Name,
        #[arg(long, value_enum)]
        action: Action,
        #[arg(long)]
        transaction_id: u64,
        #[command(flatten)]
        signing: Signing,
    },
    /// Resolve a saved broadcast from finalized message evidence before signing again.
    Reconcile {
        #[command(flatten)]
        name: Name,
    },
}

impl Signing {
    fn request(&self) -> otc::Signing<'_> {
        otc::Signing {
            key: &self.signer,
            terms_hash: &self.terms_hash,
            options: evernode_multisig::wallet::MessageOptions {
                output: self.encoding.output.clone(),
                broadcast: self.encoding.yes,
            },
        }
    }
}

pub fn run(command: Command, mut backend: Backend, root: &Path) -> Result<Value> {
    if let Command::Keygen {
        output,
        seed_output,
    } = command
    {
        return otc::keygen(&output, &seed_output);
    }
    let store = Store::open(&root.join("otc"))?;
    match command {
        Command::Keygen { .. } => unreachable!("keygen returns before store access"),
        Command::Create {
            name,
            buyer_pubkey,
            buyer_address,
            seller_pubkey,
            seller_address,
            arbitrator_pubkey,
            deployment_pubkey,
            amount,
            network_id,
            workchain,
        } => {
            let terms = Terms::new(
                name.name,
                network_id,
                Participant {
                    public_key: buyer_pubkey,
                    address: buyer_address,
                },
                Participant {
                    public_key: seller_pubkey,
                    address: seller_address,
                },
                arbitrator_pubkey,
                deployment_pubkey,
                Amount::new(amount)?,
            )?;
            otc::create(&store, &backend, terms, workchain)?.summary()
        }
        Command::Import { file, terms_hash } => {
            otc::import(&store, &backend, &file, &terms_hash)?.summary()
        }
        Command::Export { name, output } => {
            let (_, trade) = otc::load(&store, &name.name)?;
            storage::write_json(&output, &trade)?;
            trade.summary()
        }
        Command::Status { name } => otc::load(&store, &name.name)?.1.summary(),
        Command::Verify { name } => {
            let (directory, trade) = otc::load(&store, &name.name)?;
            otc::status(&backend, &directory, &trade)
        }
        Command::Deploy { name, signing } => {
            let (directory, trade) = otc::load(&store, &name.name)?;
            otc::deploy(
                &mut backend,
                &directory,
                &trade,
                &signing.signer,
                &signing.terms_hash,
                (&signing.encoding).into(),
            )
        }
        Command::Propose {
            name,
            action,
            signing,
        } => {
            let (directory, trade) = otc::load(&store, &name.name)?;
            otc::propose(&mut backend, &directory, &trade, action, signing.request())
        }
        Command::Confirm {
            name,
            action,
            transaction_id,
            signing,
        } => {
            let (directory, trade) = otc::load(&store, &name.name)?;
            otc::confirm(
                &mut backend,
                &directory,
                &trade,
                action,
                transaction_id,
                signing.request(),
            )
        }
        Command::Reconcile { name } => {
            let (directory, trade) = otc::load(&store, &name.name)?;
            otc::reconcile(&backend, &directory, &trade)
        }
    }
}

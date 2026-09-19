use std::{env, path::PathBuf, process::ExitCode};

use pay3flow_backend::wallet::create_keystore;

fn main() -> ExitCode {
    let Some(directory) = env::args_os().nth(1) else {
        eprintln!("usage: WALLET_KEYSTORE_PASSWORD='...' wallet-init <directory>");
        return ExitCode::FAILURE;
    };
    let Ok(password) = env::var("WALLET_KEYSTORE_PASSWORD") else {
        eprintln!("WALLET_KEYSTORE_PASSWORD must be set in the environment");
        return ExitCode::FAILURE;
    };

    match create_keystore(&PathBuf::from(directory), &password) {
        Ok((address, path)) => {
            println!("address={address}");
            println!("keystore={path}");
            println!("Keep WALLET_KEYSTORE_PASSWORD and this file private.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("wallet initialization failed: {error:#}");
            ExitCode::FAILURE
        }
    }
}

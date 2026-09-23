#[path = "../../providerfile.rs"]
mod providerfile;

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = root.join("providers");
    let destination = root.join("migrations/providers.sql");
    let command = env::args().nth(1).unwrap_or_else(|| "generate".into());

    let sql = providerfile::compile_dir(&source)?;
    match command.as_str() {
        "generate" => {
            fs::write(&destination, sql)?;
            println!("generated {}", destination.display());
        }
        "check" => {
            let current = fs::read_to_string(&destination).map_err(|error| {
                format!(
                    "cannot read {}: {error}; run `cargo run --bin providerfile`",
                    destination.display()
                )
            })?;
            if current != sql {
                return Err(format!(
                    "{} is stale; run `cargo run --bin providerfile`",
                    destination.display()
                )
                .into());
            }
            println!("{} is up to date", destination.display());
        }
        "print" => print!("{sql}"),
        _ => return Err("usage: providerfile [generate|check|print]".into()),
    }
    Ok(())
}

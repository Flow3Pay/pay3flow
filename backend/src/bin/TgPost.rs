use anyhow::{bail, Context, Result};
use teloxide::prelude::Requester;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(text) = args.next() else {
        bail!("usage: cargo run --manifest-path backend/Cargo.toml --bin TgPost -- \"post text\"");
    };
    let text = std::iter::once(text)
        .chain(args)
        .collect::<Vec<_>>()
        .join(" ");

    let token = required_env("TG_BOT_TOKEN")?;
    let channel = required_env("TG_CHANNEL_ID")?;
    let message = teloxide::Bot::new(token)
        .send_message(channel.clone(), text)
        .await
        .with_context(|| format!("failed to publish post to Telegram channel {channel}"))?;

    println!("Published post to {channel} (message {})", message.id.0);
    Ok(())
}

fn required_env(name: &str) -> Result<String> {
    std::env::var(name)
        .with_context(|| format!("{name} must be set in the environment"))
        .map(|value| value.trim().to_owned())
        .and_then(|value| {
            if value.is_empty() {
                bail!("{name} must not be empty");
            }
            Ok(value)
        })
}

use anyhow::{Context, Result};
use teloxide::types::ChatId;

pub(super) struct BotConfig {
    pub token: String,
    pub chat_id: ChatId,
}

impl BotConfig {
    pub fn from_env() -> Result<Option<Self>> {
        let token = non_empty_env("TG_BOT_TOKEN");
        let chat_id = non_empty_env("TG_CHAT_ID");

        match (token, chat_id) {
            (None, None) => Ok(None),
            (Some(token), Some(chat_id)) => {
                let chat_id = chat_id
                    .parse::<i64>()
                    .context("TG_CHAT_ID must be a numeric Telegram chat ID")?;
                Ok(Some(Self {
                    token,
                    chat_id: ChatId(chat_id),
                }))
            }
            (Some(_), None) => {
                tracing::warn!(
                    "TG_BOT_TOKEN is set but TG_CHAT_ID is missing; Telegram notifications are disabled"
                );
                Ok(None)
            }
            (None, Some(_)) => {
                tracing::warn!(
                    "TG_CHAT_ID is set but TG_BOT_TOKEN is missing; Telegram notifications are disabled"
                );
                Ok(None)
            }
        }
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

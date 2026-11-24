use anyhow::Result;
use teloxide::{
    payloads::SendDocumentSetters,
    prelude::Requester,
    types::{ChatId, InputFile},
    Bot,
};
use uuid::Uuid;

use super::config::BotConfig;
use crate::otc::avatar::identicon_svg;

#[derive(Clone)]
pub struct BotNotifier {
    bot: Bot,
    chat_id: ChatId,
}

impl BotNotifier {
    pub fn from_env() -> Result<Option<Self>> {
        Ok(BotConfig::from_env()?.map(|config| Self {
            bot: Bot::new(config.token),
            chat_id: config.chat_id,
        }))
    }

    pub async fn notify_new_user(
        &self,
        anonymous_id: Uuid,
        region: &str,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<()> {
        let ip_line = ip.unwrap_or("неизвестен");
        let device = user_agent.unwrap_or("неизвестно");
        let message = format!(
            "Новый пользователь\nАнонимный ID: {anonymous_id}\nРегион: {region}\nIP: {ip_line}\nУстройство: {device}"
        );
        let avatar = identicon_svg(&anonymous_id.to_string());

        self.bot
            .send_document(
                self.chat_id,
                InputFile::memory(avatar.into_bytes()).file_name("avatar.svg"),
            )
            .caption(message)
            .await?;
        Ok(())
    }
}

//! Outbound auth mail (portal §4.4).
//!
//! Delivery is plain SMTP so any provider works without a vendor SDK: the
//! operator supplies host, port and credentials through the environment. When
//! no host is configured the mailer is disabled, which is allowed only next to
//! the explicit development code.

use anyhow::Context;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use super::error::UserError;

const SUBJECT: &str = "Your Pay3Flow sign-in code";

/// SMTP transport used for sign-in codes. Cloning shares the connection pool.
#[derive(Clone, Default)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Option<Mailbox>,
    /// Fixed code for local development; `Config` refuses it in production.
    dev_code: Option<String>,
}

impl Mailer {
    /// Build the transport from configuration. `port` 465 uses implicit TLS,
    /// every other port uses STARTTLS; both are verified with the system roots.
    pub fn new(
        host: Option<&str>,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        from: Option<&str>,
        dev_code: Option<&str>,
    ) -> anyhow::Result<Self> {
        let dev_code = dev_code.map(str::to_string);
        let Some(host) = host.map(str::trim).filter(|host| !host.is_empty()) else {
            return Ok(Self {
                transport: None,
                from: None,
                dev_code,
            });
        };
        let from = from
            .map(str::trim)
            .filter(|from| !from.is_empty())
            .context("SMTP_FROM is required when SMTP_HOST is set")?;
        let from: Mailbox = from
            .parse()
            .with_context(|| format!("SMTP_FROM `{from}` is not a mailbox"))?;
        let mut builder = if port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
        }
        .port(port);
        if let Some(username) = username.map(str::trim).filter(|user| !user.is_empty()) {
            builder = builder.credentials(Credentials::new(
                username.to_string(),
                password.unwrap_or_default().to_string(),
            ));
        }
        Ok(Self {
            transport: Some(builder.build()),
            from: Some(from),
            dev_code,
        })
    }

    /// Whether a mail transport was configured.
    pub fn is_configured(&self) -> bool {
        self.transport.is_some()
    }

    /// The fixed development code, when explicitly enabled.
    pub fn dev_code(&self) -> Option<&str> {
        self.dev_code.as_deref()
    }

    /// Send the six-digit sign-in code.
    pub async fn send_code(&self, to: &str, code: &str) -> Result<(), UserError> {
        let (Some(transport), Some(from)) = (&self.transport, &self.from) else {
            return Err(UserError::Other(anyhow::anyhow!(
                "mail transport is not configured"
            )));
        };
        let to: Mailbox = to
            .parse()
            .map_err(|error| UserError::Other(anyhow::anyhow!("invalid recipient: {error}")))?;
        let message = Message::builder()
            .from(from.clone())
            .to(to)
            .subject(SUBJECT)
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "Your Pay3Flow sign-in code is {code}.\n\n\
                 It expires in 10 minutes and can be used once. \
                 If you did not ask for it, ignore this email."
            ))
            .map_err(|error| UserError::Other(anyhow::anyhow!("build auth mail: {error}")))?;
        transport
            .send(message)
            .await
            .map_err(|error| UserError::Other(anyhow::anyhow!("send auth mail: {error}")))?;
        Ok(())
    }
}

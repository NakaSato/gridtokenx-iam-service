use async_trait::async_trait;
use anyhow::{Context, Result as AnyhowResult};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
};

#[derive(Clone)]
pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

use iam_core::traits::EmailTrait;
use iam_core::error::{ApiError, Result};

impl EmailService {
    pub fn new(host: &str, port: u16, from: &str) -> AnyhowResult<Self> {
        // Mailpit listens on plain SMTP (no TLS) — use builder_dangerous for localhost
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port)
            .build();
        
        Ok(EmailService {
            mailer,
            from: from.to_string(),
        })
    }

    /// Internal helper for sending emails.
    pub async fn send_raw(&self, to_email: &str, subject: &str, body: &str) -> AnyhowResult<()> {
        let email = Message::builder()
            .from(self.from.parse()?)
            .to(to_email.parse()?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())?;

        self.mailer.send(email).await.context("Failed to send email")?;
        Ok(())
    }

    /// Internal helper for password reset email logic.
    pub async fn send_password_reset_raw(&self, to_email: &str, reset_url: &str) -> AnyhowResult<()> {
        let subject = "Reset Your Password - GridTokenX";
        let body = format!(
            "Hello,\n\nTo reset your password, please click the link below:\n{}\n\nIf you did not request this, please ignore this email.\n\nBest,\nGridTokenX Team",
            reset_url
        );
        self.send_raw(to_email, subject, &body).await
    }
}

#[async_trait]
impl EmailTrait for EmailService {
    async fn send_password_reset(&self, email: &str, reset_url: &str) -> Result<()> {
        self.send_password_reset_raw(email, reset_url).await.map_err(|e| ApiError::Internal(e.to_string()))
    }
}

use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// Outbound email.
/// * `Smtp` — real delivery (TLS is enforced outside development by `Config`).
/// * `Capture` — in-memory outbox for tests and local development.
/// * `Disabled` — refuses to send, so a misconfigured environment fails loudly instead of pretending.
#[derive(Clone)]
pub enum Mailer {
    Smtp { transport: AsyncSmtpTransport<Tokio1Executor>, from: Mailbox },
    Capture(Arc<Mutex<Vec<Email>>>),
    Disabled,
}

impl Mailer {
    pub fn capture() -> Self {
        Mailer::Capture(Arc::default())
    }

    pub fn smtp(url: &str, from: &str) -> Result<Self, String> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url).map_err(|e| format!("invalid SMTP_URL: {e}"))?.build();
        let from = from.parse::<Mailbox>().map_err(|e| format!("invalid MAIL_FROM: {e}"))?;
        Ok(Mailer::Smtp { transport, from })
    }

    pub async fn send(&self, email: Email) -> Result<(), String> {
        match self {
            Mailer::Smtp { transport, from } => {
                // Address parsing rejects CR/LF, so recipient input cannot smuggle extra headers (Bcc injection).
                let to = email.to.parse::<Mailbox>().map_err(|e| format!("invalid recipient: {e}"))?;
                let msg = Message::builder().from(from.clone()).to(to).subject(email.subject).body(email.text).map_err(|e| format!("could not build message: {e}"))?;
                transport.send(msg).await.map(|_| ()).map_err(|e| format!("smtp delivery failed: {e}"))
            }
            Mailer::Capture(outbox) => {
                outbox.lock().map_err(|_| "outbox poisoned".to_string())?.push(email);
                Ok(())
            }
            Mailer::Disabled => Err("no mail transport configured".into()),
        }
    }

    pub fn outbox(&self) -> Vec<Email> {
        match self {
            Mailer::Capture(o) => o.lock().map(|g| g.clone()).unwrap_or_default(),
            _ => vec![],
        }
    }
}

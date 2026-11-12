use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub subject: String,
    pub text: String,
}

/// Outbound email. `Capture` keeps messages in memory (tests and local development); `Disabled` refuses to send.
/// A real SMTP transport is selected by configuration in a later step.
#[derive(Clone)]
pub enum Mailer {
    Capture(Arc<Mutex<Vec<Email>>>),
    Disabled,
}

impl Mailer {
    pub fn capture() -> Self {
        Mailer::Capture(Arc::default())
    }

    pub async fn send(&self, email: Email) -> Result<(), &'static str> {
        match self {
            Mailer::Capture(outbox) => {
                outbox.lock().map_err(|_| "outbox poisoned")?.push(email);
                Ok(())
            }
            Mailer::Disabled => Err("no mail transport configured"),
        }
    }

    pub fn outbox(&self) -> Vec<Email> {
        match self {
            Mailer::Capture(o) => o.lock().map(|g| g.clone()).unwrap_or_default(),
            Mailer::Disabled => vec![],
        }
    }
}

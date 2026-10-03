use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// Best-effort SMTP delivery. The `email_logs` table is always the source of
/// truth for the validation flow; real email is an optional extra. When SMTP
/// credentials are not configured, this is a no-op.
#[derive(Clone)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: String,
    /// In dev, seeded users have fake addresses (admin@example.com), so all
    /// outgoing mail can be redirected to one real inbox for demos.
    redirect_to: Option<String>,
}

impl Mailer {
    pub fn from_env() -> Self {
        // Treat empty values as unset: .env.example ships the keys blank.
        fn nonempty(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|v| !v.trim().is_empty())
        }
        let username = nonempty("SMTP_USERNAME");
        let password = nonempty("SMTP_PASSWORD");
        let host = nonempty("SMTP_HOST").unwrap_or_else(|| "smtp.gmail.com".into());
        let redirect_to = nonempty("SMTP_DEV_REDIRECT_TO");

        let (transport, from) = match (username, password) {
            (Some(user), Some(pass)) => {
                let from =
                    nonempty("SMTP_FROM").unwrap_or_else(|| format!("Task App <{user}>"));
                match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host) {
                    Ok(builder) => {
                        tracing::info!("SMTP delivery enabled via {host} as {user}");
                        (
                            Some(builder.credentials(Credentials::new(user, pass)).build()),
                            from,
                        )
                    }
                    Err(e) => {
                        tracing::warn!("SMTP disabled, bad relay config: {e}");
                        (None, from)
                    }
                }
            }
            _ => {
                tracing::info!("SMTP not configured; 2FA codes via email_logs/console only");
                (None, String::new())
            }
        };

        Self {
            transport,
            from,
            redirect_to,
        }
    }

    /// Fire-and-forget: spawned so login latency and success never depend on
    /// the SMTP server. Failures are logged, not propagated.
    pub fn send(&self, to: &str, subject: &str, body: &str) {
        let Some(transport) = self.transport.clone() else {
            return;
        };
        let to = self.redirect_to.clone().unwrap_or_else(|| to.to_string());
        let from = self.from.clone();
        let subject = subject.to_string();
        let body = body.to_string();

        tokio::spawn(async move {
            let message = Message::builder()
                .from(match from.parse() {
                    Ok(m) => m,
                    Err(e) => return tracing::warn!("bad SMTP_FROM {from}: {e}"),
                })
                .to(match to.parse() {
                    Ok(m) => m,
                    Err(e) => return tracing::warn!("bad recipient {to}: {e}"),
                })
                .subject(subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body);

            match message {
                Ok(message) => match transport.send(message).await {
                    Ok(_) => tracing::info!("SMTP: delivered mail to {to}"),
                    Err(e) => tracing::warn!("SMTP delivery to {to} failed: {e}"),
                },
                Err(e) => tracing::warn!("failed to build email: {e}"),
            }
        });
    }
}

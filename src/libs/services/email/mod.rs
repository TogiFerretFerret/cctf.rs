use async_trait::async_trait;
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use unic_langid::{LanguageIdentifier, langid};

pub mod client;
pub mod http;
pub mod server;

pub use client::{SmtpCredentials, SmtpSenderClient, TlsMode};
pub use http::{HttpCatcher, HttpCatcherConfig};
pub use server::{Mailbox, SmtpCatcherServer};

static_loader! {
    pub static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub timestamp: i64,
}

#[derive(Debug)]
pub enum EmailError {
    Connect(String),
    Io(String),
    Tls(String),
    InvalidServerName(String),
    UnexpectedEof,
    InvalidResponse,
    Rejected { code: u16, phase: String },
    StartTlsUnsupported,
    AuthUnsupported,
    AuthFailed,
    AuthRequiresTls,
    MessageTooLarge,
    AlreadySecured,
}

fn lookup_reason(lang_id: &LanguageIdentifier, key: &str, reason: &str) -> String {
    let args = HashMap::from([(
        Cow::Borrowed("reason"),
        FluentValue::from(reason.to_string()),
    )]);
    LOCALES.lookup_with_args(lang_id, key, &args)
}

impl EmailError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            EmailError::Connect(r) => lookup_reason(&lang_id, "email-connect-failed", r),
            EmailError::Io(r) => lookup_reason(&lang_id, "email-io-error", r),
            EmailError::Tls(r) => lookup_reason(&lang_id, "email-tls-failed", r),
            EmailError::InvalidServerName(r) => {
                lookup_reason(&lang_id, "email-invalid-server-name", r)
            }
            EmailError::UnexpectedEof => LOCALES.lookup(&lang_id, "email-unexpected-eof"),
            EmailError::InvalidResponse => LOCALES.lookup(&lang_id, "email-invalid-response"),
            EmailError::StartTlsUnsupported => {
                LOCALES.lookup(&lang_id, "email-starttls-unsupported")
            }
            EmailError::AuthUnsupported => LOCALES.lookup(&lang_id, "email-auth-unsupported"),
            EmailError::AuthFailed => LOCALES.lookup(&lang_id, "email-auth-failed"),
            EmailError::AuthRequiresTls => LOCALES.lookup(&lang_id, "email-auth-requires-tls"),
            EmailError::MessageTooLarge => LOCALES.lookup(&lang_id, "email-message-too-large"),
            EmailError::Rejected { code, phase } => {
                let args = HashMap::from([
                    (Cow::Borrowed("phase"), FluentValue::from(phase.clone())),
                    (Cow::Borrowed("code"), FluentValue::from(code.to_string())),
                ]);
                LOCALES.lookup_with_args(&lang_id, "email-command-rejected", &args)
            }
            EmailError::AlreadySecured => LOCALES.lookup(&lang_id, "email-already-secured"),
        }
    }
}

impl fmt::Display for EmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.localize("en-US"))
    }
}

impl std::error::Error for EmailError {}

impl From<std::io::Error> for EmailError {
    fn from(e: std::io::Error) -> Self {
        EmailError::Io(e.to_string())
    }
}

#[async_trait]
pub trait EmailService {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError>;
}

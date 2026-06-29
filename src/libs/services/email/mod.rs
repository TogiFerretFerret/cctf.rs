use async_trait::async_trait;
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use unic_langid::{LanguageIdentifier, langid};

pub mod client;
pub mod server;
pub use client::{SmtpCredentials, SmtpSenderClient, TlsMode};
pub use server::{Mailbox, SmtpCatcherServer};

static_loader! {
    pub static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

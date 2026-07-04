use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use unic_langid::langid;

pub mod pg;
pub mod traits;

pub use traits::*;

static_loader! {
    pub static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Debug, Clone)]
pub enum RepoError {
    Connection(String),
    NotFound,
    Conflict(String),
    Internal(String),
}

impl RepoError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            RepoError::Connection(_) => LOCALES.lookup(&lang_id, "server-db-connection-failed"),
            RepoError::NotFound => LOCALES.lookup(&lang_id, "ctf-challenge-not-found"),
            RepoError::Conflict(key) => LOCALES.lookup(&lang_id, key),
            RepoError::Internal(err) => {
                let args =
                    HashMap::from([(Cow::Borrowed("reason"), FluentValue::from(err.to_string()))]);
                LOCALES.lookup_with_args(&lang_id, "admin-db-internal-error", &args)
            }
        }
    }
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.localize("en-US"))
    }
}

impl std::error::Error for RepoError {}

impl From<sqlx::Error> for RepoError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::Database(db_err) => {
                if db_err.is_unique_violation() {
                    RepoError::Conflict("auth-username-taken".to_string())
                } else {
                    RepoError::Internal(err.to_string())
                }
            }
            sqlx::Error::RowNotFound => RepoError::NotFound,
            sqlx::Error::Io(_) => RepoError::Connection(err.to_string()),
            _ => RepoError::Internal(err.to_string()),
        }
    }
}

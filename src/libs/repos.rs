use crate::libs::types::accounts::{Account, AccountId, AccountName};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use crate::libs::types::challenges::Challenge;
use crate::libs::types::solves::Solve;
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use unic_langid::langid;

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
            RepoError::Connection(_) => LOCALES.lookup(&lang_id, "server-db-connection-failed")
            RepoError::NotFound => LOCALES.lookup(&lang_id, "ctf-challenge-not-found")
            RepoError::Conflict(key) => {
                LOCALES.lookup(&lang_id, key)
            }
            RepoError::Internal(err) => {
                let args = HashMap::from([(Cow::Borrowed("reason"),FluentValue::from(err.to_string()))]);
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

pub trait AccountRepo: Send + Sync {
    async fn find_
}

use crate::libs::repos::RepoError;
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use unic_langid::langid;

static_loader! {
    pub static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Debug)]
pub enum ServiceError {
    Repo(RepoError),
    OAuth(String),
    InvalidRequest(String),
    Unauthorized,
    Kube(String),
    RateLimitExceeded,
}

impl From<RepoError> for ServiceError {
    fn from(err: RepoError) -> Self {
        ServiceError::Repo(err)
    }
}

impl From<kube::Error> for ServiceError {
    fn from(err: kube::Error) -> Self {
        ServiceError::Kube(err.to_string())
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        ServiceError::Repo(RepoError::from(err))
    }
}

impl ServiceError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            ServiceError::Repo(err) => err.localize(lang),
            ServiceError::Unauthorized => LOCALES.lookup(&lang_id, "auth-not-logged-in"),
            ServiceError::InvalidRequest(key) => LOCALES.lookup(&lang_id, key),
            ServiceError::RateLimitExceeded => LOCALES.lookup(&lang_id, "ctf-rate-limit-exceeded"),
            ServiceError::OAuth(reason) => {
                let args = HashMap::from([(
                    Cow::Borrowed("reason"),
                    FluentValue::from(reason.to_string()),
                )]);
                LOCALES.lookup_with_args(&lang_id, "oauth-invalid-credentials", &args)
            }
            ServiceError::Kube(reason) => {
                let args = HashMap::from([(
                    Cow::Borrowed("reason"),
                    FluentValue::from(reason.to_string()),
                )]);
                LOCALES.lookup_with_args(&lang_id, "kube-api-error", &args)
            }
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.localize("en-US"))
    }
}

impl std::error::Error for ServiceError {}

pub mod auth;
pub mod config;
pub mod email;
pub mod instancer;
pub mod scoreboard;
pub mod solve;

pub use auth::{AuthService, OAuthService};
pub use config::ConfigService;
pub use scoreboard::ScoreboardService;
pub use solve::SolveService;

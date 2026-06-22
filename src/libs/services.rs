use crate::libs::repos::{AccountRepo, TeamRepo, ChallengeRepo, SolveRepo, RepoError};
use crate::libs::types::accounts::{Account, AccountId, AccountName, AccountEmail, AccountRole, CtfTimeUserProfile};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use crate::libs::types::challenges::{Challenge, ScoringMode};
use crate::libs::types::solves::Solve;
use crate::libs::types::flags::FlagValidator;
use crate::libs::crypto::jwt;
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use unic_langid::langid;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Debug)]
pub enum ServiceError {
    Repo(RepoError),
    OAuth(String),
    InvalidRequest(String),
    Unauthorized
}

impl From<RepoError> for ServiceError {
    fn from(err: RepoError) -> Self {
        ServiceError::Repo(err)
    }
}

impl ServiceError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            ServiceError::Repo(err) => err.localize(lang),
            ServiceError::Unauthorized => LOCALES.lookup(&lang_id, "auth-not-logged-in"),
            ServiceError::InvalidRequest(key) => LOCALES.lookup(&lang_id, key),
            ServiceError::OAuth(reason) => {
                let args = HashMap::from([(Cow::Borrowed("reason"),FluentValue::from(reason.to_string()))]);
                LOCALES.lookup_with_args(&lang_id, "oauth-invalid-credentials", &args)
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

fn generate_salt() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let hash_hex = format!("{:x}", hasher.finalize());
    format!("{}${}", salt, hash_hex)
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parts: Vec<&str> = stored_hash.split('$').collect();
    if parts.len() != 2 {
        return false;
    }
    let salt = parts[0];
    let expected_hash = parts[1];
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let hash_hex = format!("{:x}", hasher.finalize());
    hash_hex == expected_hash
}

pub struct AuthService {
    pub account_repo: Arc<dyn AccountRepo>,
    pub team_repo: Arc<dyn TeamRepo>,
    pub jwt_secret: Vec<u8>
}

impl AuthService {
    pub async fn register(
        &self, 
        username: &str,
        email: Option<&str>,
        password: &str,
    ) -> Result<Account, ServiceError> {
        let name = AccountName(username.to_string());
        if self.account_repo.find_by_username(&name).await?.is_some() {
            return Err(ServiceError::InvalidRequest("auth-username-taken".to_string()));
        }
        let account_id = AccountId(uuid::Uuid::new_4().to_string());
        let salt = generate_salt();
        let hashed = hash_password(password, &salt);
        let account = Account {
            id: account_id,
            username: name,
            email: email.map(|e| AccountEmail(e.to_string())),
            password_hash: Some(hashed),
            role: AccountRole::Player,
            team_id: None,
            ctftime_id: None,
            fields: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.account_repo.save(account.clone()).await?;
        Ok(account)
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<String, ServiceError> {
        let name = AccountName(username.to_string());
        let account = self.account_repo.find_by_username(&name).await?
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        let stored_hash = account.password_hash.as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        if !verify_password(password, stored_hash) {
            return Err(ServiceError::InvalidRequest("auth-invalid-credentials".to_string()));
        }
        let token = jwt::encode(&account.id.0, &self.jwt_secret).map_err(|e| ServiceError::OAuth(e.to_string()))?;
        Ok(token)
    }
}

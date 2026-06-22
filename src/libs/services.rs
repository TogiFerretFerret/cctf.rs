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
    let hash_hex: String = hasher.finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
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
    let hash_hex: String = hasher.finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    hash_hex == expected_hash
}

pub struct AuthService<A, T>
where 
    A: AccountRepo,
    T: TeamRepo,
{
    pub account_repo: A, 
    pub team_repo: T,
    pub jwt_secret: Vec<u8>,
}

impl<A, T> AuthService<A, T>
where 
    A: AccountRepo,
    T: TeamRepo,
{
    pub async fn register(
        &self,
        username: &str,
        email: Option<&str>,
        password: &str,
    ) -> Result<Account, ServiceError> {
        let name = AccountName(username.to_string());
        if self.account_repo.find_by_username(&name).await?.is_some() {
            return Err(ServiceError::InvalidRequest("auth-username-token".to_string()));
        }
        let account_id = AccountId(uuid::Uuid::new_v4().to_string());
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
        let token = jwt::encode(&account.id.0, &self.jwt_secret)
            .map_err(|e| ServiceError::OAuth(e.to_string()))?;
        Ok(token)
    }
}

pub struct OAuthService<A, T>
where 
    A: AccountRepo,
    T: TeamRepo,
{
        pub account_repo: A,
        pub team_repo: T,
        pub client_id: String,
        pub client_secret: String,
        pub redirect_uri: String,
        pub jwt_secret: Vec<u8>,
}

impl<A, T> OAuthService<A, T>
where 
    A: AccountRepo,
    T: TeamRepo,
{
    pub fn get_authorize_url(&self) -> String {
        format!(
            "https://oauth.ctftime.org/authorize?client_id={}&redirect_uri={}&response_type=code&scope=profile+team",
            self.client_id, self.redirect_uri
        )
    }

    pub async fn handle_callback(&self, code: &str) -> Result<String, ServiceError> {
        let client = reqwest::Client::new();
        let token_resp = client
            .post("https://oauth.ctftime.org/token")
            .form(&[
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("redirect_uri", &self.redirect_uri),
                ("grant_type", &"authorization_code".to_string()),
                ("code", &code.to_string()),
            ])
            .send()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-token-failed".to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-token-parse-failed".to_string()))?;
        let access_token = token_resp.get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ServiceError::OAuth("auth-oauth-token-missing".to_string()))?;
        let profile = client
            .get("https://oauth.ctftime.org/user")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-profile-failed".to_string()))?
            .json::<CtfTimeUserProfile>()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-profile-parse-failed".to_string()))?;  
        let account = match self.account_repo.find_by_ctftime_id(profile.id).await? {
            Some(acc) => acc,
            None => {
                let mut base_name = profile.username.clone();
                let mut check_name = AccountName(base_name.clone());
                let mut suffix = 1;
                while self.account_repo.find_by_username(&check_name).await?.is_some() {
                    check_name = AccountName(format!("{}{}", base_name, suffix));
                    suffix += 1;
                } // TODO: this probably can't cause time issues because.. I know big O, but... it's
                  // possible :shrug:
                let mut local_team_id = None;
                if let Some(ref ctftime_team) = profile.team {
                    let team = match self.team_repo.find_by_ctftime_id(ctftime_team.id).await? {
                        Some(t) => t,
                        None => {
                            let team_id = TeamId(uuid::Uuid::new_v4().to_string());
                            let dummy_captain = AccountId("system-oauth".to_string());
                            let new_team = Team {
                                id: team_id.clone(),
                                name: TeamName(ctftime_team.name.clone()),
                                ctftime_id: Some(ctftime_team.id),
                                invite_code: None,
                                captain_id: dummy_captain,
                                member_ids: Vec::new(),
                                fields: HashMap::new(),
                                create_at: chrono::Utc::now().timestamp(),
                            };
                            self.team_repo.save(new_team.clone()).await?;
                            new_team
                        }
                    };
                    local_team_id = Some(team.id);
                }
                let new_account = Account {
                    id: AccountId(uuid::Uuid::new_v4().to_string()),
                    username: check_name,
                    email: profile.email.map(|e| AccountEmail(e)),
                    password_hash: None,
                    role: AccountRole::Player,
                    team_id: local_team_id.clone(),
                    ctftime_id: Some(profile.id),
                    fields: HashMap::new(),
                    created_at: chrono::Utc::now().timestamp(),
                };
                self.account_repo.save(new_account.clone()).await?;
                if let Some(t_id) = local_team_id {
                    if let Some(mut team) = self.team_repo.find_by_id(&t_id).await? {
                        if team.captain_id.0 == "system-oauth" { // TODO: security issue if someone names
                                                                 // something system-oauth???
                            team.captain_id = new_account.id.clone(); // this may mitigate?
                        }
                        team.member_ids.push(new_account.id.clone());
                        team.member_ids.push(new_account.id.clone());
                        self.team_repo.update(team).await?;
                    }
                }
                new_account
            }
        };
        let local_token = jwt::encode(&account.id.0, &self.jwt_secret)
            .map_err(|_| ServiceError::OAuth("auth-token-generation-failed".to_string()))?;
        Ok(local_token)
    }
}

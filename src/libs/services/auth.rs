use super::ServiceError;
use crate::libs::crypto::jwt;
use crate::libs::repos::{AccountRepo, TeamRepo};
use crate::libs::types::accounts::{
    Account, AccountEmail, AccountId, AccountName, AccountRole, CtfTimeUserProfile,
};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use std::collections::HashMap;

/// Hash a password with Argon2id. Output is a self-describing PHC string
/// (`$argon2id$...$salt$hash`) — salt and cost params are embedded, so there is
/// nothing else to store. Argon2 is memory-hard, so a leaked DB is not trivially
/// GPU-crackable the way a fast SHA-256 hash would be.
fn hash_password(password: &str) -> Result<String, ServiceError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| ServiceError::InvalidRequest("auth-hash-failed".to_string()))
}

/// Verify against a stored PHC string. Argon2's verify is constant-time, so this
/// does not leak timing about how many characters matched.
pub(crate) fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
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
            return Err(ServiceError::InvalidRequest(
                "auth-username-taken".to_string(),
            ));
        }
        let account_id = AccountId(uuid::Uuid::new_v4().to_string());
        let hashed = hash_password(password)?;
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
        let account = self
            .account_repo
            .find_by_username(&name)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        let stored_hash = account
            .password_hash
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        if !verify_password(password, stored_hash) {
            return Err(ServiceError::InvalidRequest(
                "auth-invalid-credentials".to_string(),
            ));
        }
        let token = jwt::issue(&account.id.0, 24 * 3600, &self.jwt_secret)
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
        let access_token = token_resp
            .get("access_token")
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
                let base_name = profile.username.clone();
                let mut check_name = AccountName(base_name.clone());
                let mut suffix = 1;
                while self
                    .account_repo
                    .find_by_username(&check_name)
                    .await?
                    .is_some()
                {
                    check_name = AccountName(format!("{}{}", base_name, suffix));
                    suffix += 1;
                }
                let mut new_account = Account {
                    id: AccountId(uuid::Uuid::new_v4().to_string()),
                    username: check_name,
                    email: None,
                    password_hash: None,
                    role: AccountRole::Player,
                    team_id: None,
                    ctftime_id: Some(profile.id),
                    fields: HashMap::new(),
                    created_at: chrono::Utc::now().timestamp(),
                };
                let mut local_team_id = None;
                if let Some(ref ctftime_team) = profile.team {
                    let team = match self.team_repo.find_by_ctftime_id(ctftime_team.id).await? {
                        Some(t) => t,
                        None => {
                            let team_id = TeamId(uuid::Uuid::new_v4().to_string());
                            let new_team = Team {
                                id: team_id.clone(),
                                name: TeamName(ctftime_team.name.clone()),
                                ctftime_id: Some(ctftime_team.id),
                                invite_code: None,
                                captain_id: new_account.id.clone(),
                                member_ids: vec![new_account.id.clone()],
                                bracket: "Open".to_string(),
                                fields: HashMap::new(),
                                create_at: chrono::Utc::now().timestamp(),
                            };
                            self.team_repo.save(new_team.clone()).await?;
                            new_team
                        }
                    };
                    local_team_id = Some(team.id.clone());
                    new_account.team_id = Some(team.id.clone());
                }
                self.account_repo.save(new_account.clone()).await?;
                if let Some(t_id) = local_team_id
                    && let Some(mut team) = self.team_repo.find_by_id(&t_id).await?
                    && !team.member_ids.contains(&new_account.id)
                {
                    team.member_ids.push(new_account.id.clone());
                    self.team_repo.update(team).await?;
                }
                new_account
            }
        };
        let local_token = jwt::issue(&account.id.0, 24 * 3600, &self.jwt_secret)
            .map_err(|_| ServiceError::OAuth("auth-token-generation-failed".to_string()))?;
        Ok(local_token)
    }
}

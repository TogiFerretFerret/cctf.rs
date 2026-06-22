use crate::libs::types::accounts::{Account, AccountId, AccountName};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use crate::libs::types::challenges::Challenge;
use crate::libs::types::solves::Submission;
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use std::collections::HashMap;
use std::borrow::Cow;
use std::fmt;
use unic_langid::langid;

static_loader! {
    static LOCALES = {
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
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError>;
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError>;
    async fn save(&self, account: Account) -> Result<(), RepoError>;
    async fn update(&self, account: Account) -> Result<(), RepoError>;
}

pub trait TeamRepo: Send + Sync {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError>;
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>,RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>,RepoError>;
    async fn save(&self, team: Team) -> Result<(), RepoError>;
    async fn update(&self, team: Team) -> Result<(), RepoError>;
    async fn find_all(&self) -> Result<Vec<Team>, RepoError>;
}

pub trait ChallengeRepo: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError>;
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError>;
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError>;
}

pub trait SubmissionRepo: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError>;
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError>;
    async fn save(&self, submission: Submission) -> Result<(), RepoError>;
}

impl<T: AccountRepo + ?Sized> AccountRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError> {
        (**self).find_by_username(name).await
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError> {
        (**self).find_by_ctftime_id(ctftime_id).await
    }
    async fn save(&self, account: Account) -> Result<(), RepoError> {
        (**self).save(account).await
    }
    async fn update(&self, account: Account) -> Result<(), RepoError> {
        (**self).update(account).await
    }
}

impl<T: TeamRepo + ?Sized> TeamRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError> {
        (**self).find_by_name(name).await
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError> {
        (**self).find_by_ctftime_id(ctftime_id).await
    }
    async fn save(&self, team: Team) -> Result<(), RepoError> {
        (**self).save(team).await
    }
    async fn update(&self, team: Team) -> Result<(), RepoError> {
        (**self).update(team).await
    }
    async fn find_all(&self) -> Result<Vec<Team>, RepoError> {
        (**self).find_all().await
    }
}

impl<T: ChallengeRepo + ?Sized> ChallengeRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError> {
        (**self).find_all().await
    }
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError> {
        (**self).save(challenge).await
    }
}

impl<T: SubmissionRepo + ?Sized> SubmissionRepo for std::sync::Arc<T> {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError> {
        (**self).find_all().await
    }
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError> {
        (**self).find_by_team(team_id).await
    }
    async fn save(&self, submission: Submission) -> Result<(), RepoError> {
        (**self).save(submission).await
    }
    }

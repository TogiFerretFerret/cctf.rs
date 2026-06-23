use crate::libs::types::accounts::{Account, AccountId, AccountName, AccountEmail, AccountRole};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use crate::libs::types::challenges::{Challenge, ScoringMode};
use crate::libs::types::solves::{Submission, SubmissionId};
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use sqlx::Row;
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

pub struct PgStore {
    pool: sqlx::PgPool,
}

impl PgStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
    pub async fn init_db(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS teams ( \
                id VARCHAR(64) PRIMARY KEY, \
                name VARCHAR(255) UNIQUE NOT NULL, \
                ctftime_id INT UNIQUE, \
                invite_code VARCHAR(255), \
                captain_id VARCHAR(64) NOT NULL, \
                fields JSONB NOT NULL DEFAULT '{}', \
                created_at BIGINT NOT NULL \
             );"
        ).execute(&self.pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS accounts ( \
                id VARCHAR(64) PRIMARY KEY, \
                username VARCHAR(255) UNIQUE NOT NULL, \
                email VARCHAR(255), \
                password_hash VARCHAR(255), \
                role VARCHAR(50) NOT NULL, \
                team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
                ctftime_id INT UNIQUE, \
                fields JSONB NOT NULL DEFAULT '{}', \
                created_at BIGINT NOT NULL \
             );"
        ).execute(&self.pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS challenges ( \
                id VARCHAR(64) PRIMARY KEY, \
                title VARCHAR(255) NOT NULL, \
                description TEXT NOT NULL, \
                category VARCHAR(100) NOT NULL, \
                points_mode VARCHAR(50) NOT NULL, \
                points_equation VARCHAR(255) NOT NULL, \
                flag JSONB NOT NULL, \
                author_id VARCHAR(64) NOT NULL, \
                author_username VARCHAR(255) NOT NULL, \
                hints JSONB NOT NULL DEFAULT '[]', \
                files JSONB NOT NULL DEFAULT '[]', \
                tags JSONB NOT NULL DEFAULT '[]', \
                requirements JSONB NOT NULL DEFAULT '[]' \
             );"
        ).execute(&self.pool).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS submissions ( \
                id VARCHAR(64) PRIMARY KEY, \
                challenge_id VARCHAR(64) REFERENCES challenges(id) ON DELETE CASCADE NOT NULL, \
                team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
                account_id VARCHAR(64) REFERENCES accounts(id) ON DELETE CASCADE NOT NULL, \
                points INT NOT NULL, \
                provided_flag TEXT NOT NULL, \
                is_correct BOOLEAN NOT NULL, \
                submitted_at BIGINT NOT NULL \
             );"
        ).execute(&self.pool).await?;
        Ok(())
    }
    async fn map_team(&self, row: &sqlx::postgres::PgRow) -> Result<Team, sqlx::Error> {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let ctftime_id: Option<i32> = row.get("ctftime_id");
        let invite_code: Option<String> = row.get("invite_code");
        let captain_id: String = row.get("captain_id");
        let fields_val: serde_json::Value = row.get("fields");
        let create_at: i64 = row.get("created_at");

        let members = sqlx::query("SELECT id FROM accounts WHERE team_id = $1")
            .bind(&id)
            .fetch_all(&self.pool)
            .await?;

        let member_ids = members
            .into_iter()
            .map(|r| AccountId(r.get("id")))
            .collect();

        let fields = serde_json::from_value(fields_val)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(Team {
            id: TeamId(id),
            name: TeamName(name),
            ctftime_id: ctftime_id.map(|id| id as u32),
            invite_code,
            captain_id: AccountId(captain_id),
            member_ids,
            fields,
            create_at,
        })
    }
}

use crate::libs::types::accounts::{Account, AccountEmail, AccountId, AccountName, AccountRole};
use crate::libs::types::challenges::{Challenge, ScoringMode};
use crate::libs::types::solves::{Submission, SubmissionId};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use async_trait::async_trait;
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use sqlx::Row;
use std::borrow::Cow;
use std::collections::HashMap;
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

#[async_trait]
pub trait AccountRepo: Send + Sync {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError>;
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError>;
    async fn save(&self, account: Account) -> Result<(), RepoError>;
    async fn update(&self, account: Account) -> Result<(), RepoError>;
}

#[async_trait]
pub trait TeamRepo: Send + Sync {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError>;
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError>;
    async fn save(&self, team: Team) -> Result<(), RepoError>;
    async fn update(&self, team: Team) -> Result<(), RepoError>;
    async fn find_all(&self) -> Result<Vec<Team>, RepoError>;
}

#[async_trait]
pub trait InstanceRepo: Send + Sync {
    async fn find_active_flag(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Option<String>, RepoError>;
}

#[async_trait]
pub trait ChallengeRepo: InstanceRepo + Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError>;
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError>;
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError>;
}

#[async_trait]
pub trait SubmissionRepo: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError>;
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError>;
    async fn save(&self, submission: Submission) -> Result<(), RepoError>;
}

#[async_trait]
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

#[async_trait]
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

#[async_trait]
impl<T: InstanceRepo + ?Sized> InstanceRepo for std::sync::Arc<T> {
    async fn find_active_flag(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Option<String>, RepoError> {
        (**self)
            .find_active_flag(challenge_id, team_id, account_id)
            .await
    }
}

#[async_trait]
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

#[async_trait]
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

#[async_trait]
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
             );",
        )
        .execute(&self.pool)
        .await?;
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
             );",
        )
        .execute(&self.pool)
        .await?; // TODO: what happens if username>255 len, or email etc. or role? is there a string
        // type?
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
             );",
        )
        .execute(&self.pool)
        .await?;
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
             );",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS challenge_instance ( \
                id VARCHAR(64) PRIMARY KEY, \
                challenge_id VARCHAR(64) REFERENCES challenges(id) ON DELETE CASCADE NOT NULL, \
                team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
                account_id VARCHAR(64) REFERENCES accounts(id) ON DELETE CASCADE NOT NULL, \
                flag VARCHAR(255) NOT NULL, \
                cluster_ip VARCHAR(45) NOT NULL, \
                created_at BIGINT NOT NULL, \
                expires_at BIGINT NOT NULL, \
            );",
        )
        .execute(&self.pool)
        .await?;
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

        let fields =
            serde_json::from_value(fields_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

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

fn map_account(row: &sqlx::postgres::PgRow) -> Result<Account, sqlx::Error> {
    let id: String = row.get("id");
    let username: String = row.get("username");
    let email: Option<String> = row.get("email");
    let password_hash: Option<String> = row.get("password_hash");
    let role_str: String = row.get("role");
    let team_id_str: Option<String> = row.get("team_id");
    let ctftime_id: Option<i32> = row.get("ctftime_id");
    let fields_val: serde_json::Value = row.get("fields");
    let created_at: i64 = row.get("created_at");
    let role = match role_str.as_str() {
        "Admin" => AccountRole::Admin,
        "Spectator" => AccountRole::Spectator,
        _ => AccountRole::Player,
    };
    let fields =
        serde_json::from_value(fields_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    Ok(Account {
        id: AccountId(id),
        username: AccountName(username),
        email: email.map(AccountEmail),
        password_hash,
        role,
        team_id: team_id_str.map(TeamId),
        ctftime_id: ctftime_id.map(|id| id as u32),
        fields,
        created_at,
    })
}

fn map_challenge(row: &sqlx::postgres::PgRow) -> Result<Challenge, sqlx::Error> {
    let id: String = row.get("id");
    let title: String = row.get("title");
    let description: String = row.get("description");
    let category: String = row.get("category");
    let points_mode: String = row.get("points_mode");
    let points_equation: String = row.get("points_equation");
    let flag_val: serde_json::Value = row.get("flag");
    let author_id: String = row.get("author_id");
    let author_username: String = row.get("author_username");
    let hints_val: serde_json::Value = row.get("hints");
    let files_val: serde_json::Value = row.get("files");
    let tags_val: serde_json::Value = row.get("tags");
    let requirements_val: serde_json::Value = row.get("requirements");
    let mode = match points_mode.as_str() {
        "PointAttribution" => ScoringMode::PointAttribution,
        _ => ScoringMode::PointValue,
    };
    let flag = serde_json::from_value(flag_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let hints = serde_json::from_value(hints_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let files = serde_json::from_value(files_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let tags = serde_json::from_value(tags_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let requirements =
        serde_json::from_value(requirements_val).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    Ok(Challenge {
        id,
        title: crate::libs::types::challenges::ChallengeTitle(title),
        description: crate::libs::types::challenges::ChallengeDescription(
            crate::libs::types::htmlstring::HtmlString(description),
        ),
        category: crate::libs::types::challenges::ChallengeCategory(category),
        points: crate::libs::types::challenges::ChallengePoints {
            mode,
            equation: points_equation,
        },
        flag,
        author: crate::libs::types::challenges::ChallengeAuthor {
            id: author_id,
            username: author_username,
        },
        hints,
        files,
        tags,
        requirements,
    })
}

fn map_submission(row: &sqlx::postgres::PgRow) -> Result<Submission, sqlx::Error> {
    let id: String = row.get("id");
    let challenge_id: String = row.get("challenge_id");
    let team_id_str: Option<String> = row.get("team_id");
    let account_id: String = row.get("account_id");
    let points: i32 = row.get("points");
    let provided_flag: String = row.get("provided_flag");
    let is_correct: bool = row.get("is_correct");
    let submitted_at: i64 = row.get("submitted_at");
    Ok(Submission {
        id: SubmissionId(id),
        challenge_id,
        team_id: team_id_str.map(TeamId),
        account_id: AccountId(account_id),
        points: points as u32,
        provided_flag,
        is_correct,
        submitted_at,
    })
}

#[async_trait]
impl AccountRepo for PgStore {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
        let row = sqlx::query("SELECT * FROM accounts WHERE id = $1")
            .bind(&id.0)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(map_account(&r)?)),
            None => Ok(None),
        }
    }
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError> {
        let row = sqlx::query("SELECT * FROM accounts WHERE username = $1")
            .bind(&name.0)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(map_account(&r)?)),
            None => Ok(None),
        }
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError> {
        let row = sqlx::query("SELECT * FROM accounts WHERE ctftime_id = $1")
            .bind(ctftime_id as i32)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(map_account(&r)?)),
            None => Ok(None),
        }
    }
    async fn save(&self, account: Account) -> Result<(), RepoError> {
        let role_str = match account.role {
            AccountRole::Admin => "Admin",
            AccountRole::Spectator => "Spectator",
            AccountRole::Player => "Player",
        };
        let fields_val = serde_json::to_value(&account.fields)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "INSERT INTO accounts (id, username, email, password_hash, role, team_id, ctftime_id, fields, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
        )
        .bind(&account.id.0)
        .bind(&account.username.0)
        .bind(account.email.map(|e| e.0))
        .bind(account.password_hash)
        .bind(role_str)
        .bind(account.team_id.map(|t| t.0))
        .bind(account.ctftime_id.map(|id| id as i32))
        .bind(fields_val)
        .bind(account.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update(&self, account: Account) -> Result<(), RepoError> {
        let role_str = match account.role {
            AccountRole::Admin => "Admin",
            AccountRole::Spectator => "Spectator",
            AccountRole::Player => "Player",
        };
        let fields_val = serde_json::to_value(&account.fields)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "UPDATE accounts SET username = $1, email = $2, password_hash = $3, role = $4, team_id = $5, ctftime_id = $6, fields = $7, created_at = $8 \
             WHERE id = $9"
        )
        .bind(&account.username.0)
        .bind(account.email.map(|e| e.0))
        .bind(account.password_hash)
        .bind(role_str)
        .bind(account.team_id.map(|t| t.0))
        .bind(account.ctftime_id.map(|id| id as i32))
        .bind(fields_val)
        .bind(account.created_at)
        .bind(&account.id.0)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl TeamRepo for PgStore {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError> {
        let row = sqlx::query("SELECT * FROM teams WHERE id = $1")
            .bind(&id.0)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(self.map_team(&r).await?)),
            None => Ok(None),
        }
    }
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError> {
        let row = sqlx::query("SELECT * FROM teams WHERE name = $1")
            .bind(&name.0)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(self.map_team(&r).await?)),
            None => Ok(None),
        }
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError> {
        let row = sqlx::query("SELECT * FROM teams WHERE ctftime_id = $1")
            .bind(ctftime_id as i32)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(self.map_team(&r).await?)),
            None => Ok(None),
        }
    }
    async fn save(&self, team: Team) -> Result<(), RepoError> {
        let fields_val =
            serde_json::to_value(&team.fields).map_err(|e| RepoError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO teams (id, name, ctftime_id, invite_code, captain_id, fields, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&team.id.0)
        .bind(&team.name.0)
        .bind(team.ctftime_id.map(|id| id as i32))
        .bind(team.invite_code)
        .bind(&team.captain_id.0)
        .bind(fields_val)
        .bind(team.create_at)
        .execute(&self.pool)
        .await?;
        for member_id in &team.member_ids {
            sqlx::query("UPDATE accounts SET team_id = $1 WHERE id = $2")
                .bind(&team.id.0)
                .bind(&member_id.0)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
    async fn update(&self, team: Team) -> Result<(), RepoError> {
        let fields_val =
            serde_json::to_value(&team.fields).map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "UPDATE teams SET name = $1, ctftime_id = $2, invite_code = $3, captain_id = $4, fields = $5, created_at = $6 \
             WHERE id = $7"
        )
        .bind(&team.name.0)
        .bind(team.ctftime_id.map(|id| id as i32))
        .bind(team.invite_code)
        .bind(&team.captain_id.0)
        .bind(fields_val)
        .bind(team.create_at)
        .bind(&team.id.0)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE accounts SET team_id = NULL WHERE team_id = $1")
            .bind(&team.id.0)
            .execute(&self.pool)
            .await?;
        for member_id in &team.member_ids {
            sqlx::query("UPDATE accounts SET team_id = $1 WHERE id = $2")
                .bind(&team.id.0)
                .bind(&member_id.0)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
    async fn find_all(&self) -> Result<Vec<Team>, RepoError> {
        let rows = sqlx::query("SELECT * FROM teams")
            .fetch_all(&self.pool)
            .await?;
        let mut teams = Vec::new();
        for r in rows {
            teams.push(self.map_team(&r).await?);
        }
        Ok(teams)
    }
}

#[async_trait]
impl InstanceRepo for PgStore {
    async fn find_active_flag(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Option<String>, RepoError> {
        let now = chrono::Utc::now().timestamp();
        let row = if let Some(t_id) = team_id {
            sqlx::query("SELECT flag FROM challenge_instances WHERE challenge_id = $1 AND team_id = $2 AND expires_at > $3")
                .bind(challenge_id)
                .bind(&t_id.0)
                .bind(now)
                .fetch_optional(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT flag FROM challenge_instances WHERE challenge_id = $1 AND account_id = $2 AND expires_at > $3")
                .bind(challenge_id)
                .bind(&account_id.0)
                .bind(now)
                .fetch_optional(&self.pool)
                .await?
        };
        match row {
            Some(r) => Ok(Some(r.get("flag"))),
            None => Ok(None),
        }
    }
}

#[async_trait]
impl ChallengeRepo for PgStore {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError> {
        let row = sqlx::query("SELECT * FROM challenges WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(map_challenge(&r)?)),
            None => Ok(None),
        }
    }
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError> {
        let rows = sqlx::query("SELECT * FROM challenges")
            .fetch_all(&self.pool)
            .await?;
        let mut challs = Vec::new();
        for r in rows {
            challs.push(map_challenge(&r)?);
        }
        Ok(challs)
    }
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError> {
        let mode_str = match challenge.points.mode {
            ScoringMode::PointValue => "PointValue",
            ScoringMode::PointAttribution => "PointAttribution",
        };
        let flag_val = serde_json::to_value(&challenge.flag)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let hints_val = serde_json::to_value(&challenge.hints)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let files_val = serde_json::to_value(&challenge.files)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let tags_val = serde_json::to_value(&challenge.tags)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let requirements_val = serde_json::to_value(&challenge.requirements)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "INSERT INTO challenges (id, title, description, category, points_mode, points_equation, flag, author_id, author_username, hints, files, tags, requirements) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"
        )
        .bind(&challenge.id)
        .bind(&challenge.title.0)
        .bind(&challenge.description.0.0)
        .bind(&challenge.category.0)
        .bind(mode_str)
        .bind(&challenge.points.equation)
        .bind(flag_val)
        .bind(&challenge.author.id)
        .bind(&challenge.author.username)
        .bind(hints_val)
        .bind(files_val)
        .bind(tags_val)
        .bind(requirements_val)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl SubmissionRepo for PgStore {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError> {
        let rows = sqlx::query("SELECT * FROM submissions")
            .fetch_all(&self.pool)
            .await?;
        let mut subs = Vec::new();
        for r in rows {
            subs.push(map_submission(&r)?);
        }
        Ok(subs)
    }
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError> {
        let rows = sqlx::query("SELECT * FROM submissions WHERE team_id = $1")
            .bind(&team_id.0)
            .fetch_all(&self.pool)
            .await?;
        let mut subs = Vec::new();
        for r in rows {
            subs.push(map_submission(&r)?);
        }
        Ok(subs)
    }
    async fn save(&self, submission: Submission) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO submissions (id, challenge_id, team_id, account_id, points, provided_flag, is_correct, submitted_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&submission.id.0)
        .bind(&submission.challenge_id)
        .bind(submission.team_id.map(|t| t.0))
        .bind(&submission.account_id.0)
        .bind(submission.points as i32)
        .bind(&submission.provided_flag)
        .bind(submission.is_correct)
        .bind(submission.submitted_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

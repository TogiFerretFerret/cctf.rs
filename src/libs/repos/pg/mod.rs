pub(crate) use crate::libs::repos::RepoError;
pub(crate) use crate::libs::repos::traits::{
    AccountRepo, ChallengeRepo, ConfigRepo, FileRepo, HintUnlockRepo, InstanceRepo,
    NotificationRepo, SubmissionRepo, TeamRepo,
};
pub(crate) use crate::libs::types::{
    accounts::{Account, AccountEmail, AccountId, AccountName, AccountRole},
    challenges::{Challenge, ScoringMode},
    config::CtfConfig,
    files::StoredFile,
    htmlstring::HtmlString,
    notifications::{Notification, NotificationId},
    solves::{HintUnlock, Submission},
    teams::{Team, TeamId, TeamName},
};
pub(crate) use async_trait::async_trait;
pub(crate) use sqlx::Row;

mod accounts;
mod challenges;
mod config;
mod files;
mod hints;
mod mappers;
mod notifications;
mod submissions;
mod teams;

pub struct PgStore {
    pool: sqlx::PgPool,
}

impl PgStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
    pub async fn init_db(&self) -> Result<(), sqlx::Error> {
        for stmt in SCHEMA_STATEMENTS {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
        Ok(())
    }
}

const SCHEMA_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS teams ( \
        id VARCHAR(64) PRIMARY KEY, \
        name VARCHAR(255) UNIQUE NOT NULL, \
        ctftime_id INT UNIQUE, \
        invite_code VARCHAR(255), \
        captain_id VARCHAR(64) NOT NULL, \
        bracket VARCHAR(100) DEFAULT 'Open' NOT NULL, \
        fields JSONB NOT NULL DEFAULT '{}', \
        created_at BIGINT NOT NULL \
     );",
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
        requirements JSONB NOT NULL DEFAULT '[]', \
        team_consensus BOOLEAN NOT NULL DEFAULT FALSE, \
        deployment JSONB NOT NULL DEFAULT '\"None\"', \
        visibility JSONB NOT NULL DEFAULT '\"Visible\"', \
        max_attempts JSONB \
     );",
    "CREATE TABLE IF NOT EXISTS submissions ( \
        id VARCHAR(64) PRIMARY KEY, \
        challenge_id VARCHAR(64) NOT NULL, \
        team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
        account_id VARCHAR(64) REFERENCES accounts(id) ON DELETE CASCADE NOT NULL, \
        points INT NOT NULL, \
        provided_flag TEXT NOT NULL, \
        is_correct BOOLEAN NOT NULL, \
        submitted_at BIGINT NOT NULL, \
        submitted_ip VARCHAR(64) DEFAULT '127.0.0.1' NOT NULL \
     );",
    "CREATE TABLE IF NOT EXISTS challenge_instances ( \
        id VARCHAR(64) PRIMARY KEY, \
        challenge_id VARCHAR(64) REFERENCES challenges(id) ON DELETE CASCADE NOT NULL, \
        team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
        account_id VARCHAR(64) REFERENCES accounts(id) ON DELETE CASCADE NOT NULL, \
        flag VARCHAR(255) NOT NULL, \
        cluster_ip VARCHAR(45) NOT NULL, \
        created_at BIGINT NOT NULL, \
        expires_at BIGINT NOT NULL \
     );",
    "CREATE TABLE IF NOT EXISTS ctf_config ( \
        id INT PRIMARY KEY, \
        data JSONB NOT NULL \
     );",
    "CREATE TABLE IF NOT EXISTS hint_unlocks ( \
        id VARCHAR(64) PRIMARY KEY, \
        challenge_id VARCHAR(64) NOT NULL, \
        hint_index INT NOT NULL, \
        team_id VARCHAR(64) REFERENCES teams(id) ON DELETE SET NULL, \
        account_id VARCHAR(64) REFERENCES accounts(id) ON DELETE CASCADE NOT NULL, \
        cost INT NOT NULL, \
        unlocked_at BIGINT NOT NULL \
     );",
    "CREATE TABLE IF NOT EXISTS files ( \
        id VARCHAR(64) PRIMARY KEY, \
        name TEXT NOT NULL, \
        checksum_sha256 VARCHAR(64) NOT NULL, \
        size BIGINT NOT NULL, \
        content_type VARCHAR(255) NOT NULL, \
        uploaded_at BIGINT NOT NULL \
     );",
    "CREATE TABLE IF NOT EXISTS notifications ( \
        id VARCHAR(64) PRIMARY KEY, \
        kind JSONB NOT NULL, \
        title TEXT NOT NULL, \
        message TEXT NOT NULL, \
        target JSONB NOT NULL, \
        created_bigint NOT NULL \
    );",
];

#[cfg(test)]
mod tests {
    use super::SCHEMA_STATEMENTS;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    #[test]
    fn schema_statements_are_valid_postgres() {
        let dialect = PostgreSqlDialect {};
        for stmt in SCHEMA_STATEMENTS {
            let parsed = Parser::parse_sql(&dialect, stmt);
            assert!(
                parsed.is_ok(),
                "malformed DDL statement:\n{stmt}\nerror: {:?}",
                parsed.err()
            );
            assert_eq!(
                parsed.as_ref().unwrap().len(),
                1,
                "expected exactly one statement per entry, got {}:\n{stmt}",
                parsed.unwrap().len()
            );
        }
    }

    #[test]
    fn challenges_table_has_all_columns() {
        let challenges = SCHEMA_STATEMENTS
            .iter()
            .find(|s| s.contains("CREATE TABLE IF NOT EXISTS challenges"))
            .expect("challenges table statement present");
        for col in [
            "deployment",
            "visibility",
            "max_attempts",
            "team_consensus",
            "requirements",
        ] {
            assert!(
                challenges.contains(col),
                "challenges schema missing column `{col}`"
            );
        }
        let dialect = PostgreSqlDialect {};
        Parser::parse_sql(&dialect, challenges).expect("challenges DDL parses as valid PostgreSQL");
    }
}

use super::mappers::map_account;
use super::*;

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
    async fn find_all(&self) -> Result<Vec<Account>, RepoError> {
        let rows = sqlx::query("SELECT * FROM accounts")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            out.push(map_account(&r)?);
        }
        Ok(out)
    }
}

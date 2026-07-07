use super::*;

impl PgStore {
    async fn map_team(&self, row: &sqlx::postgres::PgRow) -> Result<Team, sqlx::Error> {
        let id: String = row.get("id");
        let name: String = row.get("name");
        let ctftime_id: Option<i32> = row.get("ctftime_id");
        let invite_code: Option<String> = row.get("invite_code");
        let captain_id: String = row.get("captain_id");
        let bracket: String = row.get("bracket");
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
            bracket,
            fields,
            create_at,
        })
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
            "INSERT INTO teams (id, name, ctftime_id, invite_code, captain_id, bracket, fields, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&team.id.0)
        .bind(&team.name.0)
        .bind(team.ctftime_id.map(|id| id as i32))
        .bind(team.invite_code)
        .bind(&team.captain_id.0)
        .bind(&team.bracket)
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
            "UPDATE teams SET name = $1, ctftime_id = $2, invite_code = $3, captain_id = $4, bracket = $5, fields = $6, created_at = $7 \
             WHERE id = $8"
        )
        .bind(&team.name.0)
        .bind(team.ctftime_id.map(|id| id as i32))
        .bind(team.invite_code)
        .bind(&team.captain_id.0)
        .bind(&team.bracket)
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

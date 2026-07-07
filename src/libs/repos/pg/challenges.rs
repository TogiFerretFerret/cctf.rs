use super::mappers::map_challenge;
use super::*;

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
    async fn get_instance_ip(&self, instance_id: &str) -> Result<Option<String>, RepoError> {
        let now = chrono::Utc::now().timestamp();
        let row = sqlx::query(
            "SELECT cluster_ip FROM challenge_instances WHERE id = $1 AND expires_at > $2",
        )
        .bind(instance_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => Ok(Some(r.try_get("cluster_ip")?)),
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
            ScoringMode::DynamicDecay { .. } => "DynamicDecay",
        };
        let eq_str = match challenge.points.mode {
            ScoringMode::DynamicDecay {
                initial,
                minimum,
                decay,
            } => {
                format!("{},{},{}", initial, minimum, decay)
            }
            _ => challenge.points.equation.clone(),
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
        let deployment_val = serde_json::to_value(&challenge.deployment)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let visibility_val = serde_json::to_value(&challenge.visibility)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let max_attempts_val = serde_json::to_value(&challenge.max_attempts)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "INSERT INTO challenges (id, title, description, category, points_mode, points_equation, flag, author_id, author_username, hints, files, tags, requirements, team_consensus, deployment, visibility, max_attempts) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)"
        )
        .bind(&challenge.id)
        .bind(&challenge.title.0)
        .bind(&challenge.description.0.0)
        .bind(&challenge.category.0)
        .bind(mode_str)
        .bind(eq_str)
        .bind(flag_val)
        .bind(&challenge.author.id)
        .bind(&challenge.author.username)
        .bind(hints_val)
        .bind(files_val)
        .bind(tags_val)
        .bind(requirements_val)
        .bind(challenge.team_consensus)
        .bind(deployment_val)
        .bind(visibility_val)
        .bind(max_attempts_val)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update(&self, challenge: Challenge) -> Result<(), RepoError> {
        let mode_str = match challenge.points.mode {
            ScoringMode::PointValue => "PointValue",
            ScoringMode::PointAttribution => "PointAttribution",
            ScoringMode::DynamicDecay { .. } => "DynamicDecay",
        };
        let eq_str = match challenge.points.mode {
            ScoringMode::DynamicDecay {
                initial,
                minimum,
                decay,
            } => format!("{},{},{}", initial, minimum, decay),
            _ => challenge.points.equation.clone(),
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
        let deployment_val = serde_json::to_value(&challenge.deployment)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let visibility_val = serde_json::to_value(&challenge.visibility)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        let max_attempts_val = serde_json::to_value(&challenge.max_attempts)
            .map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "UPDATE challenges SET title = $2, description = $3, category = $4, points_mode = $5, points_equation = $6, flag = $7, author_id = $8, author_username = $9, hints = $10, files = $11, tags = $12, requirements = $13, team_consensus = $14, deployment = $15 WHERE id = $1"
        )
        .bind(&challenge.id)
        .bind(&challenge.title.0)
        .bind(&challenge.description.0.0)
        .bind(&challenge.category.0)
        .bind(mode_str)
        .bind(eq_str)
        .bind(flag_val)
        .bind(&challenge.author.id)
        .bind(&challenge.author.username)
        .bind(hints_val)
        .bind(files_val)
        .bind(tags_val)
        .bind(requirements_val)
        .bind(challenge.team_consensus)
        .bind(deployment_val)
        .bind(visibility_val)
        .bind(max_attempts_val)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete(&self, id: &str, delete_solves: bool) -> Result<(), RepoError> {
        if delete_solves {
            sqlx::query("DELETE FROM submissions WHERE challenge_id = $1")
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        sqlx::query("DELETE FROM challenges WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

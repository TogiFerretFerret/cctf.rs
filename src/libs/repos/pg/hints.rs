use super::mappers::map_hint_unlock;
use super::*;

#[async_trait]
impl HintUnlockRepo for PgStore {
    async fn find_all(&self) -> Result<Vec<HintUnlock>, RepoError> {
        let rows = sqlx::query("SELECT * FROM hint_unlocks")
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            out.push(map_hint_unlock(&r)?);
        }
        Ok(out)
    }
    async fn find_for(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Vec<HintUnlock>, RepoError> {
        let rows = if let Some(t) = team_id {
            sqlx::query("SELECT * FROM hint_unlocks WHERE challenge_id = $1 AND team_id = $2")
                .bind(challenge_id)
                .bind(&t.0)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM hint_unlocks WHERE challenge_id = $1 AND account_id = $2 AND team_id IS NULL")
                .bind(challenge_id)
                .bind(&account_id.0)
                .fetch_all(&self.pool)
                .await?
        };
        let mut out = Vec::new();
        for r in rows {
            out.push(map_hint_unlock(&r)?);
        }
        Ok(out)
    }
    async fn save(&self, unlock: HintUnlock) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO hint_unlocks (id, challenge_id, hint_index, team_id, account_id, cost, unlocked_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(&unlock.id.0)
            .bind(&unlock.challenge_id)
            .bind(unlock.hint_index as i32)
            .bind(unlock.team_id.map(|t| t.0))
            .bind(&unlock.account_id.0)
            .bind(unlock.cost as i32)
            .bind(unlock.unlocked_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

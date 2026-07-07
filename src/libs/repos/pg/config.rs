use super::*;

#[async_trait]
impl ConfigRepo for PgStore {
    async fn get(&self) -> Result<CtfConfig, RepoError> {
        let row = sqlx::query("SELECT data FROM ctf_config WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => {
                let data: serde_json::Value = r.get("data");
                serde_json::from_value(data).map_err(|e| RepoError::Internal(e.to_string()))
            }
            None => Ok(CtfConfig::default()),
        }
    }

    async fn set(&self, config: CtfConfig) -> Result<(), RepoError> {
        let data = serde_json::to_value(&config).map_err(|e| RepoError::Internal(e.to_string()))?;
        sqlx::query(
            "INSERT INTO ctf_config (id, data) VALUES (1, $1) \
             ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data",
        )
        .bind(data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

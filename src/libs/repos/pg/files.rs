use super::*;

#[async_trait]
impl FileRepo for PgStore {
    async fn save(&self, file: StoredFile) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO files (id, name, checksum_sha256, size, content_type, uploaded_at) \
            VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&file.id)
        .bind(&file.name)
        .bind(&file.checksum_sha256)
        .bind(file.size as i64)
        .bind(&file.content_type)
        .bind(file.uploaded_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<StoredFile>, RepoError> {
        let row = sqlx::query("SELECT * FROM files WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| StoredFile {
            id: r.get("id"),
            name: r.get("name"),
            checksum_sha256: r.get("checksum_sha256"),
            size: r.get::<i64, _>("size") as u64,
            content_type: r.get("content_type"),
            uploaded_at: r.get("uploaded_at"),
        }))
    }
    async fn delete(&self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM files WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

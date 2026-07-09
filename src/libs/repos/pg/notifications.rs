use super::*;

#[async_trait]
impl NotificationRepo for PgStore {
    async fn save(&self, notification: Notification) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO notifications (id, kind, title, message, target, created_at) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&notification.id.0)
            .bind(serde_json::to_value(&notification.kind).map_err(|e| RepoError::Internal(e.to_string()))?)
            .bind(&notification.title)
            .bind(&notification.message.0)
            .bind(serde_json::to_value(&notification.target).map_err(|e| RepoError::Internal(e.to_string()))?)
            .bind(notification.created_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    async fn list_recent(&self, limit: i64) -> Result<Vec<Notification>, RepoError> {
        let rows = sqlx::query("SELECT * FROM notifications ORDER BY created_at DESC LIMIT $1")
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::new();
        for r in rows {
            out.push(Notification {
                id: NotificationId(r.get("id")),
                kind: serde_json::from_value(r.get("kind"))
                    .map_err(|e| RepoError::Internal(e.to_string()))?,
                title: r.get("title"),
                message: HtmlString(r.get("message")),
                target: serde_json::from_value(r.get("target"))
                    .map_err(|e| RepoError::Internal(e.to_string()))?,
                created_at: r.get("created_at"),
            });
        }
        Ok(out)
    }
}

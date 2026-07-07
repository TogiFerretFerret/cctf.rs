use super::mappers::map_submission;
use super::*;

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
            "INSERT INTO submissions (id, challenge_id, team_id, account_id, points, provided_flag, is_correct, submitted_at, submitted_ip) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
        )
        .bind(&submission.id.0)
        .bind(&submission.challenge_id)
        .bind(submission.team_id.map(|t| t.0))
        .bind(&submission.account_id.0)
        .bind(submission.points as i32)
        .bind(&submission.provided_flag)
        .bind(submission.is_correct)
        .bind(submission.submitted_at)
        .bind(&submission.submitted_ip)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

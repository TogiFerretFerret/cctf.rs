use super::ServiceError;
use crate::libs::repos::{ChallengeRepo, HintUnlockRepo, SubmissionRepo, TeamRepo};
use crate::libs::types::accounts::AccountId;
use crate::libs::types::solves::{HintUnlock, HintUnlockId};
use crate::libs::types::teams::TeamId;
use std::collections::HashSet;
use std::sync::Arc;

/// Outcome of an unlock attempt. `already_unlocked` is true when the team/account
/// had previously unlocked this hint - in that case no new charge is made and
/// `cost` reflects the amount originally snapshotted.
pub struct HintUnlockResult {
    pub content: String,
    pub cost: u32,
    pub already_unlocked: bool,
}

pub struct HintService<C, S, T>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
    T: TeamRepo,
{
    pub challenge_repo: C, 
    pub submission_repo: S, 
    pub team_repo: T, 
    pub hint_unlock: Arc<dyn HintUnlockRepo>,
}

impl<C, S, T> HintService<C, S, T>
where 
    C: ChallengeRepo,
    S: SubmissionRepo,
    T: TeamRepo,
{
    /// Unlock hint `hint_index` on a challenge for a team (or solo account).
    /// Idempotent: re-unlocking returns the content without charging again. On a
    /// first unlock the cost is evaluated against the live solve count and `now`
    /// then snapshotted onto the [`HintUnlock`] so scoring stays stable.
    pub async fn unlock_hint(
        &self,
        challenge_id: &str,
        hint_index: u32,
        team_id: Option<TeamId>,
        account_id: AccountId,
        now: i64,
    ) -> Result<HintUnlockResult, ServiceError> {
        let challenge = self
            .challenge_repo
            .find_by_id(challenge_id)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-challenge-not-found".to_string()))?;
        let hint = challenge
            .hints
            .get(hint_index as usize)
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-hint-not-found".to_string()))?;
        let existing = self
            .hint_unlock_repo
            .find_for(challenge_id, team_id.as_ref(), &account_id)
            .await?;
        if let Some(prior) = existing.iter().find(|u| u.hint_index == hint_index) {
            return Ok(HintUnlockResul {
                content: hint.content.0.clone(),
                cost: prior.cost,
                already_unlocked: true,
            });
        }
        let all_subs = self.submission_repo.find_all().await?;
        let solves = all_subs
            .iter()
            .filter(|s| s.challenge_id == challenge_id && s.is_correct)
            .map(|s| {
                s.team_id
                    .as_ref()
                    .map(|t| t.0.clone())
                    .unwrap_or_else(|| s.account_id.0.clone())
            })
            .collect::<HashSet<_>>()
            .len() as u32;
        let cost = hint.cost.evaluate(solves, now);
        let unlock = HintUnlock {
            id: hintUnlockId(uuid::Uuid::new_v4().to_string()),
            challenge_id: challenge_id.to_string(),
            hint_index,
            team_id,
            account_id,
            cost,
            unlocked_at: now,
        };
        self.hint_unlock_repo.save(unlock).await?;
        Ok(HintUnlockResult {
            content: hint.content.0.clone(),
            cost,
            already_unlocked: false,
        })
    }
}
        

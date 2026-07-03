use super::ServiceError;
use super::scoreboard::{build_challenge_solvers, compute_team_solve_score, total_hint_cost};
use crate::libs::repos::{ChallengeRepo, HintUnlockRepo, SubmissionRepo, TeamRepo};
use crate::libs::types::{
    accounts::AccountId,
    config::HintDeductionMode,
    htmlstring::HtmlString,
    solves::{HintUnlock, HintUnlockId},
    teams::TeamId,
};
use std::collections::HashSet;
use std::sync::Arc;

/// Outcome of an unlock attempt. `already_unlocked` is true when the team/account
/// had previously unlocked this hint - in that case no new charge is made and
/// `cost` reflects the amount originally snapshotted.
pub struct HintUnlockResult {
    pub content: HtmlString,
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
    pub hint_unlock_repo: Arc<dyn HintUnlockRepo>,
    pub hint_deduction_mode: HintDeductionMode,
}

impl<C, S, T> HintService<C, S, T>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
    T: TeamRepo,
{
    /// All unlocks belonging to a viewer (team if on one, else the solo account),
    /// across ever challenge - used to mark hint state on the challenge board.
    pub async fn viewer_unlocks(
        &self,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Vec<HintUnlock>, ServiceError> {
        let all = self.hint_unlock_repo.find_all().await?;
        Ok(all
            .into_iter()
            .filter(|u| match team_id {
                Some(t) => u.team_id.as_ref() == Some(t),
                None => u.team_id.is_none() && &u.account_id == account_id,
            })
            .collect())
    }
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
            return Ok(HintUnlockResult {
                content: hint.content.clone(),
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
        if self.hint_deduction_mode == HintDeductionMode::Gate
            && let Some(ref t) = team_id
        {
            let team = self
                .team_repo
                .find_by_id(t)
                .await?
                .ok_or_else(|| ServiceError::InvalidRequest("ctf-team-not-found".to_string()))?;
            let challenges = self.challenge_repo.find_all().await?;
            let solvers = build_challenge_solvers(&all_subs);
            let solve_points =
                compute_team_solve_score(&team, &challenges, &all_subs, &solvers).points;
            let all_unlocks = self.hint_unlock_repo.find_all().await?;
            let prior_spent = total_hint_cost(&all_unlocks, Some(t), None);
            if solve_points - prior_spent < cost as i64 {
                return Err(ServiceError::InvalidRequest(
                    "ctf-insufficient-points".to_string(),
                ));
            }
        }
        let unlock = HintUnlock {
            id: HintUnlockId(uuid::Uuid::new_v4().to_string()),
            challenge_id: challenge_id.to_string(),
            hint_index,
            team_id,
            account_id,
            cost,
            unlocked_at: now,
        };
        self.hint_unlock_repo.save(unlock).await?;
        Ok(HintUnlockResult {
            content: hint.content.clone(),
            cost,
            already_unlocked: false,
        })
    }
}

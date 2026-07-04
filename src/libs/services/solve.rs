use super::ServiceError;
use crate::libs::repos::{ChallengeRepo, SubmissionRepo, TeamRepo};
use crate::libs::types::{
    accounts::AccountId,
    challenges::{AttemptCountMode, ScoringMode},
    flags::FlagValidator,
    solves::{Submission, SubmissionId},
    teams::TeamId,
};
use std::collections::{HashMap, HashSet};

/// Dynamic-decay scoring: value starts at `initial` and decays toward `minimum`
/// as more teams solve it (`decay` controls how fast). First solve
/// (`solve_count <= 1`) always earns the full `initial`.
///
/// ```
/// use cctf_rs::libs::services::solve::calculate_dynamic_points;
///
/// // First blood earns the full value.
/// assert_eq!(calculate_dynamic_points(500, 100, 10, 1), 500);
///
/// // More solves → fewer points, but never below the minimum.
/// let many = calculate_dynamic_points(500, 100, 10, 50);
/// assert!(many >= 100 && many < 500);
/// ```
pub fn calculate_dynamic_points(initial: u32, minimum: u32, decay: u32, solve_count: u32) -> u32 {
    if solve_count <= 1 {
        return initial;
    }
    let x = (solve_count - 1) as f64;
    let d = decay as f64;
    let ratio = 1.0 / (1.0 + (x * x) / (d * d));
    let points = (initial as f64 - minimum as f64) * ratio + minimum as f64;
    points.round() as u32
}

pub struct SolveService<C, S, T>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
    T: TeamRepo,
{
    pub challenge_repo: C,
    pub submission_repo: S,
    pub team_repo: T,
}

impl<C, S, T> SolveService<C, S, T>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
    T: TeamRepo,
{
    pub async fn submit_flag(
        &self,
        challenge_id: &str,
        team_id: Option<TeamId>,
        account_id: AccountId,
        submitted_flag: &str,
        submitted_ip: &str,
    ) -> Result<Submission, ServiceError> {
        let challenge = self
            .challenge_repo
            .find_by_id(challenge_id)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-challenge-not-found".to_string()))?;
        if let Some(ref limit_cfg) = challenge.max_attempts {
            let all = if let Some(ref t_id) = team_id {
                self.submission_repo.find_by_team(t_id).await?
            } else {
                self.submission_repo.find_all().await?
            };
            let prior: Vec<&Submission> = all
                .iter()
                .filter(|s| {
                    s.challenge_id == challenge_id
                        && (team_id.is_some() || s.account_id == account_id)
                    })
                .collect();
            let attempts = match limit_cfg.mode {
                AttemptCountMode::All => prior.len(),
                AttemptCountMode::IncorrectOnly => prior.iter().filter(|s| !s.is_correct).count(),
                AttemptCountMode::Unique => prior
                    .iter()
                    .map(|s| s.provided_flag.trim())
                    .collect::<HashSet<_>>()
                    .len(),
                AttemptCountMode::UniqueIncorrect => prior
                    .iter()
                    .filter(|s| !s.is_correct)
                    .map(|s| s.provided_flag.trim())
                    .collect::<HashSet<_>>()
                    .len(),
            } as u32;
            if attempts >= limit_cfg.limit {
                return Err(ServiceError::InvalidRequest(
                        "ctf-max-attempts-reached".to_string(),
                    ));
            }
        }
        let active_flag: Option<String> = self
            .challenge_repo
            .find_active_flag(challenge_id, team_id.as_ref(), &account_id)
            .await?;

        let matched_partial = challenge
            .flag
            .find_matching_partial(submitted_flag, active_flag.as_deref());

        let is_correct = if let FlagValidator::Multi(_) = &challenge.flag {
            matched_partial.is_some()
        } else {
            challenge
                .flag
                .is_match(submitted_flag, active_flag.as_deref())
        };

        let existing_correct_subs = if let Some(ref t_id) = team_id {
            let subs = self.submission_repo.find_by_team(t_id).await?;
            subs.into_iter()
                .filter(|s| s.challenge_id == challenge_id && s.is_correct)
                .collect::<Vec<_>>()
        } else {
            let subs = self.submission_repo.find_all().await?;
            subs.into_iter()
                .filter(|s| {
                    s.challenge_id == challenge_id && s.account_id == account_id && s.is_correct
                })
                .collect::<Vec<_>>()
        };

        if is_correct {
            if let Some(matched_pf) = matched_partial {
                let already_solved_pf = existing_correct_subs.iter().any(|s| {
                    matched_pf
                        .validator
                        .is_match(&s.provided_flag, active_flag.as_deref())
                });
                if already_solved_pf {
                    return Err(ServiceError::InvalidRequest(
                        "ctf-already-solved".to_string(),
                    ));
                }
            } else if challenge.team_consensus {
                let user_already_solved = existing_correct_subs
                    .iter()
                    .any(|s| s.account_id == account_id);
                if user_already_solved {
                    return Err(ServiceError::InvalidRequest(
                        "ctf-already-solved".to_string(),
                    ));
                }
            } else if !existing_correct_subs.is_empty() {
                return Err(ServiceError::InvalidRequest(
                    "ctf-already-solved".to_string(),
                ));
            }
        }

        let all_subs = self.submission_repo.find_all().await?;
        let solve_count = if team_id.is_some() {
            let mut team_solves = HashMap::new();
            for s in all_subs {
                if s.challenge_id == challenge_id
                    && s.is_correct
                    && let Some(ref t_id) = s.team_id
                {
                    team_solves
                        .entry(t_id.clone())
                        .or_insert_with(Vec::new)
                        .push(s.account_id);
                }
            }
            let mut full_solve_teams = 0;
            for (t_id, user_ids) in team_solves {
                if let Ok(Some(team)) = self.team_repo.find_by_id(&t_id).await {
                    let reached_consensus = if challenge.team_consensus {
                        !team.member_ids.is_empty()
                            && team
                                .member_ids
                                .iter()
                                .all(|member_id| user_ids.contains(member_id))
                    } else {
                        !user_ids.is_empty()
                    };
                    if reached_consensus {
                        full_solve_teams += 1;
                    }
                }
            }
            full_solve_teams
        } else {
            all_subs
                .iter()
                .filter(|s| s.challenge_id == challenge_id && s.is_correct)
                .map(|s| s.account_id.clone())
                .collect::<HashSet<_>>()
                .len() as u32
        };

        let reaches_consensus = if let Some(ref t_id) = team_id {
            if challenge.team_consensus {
                if let Ok(Some(team)) = self.team_repo.find_by_id(t_id).await {
                    team.member_ids.iter().all(|member_id| {
                        *member_id == account_id
                            || existing_correct_subs
                                .iter()
                                .any(|s| s.account_id == *member_id)
                    })
                } else {
                    false
                }
            } else {
                existing_correct_subs.is_empty()
            }
        } else {
            true
        };

        let next_solve_count = if reaches_consensus {
            solve_count + 1
        } else {
            solve_count
        };

        let points_awarded = if is_correct {
            let base_points = match challenge.points.mode {
                ScoringMode::PointValue => challenge.points.equation.parse::<u32>().unwrap_or(100),
                ScoringMode::PointAttribution => {
                    challenge.points.equation.parse::<u32>().unwrap_or(100)
                }
                ScoringMode::DynamicDecay {
                    initial,
                    minimum,
                    decay,
                } => calculate_dynamic_points(initial, minimum, decay, next_solve_count.max(1)),
            };
            if let Some(matched_pf) = matched_partial {
                (base_points as f64 * matched_pf.weight).round() as u32
            } else {
                base_points
            }
        } else {
            0
        };

        let submission = Submission {
            id: SubmissionId(uuid::Uuid::new_v4().to_string()),
            challenge_id: challenge_id.to_string(),
            team_id,
            account_id,
            points: points_awarded,
            provided_flag: submitted_flag.to_string(),
            is_correct,
            submitted_at: chrono::Utc::now().timestamp(),
            submitted_ip: submitted_ip.to_string(),
        };

        self.submission_repo.save(submission.clone()).await?;

        if !is_correct {
            return Err(ServiceError::InvalidRequest(
                "ctf-incorrect-flag".to_string(),
            ));
        }

        Ok(submission)
    }
}

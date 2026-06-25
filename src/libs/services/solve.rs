use crate::libs::repos::{ChallengeRepo, SubmissionRepo, TeamRepo};
use crate::libs::types::accounts::AccountId;
use crate::libs::types::challenges::ScoringMode;
use crate::libs::types::flags::FlagValidator;
use crate::libs::types::solves::{Submission, SubmissionId};
use crate::libs::types::teams::TeamId;
use super::ServiceError;

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
    ) -> Result<Submission, ServiceError> {
        let challenge = self
            .challenge_repo
            .find_by_id(challenge_id)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-challenge-not-found".to_string()))?;

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
                    s.challenge_id == challenge_id
                        && s.account_id == account_id
                        && s.is_correct
                })
                .collect::<Vec<_>>()
        };

        if is_correct {
            if let Some(ref matched_pf) = matched_partial {
                // Multi-flag duplicate check
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
                // Team Consensus duplicate check
                let user_already_solved = existing_correct_subs
                    .iter()
                    .any(|s| s.account_id == account_id);
                if user_already_solved {
                    return Err(ServiceError::InvalidRequest(
                        "ctf-already-solved".to_string(),
                    ));
                }
            } else {
                // Normal challenge duplicate check
                if !existing_correct_subs.is_empty() {
                    return Err(ServiceError::InvalidRequest(
                        "ctf-already-solved".to_string(),
                    ));
                }
            }
        }

        let all_subs = self.submission_repo.find_all().await?;
        let solve_count = if team_id.is_some() {
            all_subs
                .iter()
                .filter(|s| s.challenge_id == challenge_id && s.is_correct)
                .map(|s| s.team_id.clone())
                .filter_map(|x| x)
                .collect::<std::collections::HashSet<_>>()
                .len() as u32
        } else {
            all_subs
                .iter()
                .filter(|s| s.challenge_id == challenge_id && s.is_correct)
                .map(|s| s.account_id.clone())
                .collect::<std::collections::HashSet<_>>()
                .len() as u32
        };

        let points_awarded = if is_correct {
            let next_solve_count = solve_count + 1;
            let base_points = match challenge.points.mode {
                ScoringMode::PointValue => {
                    challenge.points.equation.parse::<u32>().unwrap_or(100)
                }
                ScoringMode::PointAttribution => {
                    challenge.points.equation.parse::<u32>().unwrap_or(100)
                }
                ScoringMode::DynamicDecay {
                    initial,
                    minimum,
                    decay,
                } => calculate_dynamic_points(initial, minimum, decay, next_solve_count),
            };
            if let Some(ref matched_pf) = matched_partial {
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

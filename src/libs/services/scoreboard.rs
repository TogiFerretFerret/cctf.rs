use super::ServiceError;
use super::solve::calculate_dynamic_points;
use crate::libs::repos::{ChallengeRepo, HintUnlockRepo, SubmissionRepo, TeamRepo};
use crate::libs::types::{
    challenges::{Challenge, ScoringMode},
    config::HintDeductionMode,
    flags::FlagValidator,
    scoreboard::{
        CtfTimeScoreboardExport, CtfTimeStandingsEntry, CtfTimeTaskStats, ScoreboardEntry,
    },
    solves::Submission,
    teams::{Team, TeamId},
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Distinct correct solvers per challenge (team id if a team submission, else
/// account id), used to drive dynamic-decay scoring.
pub(crate) fn build_challenge_solvers(
    submissions: &[Submission],
) -> HashMap<String, HashSet<String>> {
    let mut solvers: HashMap<String, HashSet<String>> = HashMap::new();
    for sub in submissions {
        if sub.is_correct {
            let solver_id = if sub.team_id.is_some() {
                sub.team_id.as_ref().unwrap().0.clone()
            } else {
                sub.account_id.0.clone()
            };
            solvers
                .entry(sub.challenge_id.clone())
                .or_default()
                .insert(solver_id);
        }
    }
    solvers
}

/// A team's score from solves alone (before any hint deduction), plus the data
/// the scoreboard needs. Shared by the scoreboard and the hint gate so both
/// agree on exactly what a team has earned.
pub(crate) struct TeamSolveScore {
    pub points: i64,
    pub last_solve_time: Option<i64>,
    pub solved_ids: Vec<String>,
}

pub(crate) fn compute_team_solve_score(
    team: &Team,
    challenges: &[Challenge],
    submissions: &[Submission],
    challenge_solvers: &HashMap<String, HashSet<String>>,
) -> TeamSolveScore {
    let team_subs: Vec<&Submission> = submissions
        .iter()
        .filter(|s| s.team_id.as_ref() == Some(&team.id))
        .collect();

    let mut points: i64 = 0;
    let mut last_solve_time = None;
    let mut solved_ids = Vec::new();

    for challenge in challenges {
        let solve_count = challenge_solvers
            .get(&challenge.id)
            .map(|s| s.len())
            .unwrap_or(0) as u32;

        let decayed_points = match challenge.points.mode {
            ScoringMode::PointValue => challenge.points.equation.parse::<u32>().unwrap_or(100),
            ScoringMode::PointAttribution => {
                challenge.points.equation.parse::<u32>().unwrap_or(100)
            }
            ScoringMode::DynamicDecay {
                initial,
                minimum,
                decay,
            } => calculate_dynamic_points(initial, minimum, decay, solve_count),
        };

        let mut challenge_scored = false;

        if let FlagValidator::Multi(ref partials) = challenge.flag {
            for pf in partials {
                let pf_subs: Vec<&Submission> = team_subs
                    .iter()
                    .filter(|s| {
                        s.challenge_id == challenge.id
                            && s.is_correct
                            && pf
                                .validator
                                .is_match(&s.provided_flag, Some(&s.provided_flag))
                    })
                    .cloned()
                    .collect();

                let scored_this_part = if challenge.team_consensus {
                    !team.member_ids.is_empty()
                        && team
                            .member_ids
                            .iter()
                            .all(|member_id| pf_subs.iter().any(|s| s.account_id == *member_id))
                } else {
                    !pf_subs.is_empty()
                };

                if scored_this_part {
                    let part_points = (decayed_points as f64 * pf.weight).round() as i64;
                    points += part_points;
                    challenge_scored = true;

                    let max_sub_time = pf_subs.iter().map(|s| s.submitted_at).max();
                    if let Some(sub_time) = max_sub_time {
                        last_solve_time = match last_solve_time {
                            None => Some(sub_time),
                            Some(t) => Some(t.max(sub_time)),
                        };
                    }
                }
            }
        } else {
            let c_subs: Vec<&Submission> = team_subs
                .iter()
                .filter(|s| s.challenge_id == challenge.id && s.is_correct)
                .cloned()
                .collect();

            let scored_challenge = if challenge.team_consensus {
                !team.member_ids.is_empty()
                    && team
                        .member_ids
                        .iter()
                        .all(|member_id| c_subs.iter().any(|s| s.account_id == *member_id))
            } else {
                !c_subs.is_empty()
            };

            if scored_challenge {
                points += decayed_points as i64;
                challenge_scored = true;

                let max_sub_time = c_subs.iter().map(|s| s.submitted_at).max();
                if let Some(sub_time) = max_sub_time {
                    last_solve_time = match last_solve_time {
                        None => Some(sub_time),
                        Some(t) => Some(t.max(sub_time)),
                    };
                }
            }
        }

        if challenge_scored {
            solved_ids.push(challenge.id.clone());
        }
    }

    TeamSolveScore {
        points,
        last_solve_time,
        solved_ids,
    }
}

/// Total hint cost a team (or solo account) has committed to, from a set of
/// unlocks. Used both for scoreboard deduction and the affordability gate.
pub(crate) fn total_hint_cost(
    unlocks: &[crate::libs::types::solves::HintUnlock],
    team_id: Option<&TeamId>,
    account_id: Option<&crate::libs::types::accounts::AccountId>,
) -> i64 {
    unlocks
        .iter()
        .filter(|u| match team_id {
            Some(t) => u.team_id.as_ref() == Some(t),
            None => u.team_id.is_none() && Some(&u.account_id) == account_id,
        })
        .map(|u| u.cost as i64)
        .sum()
}

/// Apply a [`HintDeductionMode`] to a team's solve points given the cost spent.
pub(crate) fn apply_hint_deduction(points: i64, spent: i64, mode: &HintDeductionMode) -> i64 {
    match mode {
        HintDeductionMode::None => points,
        HintDeductionMode::AllowNegative => points - spent,
        HintDeductionMode::FloorZero | HintDeductionMode::Gate => (points - spent).max(0),
    }
}

pub struct ScoreboardService<T, C, S>
where
    T: TeamRepo,
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub team_repo: T,
    pub challenge_repo: C,
    pub submission_repo: S,
    pub sort_by_accuracy: bool,
    pub freeze_time: Option<i64>,
    pub hint_unlock_repo: Arc<dyn HintUnlockRepo>,
    pub hint_deduction_mode: HintDeductionMode,
}

impl<T, C, S> ScoreboardService<T, C, S>
where
    T: TeamRepo,
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub async fn get_scoreboard(
        &self,
        filter_bracket: Option<&str>,
    ) -> Result<Vec<ScoreboardEntry>, ServiceError> {
        let mut teams = self.team_repo.find_all().await?;
        if let Some(bracket) = filter_bracket {
            teams.retain(|team| team.bracket == bracket);
        }
        let submissions = self.submission_repo.find_all().await?;
        let submissions = if let Some(freeze) = self.freeze_time {
            submissions
                .into_iter()
                .filter(|s| s.submitted_at < freeze)
                .collect::<Vec<_>>()
        } else {
            submissions
        };
        let challenges = self.challenge_repo.find_all().await?;

        let challenge_solvers = build_challenge_solvers(&submissions);
        let hint_unlocks = self.hint_unlock_repo.find_all().await?;
        let hint_unlocks = if let Some(freeze) = self.freeze_time {
            hint_unlocks
                .into_iter()
                .filter(|u| u.unlocked_at < freeze)
                .collect::<Vec<_>>()
        } else {
            hint_unlocks
        };
        let mut entries = Vec::new();
        for team in teams {
            let score =
                compute_team_solve_score(&team, &challenges, &submissions, &challenge_solvers);
            let spent = total_hint_cost(&hint_unlocks, Some(&team.id), None);
            let points = apply_hint_deduction(score.points, spent, &self.hint_deduction_mode);
            entries.push(ScoreboardEntry {
                team_id: team.id,
                team_name: team.name.0,
                points,
                last_solve_time: score.last_solve_time,
                solves: score.solved_ids,
                rank: 0,
            });
        }

        if self.sort_by_accuracy {
            let get_accuracy = |team_id: &TeamId| -> f64 {
                let subs: Vec<&Submission> = submissions
                    .iter()
                    .filter(|s| s.team_id.as_ref() == Some(team_id))
                    .collect();
                if subs.is_empty() {
                    1.0
                } else {
                    (subs.iter().filter(|s| s.is_correct).count() as f64) / (subs.len() as f64)
                }
            };
            entries.sort_by(|a, b| {
                b.points.cmp(&a.points).then_with(|| {
                    let acc_a = get_accuracy(&a.team_id);
                    let acc_b = get_accuracy(&b.team_id);
                    acc_b
                        .partial_cmp(&acc_a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
        } else {
            entries.sort_by(|a, b| {
                b.points
                    .cmp(&a.points)
                    .then_with(|| match (a.last_solve_time, b.last_solve_time) {
                        (Some(t1), Some(t2)) => t1.cmp(&t2),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    })
            });
        }
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = (i + 1) as u32;
        }
        Ok(entries)
    }

    pub async fn export_ctftime(&self) -> Result<CtfTimeScoreboardExport, ServiceError> {
        let standings = self.get_scoreboard(None).await?;
        let submissions = self.submission_repo.find_all().await?;
        let challenges = self.challenge_repo.find_all().await?;
        let challenge_map: HashMap<String, &Challenge> =
            challenges.iter().map(|c| (c.id.clone(), c)).collect();
        let tasks: Vec<String> = challenges.iter().map(|c| c.title.0.clone()).collect();
        let mut ctftime_standings = Vec::new();
        for entry in standings {
            let mut task_stats = HashMap::new();
            let team_solves: Vec<&Submission> = submissions
                .iter()
                .filter(|s| s.team_id.as_ref() == Some(&entry.team_id) && s.is_correct)
                .collect();
            for solve in team_solves {
                if let Some(challenge) = challenge_map.get(&solve.challenge_id) {
                    task_stats.insert(
                        challenge.title.0.clone(),
                        CtfTimeTaskStats {
                            points: solve.points,
                            time: solve.submitted_at,
                        },
                    );
                }
            }
            ctftime_standings.push(CtfTimeStandingsEntry {
                pos: Some(entry.rank),
                team: entry.team_name,
                score: entry.points as f64,
                task_stats,
            });
        }
        Ok(CtfTimeScoreboardExport {
            tasks,
            standings: ctftime_standings,
        })
    }
}

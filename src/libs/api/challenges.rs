use super::*;

#[derive(serde::Serialize)]
pub struct PublicHint {
    pub cost: u32,
    pub unlocked: bool,
    pub content: Option<HtmlString>,
}

#[derive(serde::Serialize)]
pub struct PublicChallenge {
    pub id: String,
    pub title: ChallengeTitle,
    pub description: ChallengeDescription,
    pub category: ChallengeCategory,
    pub points: Option<u32>,
    pub tags: Vec<ChallengeTag>,
    pub files: Vec<ChallengeFile>,
    pub hints: Vec<PublicHint>,
    pub requirements: Vec<ChallengeRequirement>,
    pub connection_info: Option<String>,
    pub solved: bool,
    pub locked: bool,
}

#[derive(Deserialize)]
pub struct DeleteChallengeQuery {
    #[serde(default)]
    pub delete_solves: bool,
}

fn challenge_solve_counts(submissions: &[Submission]) -> HashMap<String, HashSet<String>> {
    let mut counts: HashMap<String, HashSet<String>> = HashMap::new();
    for sub in submissions {
        if sub.is_correct {
            let solver = sub
                .team_id
                .as_ref()
                .map(|t| t.0.clone())
                .unwrap_or_else(|| sub.account_id.0.clone());
            counts
                .entry(sub.challenge_id.clone())
                .or_default()
                .insert(solver);
        }
    }
    counts
}

fn current_points(challenge: &Challenge, solve_count: u32) -> u32 {
    match challenge.points.mode {
        ScoringMode::PointValue | ScoringMode::PointAttribution => {
            challenge.points.equation.parse::<u32>().unwrap_or(100)
        }
        ScoringMode::DynamicDecay {
            initial,
            minimum,
            decay,
        } => calculate_dynamic_points(initial, minimum, decay, solve_count.max(1)),
    }
}

fn challenge_solved_by(
    challenge_id: &str,
    submissions: &[Submission],
    viewer_team: Option<&TeamId>,
    viewer_account: &AccountId,
) -> bool {
    submissions.iter().any(|s| {
        s.is_correct
            && s.challenge_id == challenge_id
            && match viewer_team {
                Some(team) => s.team_id.as_ref() == Some(team),
                None => &s.account_id == viewer_account,
            }
    })
}

fn viewer_solved_ids(
    submissions: &[Submission],
    viewer_team: Option<&TeamId>,
    viewer_account: &AccountId,
) -> HashSet<String> {
    submissions
        .iter()
        .filter(|s| {
            s.is_correct
                && match viewer_team {
                    Some(team) => s.team_id.as_ref() == Some(team),
                    None => &s.account_id == viewer_account,
                }
        })
        .map(|s| s.challenge_id.clone())
        .collect()
}

fn requirements_met(challenge: &Challenge, solved: &HashSet<String>) -> bool {
    challenge.requirements.iter().all(|req| match req {
        ChallengeRequirement::Solve(id) => solved.contains(id),
    })
}

fn to_public_challenge(
    challenge: &Challenge,
    solve_count: u32,
    solved: bool,
    unlocked: &HashSet<u32>,
    locked_reveal: Option<&LockedReveal>,
) -> PublicChallenge {
    let now = chrono::Utc::now().timestamp();
    let show = |pick: fn(&LockedReveal) -> bool| locked_reveal.is_none_or(pick);
    let connection_info = if show(|r| r.connection_info) {
        match &challenge.deployment {
            ChallengeDeployment::Shared { url } => Some(url.clone()),
            _ => None,
        }
    } else {
        None
    };
    PublicChallenge {
        id: challenge.id.clone(),
        title: challenge.title.clone(),
        description: if show(|r| r.description) {
            challenge.description.clone()
        } else {
            ChallengeDescription(HtmlString(String::new()))
        },
        category: if show(|r| r.category) {
            challenge.category.clone()
        } else {
            ChallengeCategory(String::new())
        },
        points: if show(|r| r.points) {
            Some(current_points(challenge, solve_count))
        } else {
            None
        },
        tags: if show(|r| r.tags) {
            challenge.tags.clone()
        } else {
            Vec::new()
        },
        files: if show(|r| r.files) {
            challenge.files.clone()
        } else {
            Vec::new()
        },
        hints: if show(|r| r.hints) {
            challenge
                .hints
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let is_unlocked = unlocked.contains(&(i as u32));
                    PublicHint {
                        cost: h.cost.evaluate(solve_count, now),
                        unlocked: is_unlocked,
                        content: if is_unlocked {
                            Some(h.content.clone())
                        } else {
                            None
                        },
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        requirements: if show(|r| r.requirements) {
            challenge.requirements.clone()
        } else {
            Vec::new()
        },
        connection_info,
        solved,
        locked: locked_reveal.is_some(),
    }
}

pub async fn list_challenges<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let viewer_team = match state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
    {
        Ok(Some(a)) => a.team_id,
        _ => None,
    };
    let challenges = match state.solve_service.challenge_repo.find_all().await {
        Ok(c) => c,
        Err(e) => {
            return LocalizedError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: e.localize(&lang.0),
            }
            .into_response();
        }
    };
    let submissions = state
        .solve_service
        .submission_repo
        .find_all()
        .await
        .unwrap_or_default();
    let counts = challenge_solve_counts(&submissions);
    let unlocks = state
        .hint_service
        .viewer_unlocks(viewer_team.as_ref(), &user.account_id)
        .await
        .unwrap_or_default();
    let mut unlocked_map: HashMap<String, HashSet<u32>> = HashMap::new();
    for u in &unlocks {
        unlocked_map
            .entry(u.challenge_id.clone())
            .or_default()
            .insert(u.hint_index);
    }
    let empty_unlocks: HashSet<u32> = HashSet::new();
    let account = state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
        .ok()
        .flatten();
    let viewer_team = account.as_ref().and_then(|a| a.team_id.clone());
    let is_admin = account
        .as_ref()
        .is_some_and(|a| matches!(a.role, AccountRole::Admin));
    let solved_ids = viewer_solved_ids(&submissions, viewer_team.as_ref(), &user.account_id);
    let default_reveal = LockedReveal::default();
    let public: Vec<PublicChallenge> = challenges
        .iter()
        .filter(|ch| is_admin || !matches!(ch.visibility, ChallengeVisibility::Hidden))
        .map(|ch| {
            let solve_count = counts.get(&ch.id).map(|s| s.len()).unwrap_or(0) as u32;
            let solved =
                challenge_solved_by(&ch.id, &submissions, viewer_team.as_ref(), &user.account_id);
            let unlocked = unlocked_map.get(&ch.id).unwrap_or(&empty_unlocks);
            let locked_reveal = if is_admin {
                None
            } else if let ChallengeVisibility::Locked(r) = &ch.visibility {
                Some(r)
            } else if !requirements_met(ch, &solved_ids) {
                Some(&default_reveal)
            } else {
                None
            };
            to_public_challenge(ch, solve_count, solved, unlocked, locked_reveal)
        })
        .collect();
    Json(public).into_response()
}

pub async fn get_challenge<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let viewer_team = match state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
    {
        Ok(Some(a)) => a.team_id,
        _ => None,
    };
    let challenge = match state
        .solve_service
        .challenge_repo
        .find_by_id(&challenge_id)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return LocalizedError {
                status: StatusCode::NOT_FOUND,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-challenge-not-found"),
            }
            .into_response();
        }
        Err(e) => {
            return LocalizedError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: e.localize(&lang.0),
            }
            .into_response();
        }
    };
    let submissions = state
        .solve_service
        .submission_repo
        .find_all()
        .await
        .unwrap_or_default();
    let counts = challenge_solve_counts(&submissions);
    let solve_count = counts.get(&challenge_id).map(|s| s.len()).unwrap_or(0) as u32;
    let solved = challenge_solved_by(
        &challenge_id,
        &submissions,
        viewer_team.as_ref(),
        &user.account_id,
    );
    let unlocks = state
        .hint_service
        .viewer_unlocks(viewer_team.as_ref(), &user.account_id)
        .await
        .unwrap_or_default();
    let unlocked: HashSet<u32> = unlocks
        .iter()
        .filter(|u| u.challenge_id == challenge_id)
        .map(|u| u.hint_index)
        .collect();
    let account = state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
        .ok()
        .flatten();
    let is_admin = account
        .as_ref()
        .is_some_and(|a| matches!(a.role, AccountRole::Admin));
    if !is_admin && matches!(challenge.visibility, ChallengeVisibility::Hidden) {
        return LocalizedError {
            status: StatusCode::NOT_FOUND,
            message: LOCALES.lookup(&lang_id(&lang.0), "ctf-challenge-not-found"),
        }
        .into_response();
    }
    let solved_ids = viewer_solved_ids(&submissions, viewer_team.as_ref(), &user.account_id);
    let default_reveal = LockedReveal::default();
    let locked_reveal = if is_admin {
        None
    } else if let ChallengeVisibility::Locked(r) = &challenge.visibility {
        Some(r)
    } else if !requirements_met(&challenge, &solved_ids) {
        Some(&default_reveal)
    } else {
        None
    };
    Json(to_public_challenge(
        &challenge,
        solve_count,
        solved,
        &unlocked,
        locked_reveal,
    ))
    .into_response()
}

pub async fn create_challenge<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _admin: AdminUser,
    lang: PreferredLang,
    Json(challenge): Json<Challenge>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    match state
        .solve_service
        .challenge_repo
        .save(challenge.clone())
        .await
    {
        Ok(()) => (StatusCode::CREATED, Json(challenge)).into_response(),
        Err(e) => LocalizedError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.localize(&lang.0),
        }
        .into_response(),
    }
}

pub async fn update_challenge<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _admin: AdminUser,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
    Json(mut challenge): Json<Challenge>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    challenge.id = challenge_id;
    match state
        .solve_service
        .challenge_repo
        .update(challenge.clone())
        .await
    {
        Ok(()) => Json(challenge).into_response(),
        Err(e) => LocalizedError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.localize(&lang.0),
        }
        .into_response(),
    }
}

pub async fn delete_challenge<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _admin: AdminUser,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
    Query(query): Query<DeleteChallengeQuery>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    match state
        .solve_service
        .challenge_repo
        .delete(&challenge_id, query.delete_solves)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => LocalizedError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.localize(&lang.0),
        }
        .into_response(),
    }
}

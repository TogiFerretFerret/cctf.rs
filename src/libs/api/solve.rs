use super::*;
use crate::libs::types::notifications::{
    Notification, NotificationId, NotificationKind, NotificationTarget,
};

#[derive(Deserialize)]
pub struct SubmitFlagPayload {
    pub flag: String,
}

pub async fn submit_flag<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    ClientIp(ip): ClientIp,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
    Json(payload): Json<SubmitFlagPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    if !state
        .rate_limiter
        .check_limit(&format!("sub-ip:{}", ip), 10, 60)
        .await
    {
        return LocalizedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: ServiceError::RateLimitExceeded.localize(&lang.0),
        }
        .into_response();
    }
    if !state
        .rate_limiter
        .check_limit(&format!("sub-acc:{}", user.account_id.0), 10, 60)
        .await
    {
        return LocalizedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: ServiceError::RateLimitExceeded.localize(&lang.0),
        }
        .into_response();
    }

    let account = match state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
    {
        Ok(Some(a)) => a,
        _ => {
            return LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: ServiceError::Unauthorized.localize(&lang.0),
            }
            .into_response();
        }
    };
    let is_admin = matches!(account.role, AccountRole::Admin);
    if !is_admin
        && let Ok(Some(c)) = state
            .solve_service
            .challenge_repo
            .find_by_id(&challenge_id)
            .await
        && !matches!(c.visibility, ChallengeVisibility::Visible)
    {
        return LocalizedError {
            status: StatusCode::FORBIDDEN,
            message: LOCALES.lookup(&lang_id(&lang.0), "ctf-challenge-locked"),
        }
        .into_response();
    }
    let team_id = account.team_id.clone();
    let res = state
        .solve_service
        .submit_flag(&challenge_id, team_id, user.account_id, &payload.flag, &ip)
        .await
        .map_localized(&lang.0);

    match res {
        Ok(submission) => {
            maybe_broadcast_solve(&state, &submission).await;
            Json(submission).into_response()
        }
        Err(err) => err.into_response(),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

async fn maybe_broadcast_solve<A, T, C, S>(state: &AppState<A, T, C, S>, submission: &Submission)
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let cfg = &state.notification_service.config;
    if !cfg.broadcast_solves && !cfg.broadcast_first_bloods {
        return;
    }
    let subs = state
        .solve_service
        .submission_repo
        .find_all()
        .await
        .unwrap_or_default();
    let solvers = subs
        .iter()
        .filter(|s| s.is_correct && s.challenge_id == submission.challenge_id)
        .map(|s| {
            s.team_id
                .as_ref()
                .map(|t| t.0.clone())
                .unwrap_or_else(|| s.account_id.0.clone())
        })
        .collect::<HashSet<_>>();
    let first_blood = solvers.len() <= 1;
    let (kind, wanted) = if first_blood {
        (NotificationKind::FirstBlood, cfg.broadcast_first_bloods)
    } else {
        (NotificationKind::Solve, cfg.broadcast_solves)
    };
    if !wanted {
        return;
    }
    let title = state
        .solve_service
        .challenge_repo
        .find_by_id(&submission.challenge_id)
        .await
        .ok()
        .flatten()
        .map(|c| c.title.0)
        .unwrap_or_else(|| submission.challenge_id.clone());
    let solver = match &submission.team_id {
        Some(t) => state
            .auth_service
            .team_repo
            .find_by_id(t)
            .await
            .ok()
            .flatten()
            .map(|team| team.name.0)
            .unwrap_or_else(|| t.0.clone()),
        None => state
            .auth_service
            .account_repo
            .find_by_id(&submission.account_id)
            .await
            .ok()
            .flatten()
            .map(|a| a.username.0)
            .unwrap_or_else(|| submission.account_id.0.clone()),
    };
    let (heading, verb) = if first_blood {
        ("First blood!", "drew first blood on")
    } else {
        ("New solve", "solved")
    };
    let message = HtmlString(format!(
        "{} {} {}",
        escape_html(&solver),
        verb,
        escape_html(&title)
    ));
    let now = chrono::Utc::now().timestamp();
    state.notification_service.broadcast(Notification {
        id: NotificationId(uuid::Uuid::new_v4().to_string()),
        kind,
        title: heading.to_string(),
        message,
        target: NotificationTarget::Everyone,
        created_at: now,
    });
}

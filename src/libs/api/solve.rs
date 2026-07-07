use super::*;

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
        Ok(submission) => Json(submission).into_response(),
        Err(err) => err.into_response(),
    }
}

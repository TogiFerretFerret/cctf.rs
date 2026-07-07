use super::*;

#[derive(serde::Serialize)]
pub struct UnlockHintResponse {
    pub content: HtmlString,
    pub cost: u32,
    pub already_unlocked: bool,
}

pub async fn unlock_hint<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Path((challenge_id, hint_index)): Path<(String, u32)>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
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
    let team_id = account.team_id.clone();
    let is_admin = matches!(account.role, AccountRole::Admin);
    let now = chrono::Utc::now().timestamp();
    let res = state
        .hint_service
        .unlock_hint(
            &challenge_id,
            hint_index,
            team_id,
            user.account_id,
            now,
            is_admin,
        )
        .await
        .map_localized(&lang.0);
    match res {
        Ok(result) => Json(UnlockHintResponse {
            content: result.content,
            cost: result.cost,
            already_unlocked: result.already_unlocked,
        })
        .into_response(),
        Err(err) => err.into_response(),
    }
}

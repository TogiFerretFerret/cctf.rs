use super::*;

#[derive(Deserialize)]
pub struct ScoreboardQuery {
    pub bracket: Option<String>,
}

pub async fn get_scoreboard<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    axum::extract::Query(query): axum::extract::Query<ScoreboardQuery>,
    lang: PreferredLang,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let res = state
        .scoreboard_service
        .get_scoreboard(query.bracket.as_deref())
        .await
        .map_localized(&lang.0);
    match res {
        Ok(board) => Json(board).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn export_scoreboard<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let res = state
        .scoreboard_service
        .export_ctftime()
        .await
        .map_localized(&lang.0);
    match res {
        Ok(export) => Json(export).into_response(),
        Err(err) => err.into_response(),
    }
}

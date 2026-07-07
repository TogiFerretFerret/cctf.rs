use super::*;

#[derive(Deserialize)]
pub struct RegisterPayload {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}

pub async fn register<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    ClientIp(ip): ClientIp,
    lang: PreferredLang,
    Json(payload): Json<RegisterPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    if !state
        .rate_limiter
        .check_limit(&format!("auth-ip:{}", ip), 5, 60)
        .await
    {
        return LocalizedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: ServiceError::RateLimitExceeded.localize(&lang.0),
        }
        .into_response();
    }

    let res = state
        .auth_service
        .register(
            &payload.username,
            payload.email.as_deref(),
            &payload.password,
        )
        .await
        .map_localized(&lang.0);

    match res {
        Ok(account) => (StatusCode::CREATED, Json(account)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn login<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    ClientIp(ip): ClientIp,
    lang: PreferredLang,
    Json(payload): Json<LoginPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    if !state
        .rate_limiter
        .check_limit(&format!("auth-ip:{}", ip), 5, 60)
        .await
    {
        return LocalizedError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: ServiceError::RateLimitExceeded.localize(&lang.0),
        }
        .into_response();
    }

    let res = state
        .auth_service
        .login(&payload.username, &payload.password)
        .await
        .map_localized(&lang.0);
    match res {
        Ok(token) => Json(serde_json::json!({"token":token})).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn get_oauth_url<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let url = state.oauth_service.get_authorize_url();
    Json(serde_json::json!({"url":url})).into_response()
}

pub async fn oauth_callback<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
    axum::extract::Query(query): axum::extract::Query<CallbackQuery>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let res = state
        .oauth_service
        .handle_callback(&query.code)
        .await
        .map_localized(&lang.0);
    match res {
        Ok(token) => Json(serde_json::json!({"token":token})).into_response(),
        Err(err) => err.into_response(),
    }
}

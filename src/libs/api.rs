use axum::{
    extract::{State, Path, FromRequestParts},
    http::{StatusCode, request::Parts, HeaderMap},
    response::IntoResponse,
    routing::{get, post},
    Router, 
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use std::collections::HashMap;
use std::borrow::Cow;
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use crate::libs::services::{AuthService, OAuthService, SolveService, ScoreboardService, ServiceError};
use crate::libs::repos::{AccountRepo, TeamRepo, ChallengeRepo, SubmissionRepo, PgStore};
use crate::libs::types::accounts::AccountId;
use crate::libs::types::teams::TeamId;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

pub struct AppState<A, T, C, S>
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    pub auth_service: Arc<AuthService<A, T>>,
    pub oauth_service: Arc<OAuthService<A, T>>,
    pub solve_service: Arc<SolveService<C, S>>,
    pub scoreboard_service: Arc<ScoreboardService<T, C, S>>,
    pub jwt_secret: Vec<u8>,
}

impl<A, T, C, S> Clone for AppState<A, T, C, S>
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    fn clone(&self) -> Self {
        Self {
            auth_service: self.auth_service.clone(),
            oauth_service: self.oauth_service.clone(),
            solve_service: self.solve_service.clone(),
            scoreboard_service: self.scoreboard_service.clone(),
            jwt_secret: self.jwt_secret.clone(),
        }
    }
}

pub struct PreferredLang(pub String);

fn get_lang(headers: &HeaderMap) -> String {
    headers.get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("en-US").trim().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for PreferredLang
where
    S: Send + Sync,
{
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(PreferredLang(get_lang(&parts.headers)))
    }
}

pub struct LocalizedError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for LocalizedError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(serde_json::json!({"error":self.message}))).into_response()
    }
}

pub trait MapLocalized<T> {
    fn map_localized(self, lang: &str) -> Result<T, LocalizedError>;
}

impl<T> MapLocalized<T> for Result<T, ServiceError> {
    fn map_localized(self, lang: &str) -> Result<T, LocalizedError> {
        self.map_err(|e| {
            let status = match &e {
                ServiceError::Unauthorized => StatusCode::UNAUTHORIZED,
                ServiceError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
                ServiceError::OAuth(_) => StatusCode::BAD_REQUEST,
                ServiceError::Repo(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            LocalizedError {
                status,
                message: e.localize(lang),
            }
        })
    }
}

pub struct AuthenticatedUser {
    pub account_id: AccountId,
}

#[axum::async_trait]
impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for AuthenticatedUser
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    type Rejection = LocalizedError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState<A, T, C, S>) -> Result<Self, Self::Rejection> {
        let lang = get_lang(&parts.headers);
        let lang_id = lang.parse().unwrap_or_else(|_| unic_langid::langid!("en-US"));
        let auth_header = parts.headers.get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: LOCALES.lookup(&lang_id, "auth-missing-header"),
            })?;
        if !auth_header.starts_with("Bearer ") {
            return Err(LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: LOCALES.lookup(&lang_id, "auth-invalid-scheme"),
            });
        }
        let token = &auth_header["Bearer ".len()..];
        let (_,account_id_str) = crate::libs::crypto::jwt::decode(token, &state.jwt_secret)
            .map_err(|e| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: {
                    let args = HashMap::from([
                        (Cow::Borrowed("reason"), FluentValue::from(e.to_string()))
                    ]);
                    LOCALES.lookup_with_args(&lang_id, "auth-invalid-token", &args)
                }
            })?;
        Ok(AuthenticatedUser {
            account_id: AccountId(account_id_str),
        })
    }
}

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

#[derive(Deserialize)]
pub struct SubmitFlagPayload {
    pub team_id: Option<String>,
    pub flag: String,
}

pub async fn register<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
    Json(payload): Json<RegisterPayload>,
) -> axum::response::Response
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let res = state.auth_service.register(
        &payload.username,
        payload.email.as_deref(),
        &payload.password,
    ).await.map_localized(&lang.0);

    match res {
        Ok(account) => (StatusCode::CREATED, Json(account)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn login<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
    Json(payload): Json<LoginPayload>,
) -> axum::response::Response
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let res = state.auth_service.login(&payload.username, &payload.password).await.map_localized(&lang.0);
    match res {
        Ok(token) => Json(serde_json::json!({"token":token})).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn get_oauth_url<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
) -> axum::response::Response
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
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
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let res = state.oauth_service.handle_callback(&query.code).await.map_localized(&lang.0);
    match res {
        Ok(token) => Json(serde_json::json!({"token":token})).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn submit_flag<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
    Json(payload): Json<SubmitFlagPayload>,
) -> axum::response::Response
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let team_id = payload.team_id.map(TeamId);
    let res = state.solve_service.submit_flag(
        &challenge_id,
        team_id,
        user.account_id,
        &payload.flag,
    ).await
    .map_localized(&lang.0);

    match res {
        Ok(submission) => Json(submission).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn get_scoreboard<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
) -> axum::response::Response
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let res = state.scoreboard_service.get_scoreboard().await.map_localized(&lang.0);
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
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    let res = state.scoreboard_service.export_ctftime().await.map_localized(&lang.0);
    match res {
        Ok(export) => Json(export).into_response(),
        Err(err) => err.into_response(),
    }
}

pub fn create_router<A, T, C, S>(state: AppState<A, T, C, S>) -> Router
where
    A: AccountRepo + 'static,
    T: TeamRepo + 'static,
    C: ChallengeRepo + 'static,
    S: SubmissionRepo + 'static,
{
    Router::new()
        .route("/api/v1/auth/register", post(register::<A, T, C, S>))
        .route("/api/v1/auth/login", post(login::<A, T, C, S>))
        .route("/api/v1/auth/oauth/url", get(get_oauth_url::<A, T, C, S>))
        .route("/api/v1/auth/oauth/callback", get(oauth_callback::<A, T, C, S>))
        .route("/api/v1/challenges/:id/submit", post(submit_flag::<A, T, C, S>))
        .route("/api/v1/scoreboard", get(get_scoreboard::<A, T, C, S>))
        .route("/api/v1/scoreboard/export", get(export_scoreboard::<A, T, C, S>))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;
    use tokio::sync::RwLock;
    use crate::libs::repos::{AccountRepo, TeamRepo, ChallengeRepo, SubmissionRepo, RepoError};
    use crate::libs::types::accounts::{Account, AccountName};
    use crate::libs::types::teams::{Team, TeamName};
    use crate::libs::types::challenges::Challenge;
    use crate::libs::types::solves::Submission;

    #[derive(Default)]
    struct TestStore {
        accounts: RwLock<HashMap<AccountId, Account>>,
        teams: RwLock<HashMap<TeamId, Team>>,
        challenges: RwLock<HashMap<String, Challenge>>,
        submissions: RwLock<Vec<Submission>>,
    }
    impl AccountRepo for TestStore {
        async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
            Ok(self.accounts.read().await.get(id).cloned())
        }
        async fn find_by_username(&self, username: &AccountName) -> Result<Option<Account>, RepoError> {
            Ok(self.accounts.read().await.values().find(|a| &a.username == username).cloned())
        }
        async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError> {
            Ok(self.accounts.read().await.values().find(|a| a.ctftime_id == Some(ctftime_id)).cloned())
        }
        async fn save(&self, account: Account) -> Result<(), RepoError> {
            self.accounts.write().await.insert(account.id.clone(), account);
            Ok(())
        }
        async fn update(&self, account: Account) -> Result<(), RepoError> {
            self.accounts.write().await.insert(account.id.clone(), account);
            Ok(())
        }
    }
    impl TeamRepo for TestStore {
        async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError> {
            Ok(self.teams.read().await.get(id).cloned())
        }
        async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError> {
            Ok(self.teams.read().await.values().find(|t| &t.name == name).cloned())
        }
        async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError> {
            Ok(self.teams.read().await.values().find(|t| t.ctftime_id == Some(ctftime_id)).cloned())
        }
        async fn save(&self, team: Team) -> Result<(), RepoError> {
            self.teams.write().await.insert(team.id.clone(), team);
            Ok(())
        }
        async fn update(&self, team: Team) -> Result<(), RepoError> {
            self.teams.write().await.insert(team.id.clone(), team);
            Ok(())
        }
        async fn find_all(&self) -> Result<Vec<Team>, RepoError> {
            Ok(self.teams.read().await.values().cloned().collect())
        }
    }
    impl ChallengeRepo for TestStore {
        async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError> {
            Ok(self.challenges.read().await.get(id).cloned())
        }
        async fn save(&self, challenge: Challenge) -> Result<(), RepoError> {
            self.challenges.write().await.insert(challenge.id.clone(), challenge);
            Ok(())
        }
        async fn find_all(&self) -> Result<Vec<Challenge>, RepoError> {
            Ok(self.challenges.read().await.values().cloned().collect())
        }
    }
    impl SubmissionRepo for TestStore {
        async fn save(&self, submission: Submission) -> Result<(), RepoError> {
            self.submissions.write().await.push(submission);
            Ok(())
        }
        async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError> {
            Ok(self.submissions.read().await.iter().filter(|s| s.team_id.as_ref() == Some(team_id)).cloned().collect())
        }
        async fn find_all(&self) -> Result<Vec<Submission>, RepoError> {
            Ok(self.submissions.read().await.clone())
        }
    }
    #[tokio::test]
    async fn test_api_register_and_login() {
        let store = Arc::new(TestStore::default());
        let state = AppState {
            auth_service: Arc::new(AuthService {
                account_repo: store.clone(),
                team_repo: store.clone(),
                jwt_secret: b"secret".to_vec(),
            }),
            oauth_service: Arc::new(OAuthService {
                account_repo: store.clone(),
                team_repo: store.clone(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
                redirect_uri: "uri".to_string(),
                jwt_secret: b"secret".to_vec(),
            }),
            solve_service: Arc::new(SolveService {
                challenge_repo: store.clone(),
                submission_repo: store.clone(),
            }),
            scoreboard_service: Arc::new(ScoreboardService {
                team_repo: store.clone(),
                challenge_repo: store.clone(),
                submission_repo: store.clone(),
                sort_by_accuracy: false,
            }),
            jwt_secret: b"secret".to_vec(),
        };
        let app = create_router(state);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"username":"testuser","password":"testpassword","email":null}"#
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"username":"testuser","password":"testpassword"}"#
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

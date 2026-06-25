use crate::libs::repos::{AccountRepo, ChallengeRepo, InstanceRepo, SubmissionRepo, TeamRepo};
use crate::libs::services::{
    AuthService, OAuthService, ScoreboardService, ServiceError, SolveService,
};
use crate::libs::types::accounts::AccountId;
use crate::libs::types::teams::TeamId;
use axum::{
    Json, Router,
    extract::{FromRequestParts, Host, Path, Request, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

pub struct AppState<A, T, C, S>
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    pub auth_service: Arc<AuthService<A, T>>,
    pub oauth_service: Arc<OAuthService<A, T>>,
    pub solve_service: Arc<SolveService<C, S>>,
    pub scoreboard_service: Arc<ScoreboardService<T, C, S>>,
    pub jwt_secret: Vec<u8>,
    pub http_client: reqwest::Client,
}

impl<A, T, C, S> Clone for AppState<A, T, C, S>
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            auth_service: self.auth_service.clone(),
            oauth_service: self.oauth_service.clone(),
            solve_service: self.solve_service.clone(),
            scoreboard_service: self.scoreboard_service.clone(),
            jwt_secret: self.jwt_secret.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

pub struct PreferredLang(pub String);

fn get_lang(headers: &HeaderMap) -> String {
    headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("en-US").trim().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

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
                ServiceError::Kube(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for AuthenticatedUser
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    type Rejection = LocalizedError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A, T, C, S>,
    ) -> Result<Self, Self::Rejection> {
        let lang = get_lang(&parts.headers);
        let lang_id = lang
            .parse()
            .unwrap_or_else(|_| unic_langid::langid!("en-US"));
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
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
        let (_, account_id_str) = crate::libs::crypto::jwt::decode(token, &state.jwt_secret)
            .map_err(|e| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: {
                    let args = HashMap::from([(
                        Cow::Borrowed("reason"),
                        FluentValue::from(e.to_string()),
                    )]);
                    LOCALES.lookup_with_args(&lang_id, "auth-invalid-token", &args)
                },
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

fn extract_instance_id(host: &str) -> Option<String> {
    let first_part = host.split('.').next()?;
    if first_part.starts_with("inst-") {
        Some(first_part.to_string())
    } else {
        None
    }
}

pub async fn proxy_handler<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    Host(host): Host,
    req: Request,
) -> Result<Response, StatusCode>
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let instance_id = match extract_instance_id(&host) {
        Some(id) => id,
        None => return Err(StatusCode::NOT_FOUND),
    };
    let cluster_ip = state
        .solve_service
        .challenge_repo
        .get_instance_ip(&instance_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("");
    let target_url = format!("http://{}{}", cluster_ip, path_and_query);
    let method = req.method.clone();
    let headers = req.headers().clone();
    let body = req.into_body();
    let reqwest_body = reqwest::Body::wrap_stream(axum::body::BodyDataStream::new(body));
    let res = state
        .http_client
        .request(method, &target_url)
        .headers(headers)
        .body(reqwest_body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("Proxy gateway error: {:?}", e);
            StatusCode::BAD_GATEWAY
        })?;
    let mut response_builder = Response::builder().status(res.status());
    if let Some(headers_mut) = response_builder.headers_mut() {
        for (key, value) in res.headers() {
            headers_mut.insert(key, value.clone());
        }
    }
    let response_stream = res.bytes_stream();
    let body = axum::body::Body::from_stream(response_stream);
    let response = response_builder
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}

pub async fn register<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    lang: PreferredLang,
    Json(payload): Json<RegisterPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
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
    lang: PreferredLang,
    Json(payload): Json<LoginPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
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

pub async fn submit_flag<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
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
    let team_id = payload.team_id.map(TeamId);
    let res = state
        .solve_service
        .submit_flag(&challenge_id, team_id, user.account_id, &payload.flag)
        .await
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
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let res = state
        .scoreboard_service
        .get_scoreboard()
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

pub fn create_router<A, T, C, S>(state: AppState<A, T, C, S>) -> Router
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/auth/register", post(register::<A, T, C, S>))
        .route("/api/v1/auth/login", post(login::<A, T, C, S>))
        .route("/api/v1/auth/oauth/url", get(get_oauth_url::<A, T, C, S>))
        .route(
            "/api/v1/auth/oauth/callback",
            get(oauth_callback::<A, T, C, S>),
        )
        .route(
            "/api/v1/challenges/{id}/submit",
            post(submit_flag::<A, T, C, S>),
        )
        .route("/api/v1/scoreboard", get(get_scoreboard::<A, T, C, S>))
        .route(
            "/api/v1/scoreboard/export",
            get(export_scoreboard::<A, T, C, S>),
        )
        .fallback(proxy_handler::<A, T, C, S>)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::repos::{
        AccountRepo, ChallengeRepo, InstanceRepo, RepoError, SubmissionRepo, TeamRepo,
    };
    use crate::libs::types::accounts::{Account, AccountName};
    use crate::libs::types::challenges::Challenge;
    use crate::libs::types::solves::Submission;
    use crate::libs::types::teams::{Team, TeamName};
    use async_trait::async_trait;
    use axum::http::Request;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    #[derive(Default)]
    struct TestStore {
        accounts: RwLock<HashMap<AccountId, Account>>,
        teams: RwLock<HashMap<TeamId, Team>>,
        challenges: RwLock<HashMap<String, Challenge>>,
        submissions: RwLock<Vec<Submission>>,
    }

    #[async_trait]
    impl AccountRepo for TestStore {
        async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
            Ok(self.accounts.read().await.get(id).cloned())
        }
        async fn find_by_username(
            &self,
            username: &AccountName,
        ) -> Result<Option<Account>, RepoError> {
            Ok(self
                .accounts
                .read()
                .await
                .values()
                .find(|a| &a.username == username)
                .cloned())
        }
        async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError> {
            Ok(self
                .accounts
                .read()
                .await
                .values()
                .find(|a| a.ctftime_id == Some(ctftime_id))
                .cloned())
        }
        async fn save(&self, account: Account) -> Result<(), RepoError> {
            self.accounts
                .write()
                .await
                .insert(account.id.clone(), account);
            Ok(())
        }
        async fn update(&self, account: Account) -> Result<(), RepoError> {
            self.accounts
                .write()
                .await
                .insert(account.id.clone(), account);
            Ok(())
        }
    }
    #[async_trait]
    impl TeamRepo for TestStore {
        async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError> {
            Ok(self.teams.read().await.get(id).cloned())
        }
        async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError> {
            Ok(self
                .teams
                .read()
                .await
                .values()
                .find(|t| &t.name == name)
                .cloned())
        }
        async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError> {
            Ok(self
                .teams
                .read()
                .await
                .values()
                .find(|t| t.ctftime_id == Some(ctftime_id))
                .cloned())
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

    #[async_trait]
    impl InstanceRepo for TestStore {
        async fn find_active_flag(
            &self,
            _challenge_id: &str,
            _team_id: Option<&TeamId>,
            _account_id: &AccountId,
        ) -> Result<Option<String>, RepoError> {
            Ok(None)
        }
        async fn get_instance_ip(&self, _instance_id: &str) -> Result<Option<String>, RepoError> {
            Ok(None)
        }
    }

    #[async_trait]
    impl ChallengeRepo for TestStore {
        async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError> {
            Ok(self.challenges.read().await.get(id).cloned())
        }
        async fn save(&self, challenge: Challenge) -> Result<(), RepoError> {
            self.challenges
                .write()
                .await
                .insert(challenge.id.clone(), challenge);
            Ok(())
        }
        async fn find_all(&self) -> Result<Vec<Challenge>, RepoError> {
            Ok(self.challenges.read().await.values().cloned().collect())
        }
    }
    #[async_trait]
    impl SubmissionRepo for TestStore {
        async fn save(&self, submission: Submission) -> Result<(), RepoError> {
            self.submissions.write().await.push(submission);
            Ok(())
        }
        async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError> {
            Ok(self
                .submissions
                .read()
                .await
                .iter()
                .filter(|s| s.team_id.as_ref() == Some(team_id))
                .cloned()
                .collect())
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
            http_client: reqwest::Client::new(),
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
                        r#"{"username":"testuser","password":"testpassword","email":null}"#,
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
                        r#"{"username":"testuser","password":"testpassword"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

use crate::libs::repos::{AccountRepo, ChallengeRepo, SubmissionRepo, TeamRepo};
use crate::libs::services::solve::calculate_dynamic_points;
use crate::libs::services::{
    AuthService, OAuthService, ScoreboardService, ServiceError, SolveService,
};
use crate::libs::types::accounts::{AccountId, AccountRole};
use crate::libs::types::challenges::{
    Challenge, ChallengeDeployment, ChallengeFile, ChallengeRequirement, ChallengeTag, ScoringMode,
};
use crate::libs::types::solves::Submission;
use crate::libs::types::teams::TeamId;
use axum::{
    Json, Router,
    extract::{ConnectInfo, FromRequestParts, Path, Query, Request, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
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
    pub solve_service: Arc<SolveService<C, S, T>>,
    pub scoreboard_service: Arc<ScoreboardService<T, C, S>>,
    pub jwt_secret: Vec<u8>,
    pub http_client: reqwest::Client,
    pub rate_limiter: Arc<RateLimiter>,
    pub bracket_acl_scripts: Arc<tokio::sync::RwLock<HashMap<String, String>>>,
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
            rate_limiter: self.rate_limiter.clone(),
            bracket_acl_scripts: self.bracket_acl_scripts.clone(),
        }
    }
}

pub struct PreferredLang(pub String);

/// Parse a language tag, falling back to en-US. NEVER `.unwrap()` this on the
/// raw Accept-Language header — a client sending garbage would panic the handler.
fn lang_id(lang: &str) -> unic_langid::LanguageIdentifier {
    lang.parse()
        .unwrap_or_else(|_| unic_langid::langid!("en-US"))
}

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
                ServiceError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
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
        let (_, claims) = crate::libs::crypto::jwt::decode::<crate::libs::crypto::jwt::Claims>(
            token,
            &state.jwt_secret,
        )
        .map_err(|e| LocalizedError {
            status: StatusCode::UNAUTHORIZED,
            message: {
                let args =
                    HashMap::from([(Cow::Borrowed("reason"), FluentValue::from(e.to_string()))]);
                LOCALES.lookup_with_args(&lang_id, "auth-invalid-token", &args)
            },
        })?;
        Ok(AuthenticatedUser {
            account_id: AccountId(claims.sub),
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
    pub flag: String,
}

#[derive(Deserialize)]
pub struct CreateInvitePayload {
    pub team_id: String,
    pub lifespan_hours: Option<i64>,
}

#[derive(Deserialize)]
pub struct JoinTeamPayload {
    pub token: String,
}

fn extract_instance_id(host: &str) -> Option<String> {
    let first_part = host.split('.').next()?;
    if first_part.starts_with("inst-") {
        Some(first_part.to_string())
    } else {
        None
    }
}

fn generate_invite_token(team_id: &str, expires_at: i64, secret: &[u8]) -> String {
    let message = format!("{}:{}", team_id, expires_at);
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let signature = BASE64_URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{}:{}:{}", team_id, expires_at, signature)
}

fn verify_invite_token(token: &str, secret: &[u8]) -> Option<(String, i64)> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let team_id = parts[0];
    let expires_at_str = parts[1];
    let signature_base64 = parts[2];

    let expires_at = expires_at_str.parse::<i64>().ok()?;
    let now = chrono::Utc::now().timestamp();
    if now > expires_at {
        return None;
    }

    let message = format!("{}:{}", team_id, expires_at);
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(message.as_bytes());

    let provided_sig = BASE64_URL_SAFE_NO_PAD.decode(signature_base64).ok()?;
    if mac.verify_slice(&provided_sig).is_ok() {
        Some((team_id.to_string(), expires_at))
    } else {
        None
    }
}

pub fn load_bracket_scripts() -> HashMap<String, String> {
    if let Ok(content) = std::fs::read_to_string("brackets.json") {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        let mut map = HashMap::new();
        map.insert(
            "Collegiate".to_string(),
            "email.ends_with(\".edu\")".to_string(),
        );
        map
    }
}

pub fn validate_bracket_join_rhai(email: &str, username: &str, script_content: &str) -> bool {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_string_size(16 * 1024);
    let mut scope = rhai::Scope::new();
    scope.push("email", email.to_string());
    scope.push("username", username.to_string());
    match engine.eval_with_scope::<bool>(&mut scope, script_content) {
        Ok(is_allowed) => is_allowed,
        Err(e) => {
            eprintln!("rhaiBracket!: {:?}", e);
            false
        }
    }
}

pub async fn proxy_handler<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    req: Request,
) -> Result<Response, StatusCode>
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let host = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
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
    let method = req.method().clone();
    let mut headers = req.headers().clone();
    for h in [
        "authorization",
        "cookie",
        "connection",
        "keep-alive",
        "proxy-authorization",
        "proxy-authenticate",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(h);
    }
    let body = req.into_body();
    let bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let reqwest_body = reqwest::Body::from(bytes);
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

pub async fn create_invite<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Json(payload): Json<CreateInvitePayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let team = match state
        .auth_service
        .team_repo
        .find_by_id(&TeamId(payload.team_id.clone()))
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            return LocalizedError {
                status: StatusCode::NOT_FOUND,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-team-not-found"),
            }
            .into_response();
        }
    };
    if team.captain_id != user.account_id {
        return LocalizedError {
            status: StatusCode::FORBIDDEN,
            message: LOCALES.lookup(&lang_id(&lang.0), "ctf-not-captain"),
        }
        .into_response();
    }
    let lifespan = payload.lifespan_hours.unwrap_or(24).clamp(1, 168);
    let expires_at = chrono::Utc::now().timestamp() + (lifespan * 3600);
    let token = generate_invite_token(&team.id.0, expires_at, &state.jwt_secret);
    Json(serde_json::json!({"token":token, "expires_at":expires_at})).into_response()
}

pub async fn join_team<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Json(payload): Json<JoinTeamPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let (team_id_str, _) = match verify_invite_token(&payload.token, &state.jwt_secret) {
        Some(val) => val,
        None => {
            return LocalizedError {
                status: StatusCode::BAD_REQUEST,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-invalid-invite-token"),
            }
            .into_response();
        }
    };
    let mut team = match state
        .auth_service
        .team_repo
        .find_by_id(&TeamId(team_id_str))
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            return LocalizedError {
                status: StatusCode::NOT_FOUND,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-team-not-found"),
            }
            .into_response();
        }
    };
    let account = match state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
    {
        Ok(Some(a)) => a,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let scripts = state.bracket_acl_scripts.read().await;
    if let Some(script) = scripts.get(&team.bracket) {
        let email_str = account.email.as_ref().map(|e| e.0.as_str()).unwrap_or("");
        let username_str = account.username.0.as_str();
        let is_allowed = validate_bracket_join_rhai(email_str, username_str, script);
        if !is_allowed {
            return LocalizedError {
                status: StatusCode::FORBIDDEN,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-bracket-domain-restricted"),
            }
            .into_response();
        }
    }
    let mut updated_account = account;
    updated_account.team_id = Some(team.id.clone());
    if let Err(_) = state
        .auth_service
        .account_repo
        .update(updated_account)
        .await
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if !team.member_ids.contains(&user.account_id) {
        team.member_ids.push(user.account_id.clone());
        let _ = state.auth_service.team_repo.update(team).await;
    }
    StatusCode::OK.into_response()
}

#[derive(serde::Serialize)]
pub struct PublicHint {
    pub cost: u32,
}

#[derive(serde::Serialize)]
pub struct PublicChallenge {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub points: u32,
    pub tags: Vec<ChallengeTag>,
    pub files: Vec<ChallengeFile>,
    pub hints: Vec<PublicHint>,
    pub requirements: Vec<ChallengeRequirement>,
    pub connection_info: Option<String>,
    pub solved: bool,
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

fn to_public_challenge(challenge: &Challenge, solve_count: u32, solved: bool) -> PublicChallenge {
    let connection_info = match &challenge.deployment {
        ChallengeDeployment::Shared { url } => Some(url.clone()),
        _ => None,
    };
    PublicChallenge {
        id: challenge.id.clone(),
        title: challenge.title.0.clone(),
        description: challenge.description.0.0.clone(),
        category: challenge.category.0.clone(),
        points: current_points(challenge, solve_count),
        tags: challenge.tags.clone(),
        files: challenge.files.clone(),
        hints: challenge
            .hints
            .iter()
            .map(|h| PublicHint { cost: h.cost })
            .collect(),
        requirements: challenge.requirements.clone(),
        connection_info,
        solved,
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
    let public: Vec<PublicChallenge> = challenges
        .iter()
        .map(|ch| {
            let solve_count = counts.get(&ch.id).map(|s| s.len()).unwrap_or(0) as u32;
            let solved =
                challenge_solved_by(&ch.id, &submissions, viewer_team.as_ref(), &user.account_id);
            to_public_challenge(ch, solve_count, solved)
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
    Json(to_public_challenge(&challenge, solve_count, solved)).into_response()
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

pub const API_ROUTES: &[(&str, &str)] = &[
    ("POST", "/api/v1/auth/register"),
    ("POST", "/api/v1/auth/login"),
    ("GET", "/api/v1/auth/oauth/url"),
    ("GET", "/api/v1/auth/oauth/callback"),
    ("GET", "/api/v1/challenges"),
    ("POST", "/api/v1/challenges"),
    ("GET", "/api/v1/challenges/{id}"),
    ("PATCH", "/api/v1/challenges/{id}"),
    ("DELETE", "/api/v1/challenges/{id}"),
    ("POST", "/api/v1/challenges/{id}/submit"),
    ("GET", "/api/v1/scoreboard"),
    ("GET", "/api/v1/scoreboard/export"),
    ("POST", "/api/v1/teams/invite"),
    ("POST", "/api/v1/teams/join"),
];

const OPENAPI_YAML: &str = include_str!("../../openapi.yaml");

#[derive(Deserialize)]
pub struct SpecLangQuery {
    pub lang: Option<String>,
}

async fn openapi_yaml(lang: PreferredLang, Query(q): Query<SpecLangQuery>) -> impl IntoResponse {
    let lid = lang_id(&q.lang.unwrap_or(lang.0));
    let mut doc: serde_json::Value =
        serde_norway::from_str(OPENAPI_YAML).expect("openapi.yaml must be valid YAML");
    localize_spec(&mut doc, &lid);
    let body = serde_norway::to_string(&doc).expect("serialize localized yaml");
    (
        [(axum::http::header::CONTENT_TYPE, "application/yaml")],
        body,
    )
}

async fn openapi_json(lang: PreferredLang, Query(q): Query<SpecLangQuery>) -> impl IntoResponse {
    let lid = lang_id(&q.lang.unwrap_or(lang.0));
    let mut doc: serde_json::Value =
        serde_norway::from_str(OPENAPI_YAML).expect("openapi.yaml must be valid YAML");
    localize_spec(&mut doc, &lid);
    let body = serde_json::to_string(&doc).expect("serialize localized json");
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
}

async fn api_docs(
    lang: PreferredLang,
    Query(q): Query<SpecLangQuery>,
) -> axum::response::Html<String> {
    let tag = lang_id(&q.lang.unwrap_or(lang.0)).to_string();
    axum::response::Html(format!(
        r#"<!doctype html><html><head><title>cctf.rs API</title><meta charset="utf-8" /></head><body><script id="api-reference" data-url="/openapi.yaml?lang={tag}"></script><script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference">/script></body></html>"#
    ))
}

fn localize_spec(value: &mut serde_json::Value, lang: &unic_langid::LanguageIdentifier) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if matches!(k.as_str(), "summary" | "description" | "title") {
                    if let serde_json::Value::String(s) = v {
                        if !s.contains(char::is_whitespace) {
                            let t = LOCALES.lookup(lang, s);
                            if !t.is_empty() && &t != s {
                                *s = t;
                            }
                        }
                        continue;
                    }
                } else {
                    localize_spec(v, lang);
                }
            }
        }
        serde_json::Value::Array(arr) => arr.iter_mut().for_each(|v| localize_spec(v, lang)),
        _ => {}
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
            "/api/v1/challenges",
            get(list_challenges::<A, T, C, S>).post(create_challenge::<A, T, C, S>),
        )
        .route(
            "/api/v1/challenges/{id}",
            get(get_challenge::<A, T, C, S>)
                .patch(update_challenge::<A, T, C, S>)
                .delete(delete_challenge::<A, T, C, S>),
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
        .route("/api/v1/teams/invite", post(create_invite::<A, T, C, S>))
        .route("/api/v1/teams/join", post(join_team::<A, T, C, S>))
        .route("/openapi.yaml", get(openapi_yaml))
        .route("/openapi.json", get(openapi_json))
        .route("/docs", get(api_docs))
        .fallback(proxy_handler::<A, T, C, S>)
        .with_state(state)
}

pub struct ClientIp(pub String);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let trust_proxy = std::env::var("TRUST_PROXY_HEADERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if trust_proxy {
            if let Some(ip) = parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').last())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                return Ok(ClientIp(ip));
            }
            if let Some(ip) = parts
                .headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                return Ok(ClientIp(ip));
            }
        }

        if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(ClientIp(addr.ip().to_string()));
        }

        Ok(ClientIp("127.0.0.1".to_string()))
    }
}

pub struct RateLimiter {
    requests: tokio::sync::Mutex<std::collections::HashMap<String, Vec<i64>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            requests: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub async fn check_limit(&self, key: &str, limit: usize, window_secs: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut map = self.requests.lock().await;
        let entry = map.entry(key.to_string()).or_insert_with(Vec::new);

        entry.retain(|&ts| now - ts < window_secs);

        if entry.len() >= limit {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

pub struct AdminUser {
    pub account_id: AccountId,
}

impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for AdminUser
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
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        let lang = get_lang(&parts.headers);
        let account = state
            .auth_service
            .account_repo
            .find_by_id(&user.account_id)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: ServiceError::Unauthorized.localize(&lang),
            })?;
        if !matches!(account.role, AccountRole::Admin) {
            return Err(LocalizedError {
                status: StatusCode::FORBIDDEN,
                message: LOCALES.lookup(&lang_id(&lang), "auth-admin-required"),
            });
        }
        Ok(AdminUser {
            account_id: user.account_id,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::repos::{
        AccountRepo, ChallengeRepo, InstanceRepo, RepoError, SubmissionRepo, TeamRepo,
    };
    use crate::libs::types::accounts::{Account, AccountEmail, AccountName, AccountRole};
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
        async fn update(&self, challenge: Challenge) -> Result<(), RepoError> {
            self.challenges
                .write()
                .await
                .insert(challenge.id.clone(), challenge);
            Ok(())
        }
        async fn delete(&self, id: &str, delete_solves: bool) -> Result<(), RepoError> {
            self.challenges.write().await.remove(id);
            if delete_solves {
                self.submissions
                    .write()
                    .await
                    .retain(|s| s.challenge_id.as_str() != id);
            }
            Ok(())
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
        let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
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
                team_repo: store.clone(),
            }),
            scoreboard_service: Arc::new(ScoreboardService {
                team_repo: store.clone(),
                challenge_repo: store.clone(),
                submission_repo: store.clone(),
                sort_by_accuracy: false,
                freeze_time: None,
            }),
            jwt_secret: b"secret".to_vec(),
            http_client: reqwest::Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
            bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(load_bracket_scripts())),
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

    #[test]
    fn test_invite_token_signature_and_expiration() {
        let secret = b"unit-test-secret-key-123456";
        let team_id = "test-team-id";
        let expires_at = chrono::Utc::now().timestamp() + 3600;
        let token = generate_invite_token(team_id, expires_at, secret);
        let result = verify_invite_token(&token, secret);
        assert!(result.is_some());
        let (verified_team_id, verified_expires_at) = result.unwrap();
        assert_eq!(verified_team_id, team_id);
        assert_eq!(verified_expires_at, expires_at);
        let expired_at = chrono::Utc::now().timestamp() - 10;
        let expired_token = generate_invite_token(team_id, expired_at, secret);
        assert!(verify_invite_token(&expired_token, secret).is_none());
        let parts: Vec<&str> = token.split(':').collect();
        let tampered_token = format!("{}:{}:{}", parts[0], parts[1], "invalid_signature");
        assert!(verify_invite_token(&tampered_token, secret).is_none());
    }

    #[tokio::test]
    async fn test_collegiate_bracket_acl() {
        let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
        let store = Arc::new(TestStore::default());

        let team = Team {
            id: TeamId("team-col".to_string()),
            name: TeamName("Collegiate Team".to_string()),
            ctftime_id: None,
            invite_code: None,
            captain_id: AccountId("captain".to_string()),
            member_ids: vec![AccountId("captain".to_string())],
            bracket: "Collegiate".to_string(),
            fields: Default::default(),
            create_at: chrono::Utc::now().timestamp(),
        };
        TeamRepo::save(store.as_ref(), team).await.unwrap();

        let player1 = Account {
            id: AccountId("player1".to_string()),
            username: AccountName("player1".to_string()),
            email: Some(AccountEmail("student@test.edu".to_string())),
            password_hash: None,
            role: AccountRole::Player,
            team_id: None,
            ctftime_id: None,
            fields: Default::default(),
            created_at: chrono::Utc::now().timestamp(),
        };
        AccountRepo::save(store.as_ref(), player1).await.unwrap();

        let player2 = Account {
            id: AccountId("player2".to_string()),
            username: AccountName("player2".to_string()),
            email: Some(AccountEmail("user@gmail.com".to_string())),
            password_hash: None,
            role: AccountRole::Player,
            team_id: None,
            ctftime_id: None,
            fields: Default::default(),
            created_at: chrono::Utc::now().timestamp(),
        };
        AccountRepo::save(store.as_ref(), player2).await.unwrap();

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
                team_repo: store.clone(),
            }),
            scoreboard_service: Arc::new(ScoreboardService {
                team_repo: store.clone(),
                challenge_repo: store.clone(),
                submission_repo: store.clone(),
                sort_by_accuracy: false,
                freeze_time: None,
            }),
            jwt_secret: b"secret".to_vec(),
            http_client: reqwest::Client::new(),
            rate_limiter: Arc::new(RateLimiter::new()),
            bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(load_bracket_scripts())),
        };

        let app = create_router(state);

        let token =
            generate_invite_token("team-col", chrono::Utc::now().timestamp() + 3600, b"secret");

        let p1_auth_token = crate::libs::crypto::jwt::issue("player1", 3600, b"secret").unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/teams/join")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", p1_auth_token))
                    .body(axum::body::Body::from(format!(
                        r#"{{"token":"{}"}}"#,
                        token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let p2_auth_token = crate::libs::crypto::jwt::issue("player2", 3600, b"secret").unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/teams/join")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", p2_auth_token))
                    .body(axum::body::Body::from(format!(
                        r#"{{"token":"{}"}}"#,
                        token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

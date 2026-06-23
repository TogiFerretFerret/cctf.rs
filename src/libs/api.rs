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
use crate::libs::repos::PgStore;
use crate::libs::types::accounts::{AccountId, Account};
use crate::libs::types::teams::TeamId;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Clone)]
pub struct AppState {
    pub auth_service: Arc<AuthService<PgStore, PgStore>>,
    pub oauth_service: Arc<OAuthService<PgStore, PgStore>>,
    pub solve_service: Arc<SolveService<PgStore, PgStore>>,
    pub scoreboard_service: Arc<ScoreboardService<PgStore, PgStore, PgStore>>,
    pub jwt_secret: Vec<u8>,
}

pub struct PreferredLang(pub String);

fn get_lang(headers: &HeaderMap) -> String {
    headers.get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("en-US").trim().to_string())
        .unwrap_or_else(|| "en-US".to_string())
}

impl<S> FromRequestParts<S> for PreferredLang
where 
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;
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

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = LocalizedError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
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
        let account_id_str = crate::libs::crypto::jwt::decode(token, &state.jwt_secret)
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

pub async fn register(
    State(state): State<AppState>,
    lang: PreferredLang,
    Json(payload): Json<RegisterPayload>,
) -> Result<impl IntoResponse, LocalizedError> {
    let account = state.auth_service.register(
        &payload.username,
        payload.email.as_deref(),
        &payload.password,
    ).await.map_localized(&lang.0)?;
    Ok((StatusCode::CREATED, Json(account)))
}

#[derive(Deserialize)]
pub struct Loginayload {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    lang: PreferredLang,
    Json(payload): Json<LoginPayload>,
) -> Result<impl IntoResponse, LocalizedError> {
    let token = state.auth_service.login(&payload.username, &payload.password).await.map_localized(&lang.0)?;
    Ok(Json(serde_json::json!({"token":token})))
}

pub async fn get_oauth_url(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let url = state.oauth_service.get_authorize_url();
    Json(serde_json::json!({"url":url}))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
}

pub async fn oauth_callback(
    State(state): State<AppState>,
    lang: PreferredLang,
    axum::extract::Query(query): axum::extract::Query<CallbackQuery>,
) -> Result<impl IntoResponse, LocalizedError> {
    let token = state.oauth_service.handle_callback(&query.code).await.map_localized(&lang.0)?;
    Ok(Json(serde_json::json!({"token":token})))
}

#[derive(Deserialize)]
pub struct SubmitFlagPayload {
    pub team_id: Option<String>,
    pub flag: String,
}

pub async fn submit_flag(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Path(challenge_id): Path<String>,
    Json(payload): Json<SubmitFlagPayload>,
) -> Result<impl IntoResponse, LocalizedError> {
    let team_id = payload.team_id.map(TeamId);
    let submission = state.solve_service.submit_flag(
        &challenge_id,
        team_id,
        user.account_id,
        &payload.flag,
    ).await
    .map_localized(&lang.0)?;
    Ok(Json(submission))
}

pub async fn get_scoreboard(
    State(state): State<AppState>,
    lang: PreferredLang,
) -> Result<impl IntoResponse, LocalizedError> {
    let board = state.scoreboard_service.get_scoreboard().await.map_localized(&lang.0)?;
    Ok(Json(board))
}

pub async fn export_scoreboard(
    State(state): State<AppState>,
    lang: PreferredLang,
) -> Result<impl IntoResponse, LocalizedError> {
    let export = state.scoreboard_service.export_ctftime().await.map_localized(&lang.0)?;
    Ok(Json(export))
}


pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/oauth/url", get(get_oauth_url))
        .route("/api/v1/auth/oauth/callback", get(oauth_callback))
        .route("/api/v1/challenges/:id/submit", post(submit_flag))
        .route("/api/v1/scoreboard", get(get_scoreboard))
        .route("/api/v1/scoreboard/export", get(export_scoreboard))
        .with_state(state)
}


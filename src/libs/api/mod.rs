pub(crate) use crate::libs::repos::{AccountRepo, ChallengeRepo, SubmissionRepo, TeamRepo};
pub(crate) use crate::libs::services::{
    AuthService, FileService, HintService, OAuthService, ScoreboardService, ServiceError,
    SolveService, solve::calculate_dynamic_points,
};
pub(crate) use crate::libs::types::{
    accounts::{AccountId, AccountRole},
    challenges::{
        Challenge, ChallengeCategory, ChallengeDeployment, ChallengeDescription, ChallengeFile,
        ChallengeRequirement, ChallengeTag, ChallengeTitle, ChallengeVisibility, LockedReveal,
        ScoringMode,
    },
    htmlstring::HtmlString,
    solves::Submission,
    teams::TeamId,
};
pub(crate) use axum::{
    Json, Router,
    extract::{
        ConnectInfo, DefaultBodyLimit, FromRequestParts, Multipart, Path, Query, Request, State,
    },
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
pub(crate) use fluent_templates::{Loader, fluent_bundle::FluentValue};
pub(crate) use serde::Deserialize;
pub(crate) use std::borrow::Cow;
pub(crate) use std::collections::{HashMap, HashSet};
pub(crate) use std::net::SocketAddr;
pub(crate) use std::sync::Arc;

use fluent_templates::static_loader;

mod auth;
mod brackets;
mod challenges;
mod extract;
mod files;
mod hints;
mod openapi;
mod proxy;
mod scoreboard;
mod solve;
mod state;
mod teams;
#[cfg(test)]
mod tests;

pub use brackets::load_bracket_scripts;
pub use extract::*;
pub use state::AppState;

use self::auth::*;
use self::challenges::*;
use self::files::*;
use self::hints::*;
use self::openapi::*;
use self::proxy::*;
use self::scoreboard::*;
use self::solve::*;
use self::teams::*;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

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
    ("POST", "/api/v1/challenges/{id}/hints/{index}/unlock"),
    ("POST", "/api/v1/files"),
    ("GET", "/api/v1/files/{id}"),
    ("GET", "/api/v1/scoreboard"),
    ("GET", "/api/v1/scoreboard/export"),
    ("POST", "/api/v1/teams/invite"),
    ("POST", "/api/v1/teams/join"),
];

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
        .route(
            "/api/v1/challenges/{id}/hints/{index}/unlock",
            post(unlock_hint::<A, T, C, S>),
        )
        .route(
            "/api/v1/files",
            post(upload_file::<A, T, C, S>).layer(DefaultBodyLimit::max(128 * 1024 * 1024)),
        )
        .route("/api/v1/files/{id}", get(download_file::<A, T, C, S>))
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

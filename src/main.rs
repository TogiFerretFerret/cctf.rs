use std::net::SocketAddr;
use std::sync::Arc;

use cctf_rs::libs::{
    api::{self, AppState, RateLimiter},
    repos::pg::PgStore,
    services::{
        AuthService, ConfigService, OAuthService, ScoreboardService, SolveService,
        email::{HttpCatcher, HttpCatcherConfig},
    },
};

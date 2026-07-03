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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tokio_rustls::rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring provider");
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET is required")
        .into_bytes();
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let pool = sqlx::PgPool::connect(&database_url).await?;
    let store = Arc::new(PgStore::new(pool));
    store.init_db().await?;
    let config_service = ConfigService {
        config_repo: store.clone(),
    };
    let cfg = config_service.get().await?;
    let auth_service = Arc::new(AuthService {
        account_repo: store.clone(),
        team_repo: store.clone(),
        jwt_secret: jwt_secret.clone(),
    });
    let oauth_service = Arc::new(OAuthService {
        account_repo: store.clone(),
        team_repo: store.clone(),
        client_id: std::env::var("CTFTIME_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("CTFTIME_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("CTFTIME_REDIRECT_URI").unwrap_or_default(),
        jwt_secret: jwt_secret.clone(),
    });
    let solve_service = Arc::new(SolveService {
        challenge_repo: store.clone(),
        submission_repo: store.clone(),
        team_repo: store.clone(),
    });
    let scoreboard_service = Arc::new(ScoreboardService {
        team_repo: store.clone(),
        challenge_repo: store.clone(),
        submission_repo: store.clone(),
        sort_by_accuracy: cfg.sort_by_accuracy,
        freeze_time: cfg.freeze_time,
        hint_unlock_repo: store.clone(),
        deduct_hint_costs: true, // TODO: make this configurable!!!
    });
    let state = AppState {
        auth_service,
        oauth_service,
        solve_service,
        scoreboard_service,
        jwt_secret,
        http_client: reqwest::Client::new(),
        rate_limiter: Arc::new(RateLimiter::new()),
        bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(api::load_bracket_scripts())),
    };
    let catcher = HttpCatcher::new(HttpCatcherConfig {
        secret: std::env::var("INBOUND_EMAIL_SECRET").ok(),
        ..Default::default()
    });
    let app = api::create_router(state).merge(catcher.router());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    println!(
        "cctf.rs '{}' listening on http://{}",
        cfg.ctf_name, bind_addr
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

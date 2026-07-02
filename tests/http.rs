use std::sync::{Arc, LazyLock};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use cctf_rs::libs::api::{AppState, RateLimiter, create_router};
use cctf_rs::libs::crypto::jwt;
use cctf_rs::libs::repos::{AccountRepo, SubmissionRepo, pg::PgStore};
use cctf_rs::libs::services::{AuthService, OAuthService, ScoreboardService, SolveService};
use cctf_rs::libs::types::accounts::{Account, AccountId, AccountName, AccountRole};
use cctf_rs::libs::types::challenges::{
    Challenge, ChallengeAuthor, ChallengeCategory, ChallengeDeployment, ChallengeDescription,
    ChallengePoints, ChallengeTitle, ScoringMode,
};
use cctf_rs::libs::types::flags::FlagValidator;
use cctf_rs::libs::types::htmlstring::HtmlString;
use tokio::sync::Mutex;
use tower::ServiceExt;

static DB_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const SECRET: &[u8] = b"http-integration-secret";
const FLAG: &str = "cctf{secret_flag}";

async fn setup() -> Arc<PgStore> {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must be set to run the http integration tests");
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    let store = Arc::new(PgStore::new(pool.clone()));
    store.init_db().await.expect("init_db");
    sqlx::query("TRUNCATE accounts, teams, challenges, submissions, challenge_instances, ctf_config CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    store
}

fn build_app(store: Arc<PgStore>) -> Router {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let state = AppState {
        auth_service: Arc::new(AuthService {
            account_repo: store.clone(),
            team_repo: store.clone(),
            jwt_secret: SECRET.to_vec(),
        }),
        oauth_service: Arc::new(OAuthService {
            account_repo: store.clone(),
            team_repo: store.clone(),
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: String::new(),
            jwt_secret: SECRET.to_vec(),
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
        jwt_secret: SECRET.to_vec(),
        http_client: reqwest::Client::new(),
        rate_limiter: Arc::new(RateLimiter::new()),
        bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    };
    create_router(state)
}

async fn make_account(store: &Arc<PgStore>, id: &str, role: AccountRole) -> String {
    let acct = Account {
        id: AccountId(id.to_string()),
        username: AccountName(id.to_string()),
        email: None,
        password_hash: None,
        role,
        team_id: None,
        ctftime_id: None,
        fields: Default::default(),
        created_at: 0,
    };
    AccountRepo::save(store.as_ref(), acct).await.unwrap();
    jwt::issue(id, 3600, SECRET).unwrap()
}

fn sample_challenge() -> Challenge {
    Challenge {
        id: "web1".to_string(),
        title: ChallengeTitle("Web 1".to_string()),
        description: ChallengeDescription(HtmlString("find the flag".to_string())),
        category: ChallengeCategory("web".to_string()),
        points: ChallengePoints {
            mode: ScoringMode::PointValue,
            equation: "500".to_string(),
        },
        flag: FlagValidator::Static(FLAG.to_string()),
        author: ChallengeAuthor {
            id: "admin".to_string(),
            username: "admin".to_string(),
        },
        hints: Vec::new(),
        files: Vec::new(),
        tags: Vec::new(),
        requirements: Vec::new(),
        team_consensus: false,
        deployment: ChallengeDeployment::None,
    }
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Vec<u8>>,
) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(tok) = token {
        req = req.header("authorization", format!("Bearer {}", tok));
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
#[ignore = "requires Postgres; run: TEST_DATABASE_URL=... cargo test --test http -- --ignored"]
async fn http_challenge_lifecycle() {
    let _guard = DB_LOCK.lock().await;
    let store = setup().await;
    let app = build_app(store.clone());
    let admin = make_account(&store, "admin-1", AccountRole::Admin).await;
    let player = make_account(&store, "player-1", AccountRole::Player).await;

    let body = serde_json::to_vec(&sample_challenge()).unwrap();
    let (status, _) = send(&app, "POST", "/api/v1/challenges", Some(&admin), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, listing) = send(&app, "GET", "/api/v1/challenges", Some(&player), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!listing.contains(FLAG), "flag leaked in listing: {listing}");
    assert!(listing.contains("\"solved\":false"));
    assert!(listing.contains("\"points\":500"));

    let submit = serde_json::to_vec(&serde_json::json!({ "flag": FLAG })).unwrap();
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/challenges/web1/submit",
        Some(&player),
        Some(submit),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, view) = send(&app, "GET", "/api/v1/challenges/web1", Some(&player), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(view.contains("\"solved\":true"));
    assert!(!view.contains(FLAG), "flag leaked in detail view: {view}");

    let (status, _) = send(
        &app,
        "DELETE",
        "/api/v1/challenges/web1?delete_solves=true",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(&app, "GET", "/api/v1/challenges/web1", Some(&player), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires Postgres; run: TEST_DATABASE_URL=... cargo test --test http -- --ignored"]
async fn http_delete_keeps_solves_when_requested() {
    let _guard = DB_LOCK.lock().await;
    let store = setup().await;
    let app = build_app(store.clone());
    let admin = make_account(&store, "admin-1", AccountRole::Admin).await;
    let player = make_account(&store, "player-1", AccountRole::Player).await;

    let body = serde_json::to_vec(&sample_challenge()).unwrap();
    send(&app, "POST", "/api/v1/challenges", Some(&admin), Some(body)).await;

    let submit = serde_json::to_vec(&serde_json::json!({ "flag": FLAG })).unwrap();
    let (status, _) = send(
        &app,
        "POST",
        "/api/v1/challenges/web1/submit",
        Some(&player),
        Some(submit),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send(
        &app,
        "DELETE",
        "/api/v1/challenges/web1?delete_solves=false",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let subs = SubmissionRepo::find_all(store.as_ref()).await.unwrap();
    assert_eq!(subs.len(), 1, "solve should survive delete_solves=false");
    assert_eq!(subs[0].challenge_id, "web1");
}

#[tokio::test]
#[ignore = "requires Postgres; run: TEST_DATABASE_URL=... cargo test --test http -- --ignored"]
async fn http_non_admin_cannot_create() {
    let _guard = DB_LOCK.lock().await;
    let store = setup().await;
    let app = build_app(store.clone());
    let player = make_account(&store, "player-1", AccountRole::Player).await;

    let body = serde_json::to_vec(&sample_challenge()).unwrap();
    let (status, _) = send(&app, "POST", "/api/v1/challenges", Some(&player), Some(body)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

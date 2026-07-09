use super::teams::{generate_invite_token, verify_invite_token};
use super::*;
use crate::libs::repos::{
    AccountRepo, ChallengeRepo, FileRepo, HintUnlockRepo, InstanceRepo, NotificationRepo,
    RepoError, SubmissionRepo, TeamRepo,
};
use crate::libs::types::{
    accounts::{Account, AccountEmail, AccountName, AccountRole},
    challenges::Challenge,
    config::HintDeductionMode,
    files::StoredFile,
    notifications::Notification,
    solves::{HintUnlock, Submission},
    teams::{Team, TeamName},
};
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
    hint_unlocks: RwLock<Vec<HintUnlock>>,
    files: RwLock<HashMap<String, StoredFile>>,
}

#[async_trait]
impl AccountRepo for TestStore {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
        Ok(self.accounts.read().await.get(id).cloned())
    }
    async fn find_by_username(&self, username: &AccountName) -> Result<Option<Account>, RepoError> {
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
#[async_trait]
impl HintUnlockRepo for TestStore {
    async fn find_all(&self) -> Result<Vec<HintUnlock>, RepoError> {
        Ok(self.hint_unlocks.read().await.clone())
    }
    async fn find_for(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Vec<HintUnlock>, RepoError> {
        Ok(self
            .hint_unlocks
            .read()
            .await
            .iter()
            .filter(|u| {
                u.challenge_id == challenge_id
                    && match team_id {
                        Some(t) => u.team_id.as_ref() == Some(t),
                        None => u.team_id.is_none() && &u.account_id == account_id,
                    }
            })
            .cloned()
            .collect())
    }
    async fn save(&self, unlock: HintUnlock) -> Result<(), RepoError> {
        self.hint_unlocks.write().await.push(unlock);
        Ok(())
    }
}

#[async_trait]
impl FileRepo for TestStore {
    async fn save(&self, file: StoredFile) -> Result<(), RepoError> {
        self.files.write().await.insert(file.id.clone(), file);
        Ok(())
    }
    async fn find_by_id(&self, id: &str) -> Result<Option<StoredFile>, RepoError> {
        Ok(self.files.read().await.get(id).cloned())
    }
    async fn delete(&self, id: &str) -> Result<(), RepoError> {
        self.files.write().await.remove(id);
        Ok(())
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
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        hint_service: Arc::new(HintService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        file_service: Arc::new(FileService {
            storage: Arc::new(crate::libs::services::storage::LocalFileStorage::new(
                std::env::temp_dir().join("cctf-test-files"),
            )),
            repo: store.clone(),
            max_bytes: 25 * 1024 * 1024,
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
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        hint_service: Arc::new(HintService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        file_service: Arc::new(FileService {
            storage: Arc::new(crate::libs::services::storage::LocalFileStorage::new(
                std::env::temp_dir().join("cctf-test-files"),
            )),
            repo: store.clone(),
            max_bytes: 25 * 1024 * 1024,
        }),
        jwt_secret: b"secret".to_vec(),
        http_client: reqwest::Client::new(),
        rate_limiter: Arc::new(RateLimiter::new()),
        bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(load_bracket_scripts())),
    };

    let app = create_router(state);

    let token = generate_invite_token("team-col", chrono::Utc::now().timestamp() + 3600, b"secret");

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

fn build_test_app() -> Router {
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
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        hint_service: Arc::new(HintService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        }),
        file_service: Arc::new(FileService {
            storage: Arc::new(crate::libs::services::storage::LocalFileStorage::new(
                std::env::temp_dir().join("cctf-test-files"),
            )),
            repo: store.clone(),
            max_bytes: 25 * 1024 * 1024,
        }),
        jwt_secret: b"secret".to_vec(),
        http_client: reqwest::Client::new(),
        rate_limiter: Arc::new(RateLimiter::new()),
        bracket_acl_scripts: Arc::new(tokio::sync::RwLock::new(load_bracket_scripts())),
    };
    create_router(state)
}

#[tokio::test]
async fn router_covers_all_api_routes() {
    let app = build_test_app();
    for (method, path) in API_ROUTES {
        let uri = path.replace("{id}", "x");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(*method)
                    .uri(&uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "route not registered: {} {}",
            method,
            path
        );
    }
}

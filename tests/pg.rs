use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use cctf_rs::libs::{
    repos::{AccountRepo, pg::PgStore},
    services::{AuthService, ConfigService},
    types::accounts::{Account, AccountEmail, AccountId, AccountName, AccountRole}
};

static DB_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

async fn fresh_store() -> Option<Arc<PgStore>> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    let store = Arc::new(PgStore::new(pool.clone()));
    store.init_db().await.expect("init db");
    sqlx::query("TRUNCATE accounts, teams, challenges, submissions, challenge_instances, ctf_config CASCADE")
        .execute(&pool)
        .await
        .expect("truncate");
    Some(store)
}

#[tokio::test]
async fn pg_account_roundtrip() {
    let _guard = DB_LOCK.lock().await;
    let Some(store) = fresh_store().await else {
        eprintln!("skipping pg_accuont_roundtrip - set TEST_DATABASE_URL to run");
        return;
    };
    let acct = Account {
        id: AccountId("acc-1".to_string()),
        username: AccountName("user-1".to_string()),
        email: Some(AccountEmail("user1@cctf.rs".to_string())),
        password_hash: Some("hash".to_string()),
        role: AccountRole::Admin,
        team_id: None,
        ctftime_id: None,
        fields: Default::default(),
        created_at: 1_700_000_000,
    };
    AccountRepo::save(store.as_ref(), acct).await.unwrap();
    let by_id = AccountRepo::find_by_id(store.as_ref(), &AccountId("acc-1".to_string()))
        .await
        .unwrap()
        .expect("account should exist");
    assert_eq!(by_id.username.0, "user-1");
    assert_eq!(by_id.email.unwrap().0, "user1@cctf.rs");
    assert!(matches!(by_id.role, AccountRole::Admin));
    let by_name = AccountRepo::find_by_username(store.as_ref(), &AccountName("user-1".to_string()))
        .await
        .unwrap();
    assert!(by_name.is_some());
}

#[tokio::test]
async fn pg_config_upsert() {
    let _guard = DB_LOCK.lock().await;
    let Some(store) = fresh_store().await else {
        eprintln!("skipping pg_config_upsert - set TEST_DATABASE_URL to run");
        return;
    };
    let svc = ConfigService { config_repo: store.clone() };
    let cfg = svc.get().await.unwrap();
    assert!(cfg.registration_open);
    assert_eq!(cfg.freeze_time, None);
    let mut updated = cfg;
    updated.freeze_time = Some(1_700_000_500);
    updated.sort_by_accuracy = true;
    updated.registration_open = false;
    svc.update(updated).await.unwrap();
    let after = svc.get().await.unwrap();
    assert_eq!(after.freeze_time, Some(1_700_000_500));
    assert!(after.sort_by_accuracy);
    assert!(!after.registration_open);
    let mut again = after;
    again.ctf_name = "chordjack ctf".to_string();
    svc.update(again).await.unwrap();
    assert_eq!(svc.get().await.unwrap().ctf_name, "chordjack ctf");
}

#[tokio::test]
async fn pg_auth_register_and_login() {
    let _guard = DB_LOCK.lock().await;
    let Some(store) = fresh_store().await else {
        eprintln!("skipping pg_auth_register_and_login - set TEST_DATABASE_URL to run");
        return;
    };
    let auth = AuthService {
        account_repo: store.clone(),
        team_repo: store.clone(),
        jwt_secret: b"integration-secret".to_vec(),
    };
    let account = auth
        .register("user-1", Some("user1@cctf.rs"), "hunter2")
        .await
        .unwrap();
    assert_eq!(account.username.0, "user-1");
    let token = auth.login("user-1", "hunter2").await.unwrap();
    assert!(!token.is_empty());
    assert!(auth.login("user-1", "wrongpassword").await.is_err());
}

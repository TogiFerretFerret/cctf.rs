use crate::libs::crypto::jwt;
use crate::libs::repos::{AccountRepo, ChallengeRepo, RepoError, SubmissionRepo, TeamRepo};
use crate::libs::types::accounts::{
    Account, AccountEmail, AccountId, AccountName, AccountRole, CtfTimeUserProfile,
};
use crate::libs::types::challenges::{Challenge, ScoringMode};
use crate::libs::types::flags::FlagValidator;
use crate::libs::types::scoreboard::{
    CtfTimeScoreboardExport, CtfTimeStandingsEntry, CtfTimeTaskStats, ScoreboardEntry,
};
use crate::libs::types::solves::{Submission, SubmissionId};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use fluent_templates::{Loader, fluent_bundle::FluentValue, static_loader};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, Pod, PodSpec, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{Api, Client};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use unic_langid::langid;

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

#[derive(Debug)]
pub enum ServiceError {
    Repo(RepoError),
    OAuth(String),
    InvalidRequest(String),
    Unauthorized,
    Kube(String),
}

impl From<RepoError> for ServiceError {
    fn from(err: RepoError) -> Self {
        ServiceError::Repo(err)
    }
}

impl From<kube::Error> for ServiceError {
    fn from(err: kube::Error) -> Self {
        ServiceError::Kube(err.to_string())
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        ServiceError::Repo(RepoError::from(err))
    }
}

impl ServiceError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            ServiceError::Repo(err) => err.localize(lang),
            ServiceError::Unauthorized => LOCALES.lookup(&lang_id, "auth-not-logged-in"),
            ServiceError::InvalidRequest(key) => LOCALES.lookup(&lang_id, key),
            ServiceError::OAuth(reason) => {
                let args = HashMap::from([(
                    Cow::Borrowed("reason"),
                    FluentValue::from(reason.to_string()),
                )]);
                LOCALES.lookup_with_args(&lang_id, "oauth-invalid-credentials", &args)
            }
            ServiceError::Kube(reason) => {
                let args = HashMap::from([(
                    Cow::Borrowed("reason"),
                    FluentValue::from(reason.to_string()),
                )]);
                LOCALES.lookup_with_args(&lang_id, "kube-api-error", &args)
            }
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.localize("en-US"))
    }
}

impl std::error::Error for ServiceError {}

fn generate_salt() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let hash_hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    format!("{}${}", salt, hash_hex)
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parts: Vec<&str> = stored_hash.split('$').collect();
    if parts.len() != 2 {
        return false;
    }
    let salt = parts[0];
    let expected_hash = parts[1];
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let hash_hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    hash_hex == expected_hash
}

pub struct AuthService<A, T>
where
    A: AccountRepo,
    T: TeamRepo,
{
    pub account_repo: A,
    pub team_repo: T,
    pub jwt_secret: Vec<u8>,
}

impl<A, T> AuthService<A, T>
where
    A: AccountRepo,
    T: TeamRepo,
{
    pub async fn register(
        &self,
        username: &str,
        email: Option<&str>,
        password: &str,
    ) -> Result<Account, ServiceError> {
        let name = AccountName(username.to_string());
        if self.account_repo.find_by_username(&name).await?.is_some() {
            return Err(ServiceError::InvalidRequest(
                "auth-username-taken".to_string(),
            ));
        }
        let account_id = AccountId(uuid::Uuid::new_v4().to_string());
        let salt = generate_salt();
        let hashed = hash_password(password, &salt);
        let account = Account {
            id: account_id,
            username: name,
            email: email.map(|e| AccountEmail(e.to_string())),
            password_hash: Some(hashed),
            role: AccountRole::Player,
            team_id: None,
            ctftime_id: None,
            fields: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
        };
        self.account_repo.save(account.clone()).await?;
        Ok(account)
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<String, ServiceError> {
        let name = AccountName(username.to_string());
        let account = self
            .account_repo
            .find_by_username(&name)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        let stored_hash = account
            .password_hash
            .as_deref()
            .ok_or_else(|| ServiceError::InvalidRequest("auth-invalid-credentials".to_string()))?;
        if !verify_password(password, stored_hash) {
            return Err(ServiceError::InvalidRequest(
                "auth-invalid-credentials".to_string(),
            ));
        }
        let token = jwt::encode(&account.id.0, &self.jwt_secret)
            .map_err(|e| ServiceError::OAuth(e.to_string()))?;
        Ok(token)
    }
}

pub struct OAuthService<A, T>
where
    A: AccountRepo,
    T: TeamRepo,
{
    pub account_repo: A,
    pub team_repo: T,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub jwt_secret: Vec<u8>,
}

impl<A, T> OAuthService<A, T>
where
    A: AccountRepo,
    T: TeamRepo,
{
    pub fn get_authorize_url(&self) -> String {
        format!(
            "https://oauth.ctftime.org/authorize?client_id={}&redirect_uri={}&response_type=code&scope=profile+team",
            self.client_id, self.redirect_uri
        )
    }

    pub async fn handle_callback(&self, code: &str) -> Result<String, ServiceError> {
        let client = reqwest::Client::new();
        let token_resp = client
            .post("https://oauth.ctftime.org/token")
            .form(&[
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("redirect_uri", &self.redirect_uri),
                ("grant_type", &"authorization_code".to_string()),
                ("code", &code.to_string()),
            ])
            .send()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-token-failed".to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-token-parse-failed".to_string()))?;
        let access_token = token_resp
            .get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ServiceError::OAuth("auth-oauth-token-missing".to_string()))?;
        let profile = client
            .get("https://oauth.ctftime.org/user")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-profile-failed".to_string()))?
            .json::<CtfTimeUserProfile>()
            .await
            .map_err(|_| ServiceError::OAuth("auth-oauth-profile-parse-failed".to_string()))?;
        let account = match self.account_repo.find_by_ctftime_id(profile.id).await? {
            Some(acc) => acc,
            None => {
                let base_name = profile.username.clone();
                let mut check_name = AccountName(base_name.clone());
                let mut suffix = 1;
                while self
                    .account_repo
                    .find_by_username(&check_name)
                    .await?
                    .is_some()
                {
                    check_name = AccountName(format!("{}{}", base_name, suffix));
                    suffix += 1;
                } // TODO: this probably can't cause time issues because.. I know big O, but... it's
                // possible :shrug:
                let mut new_account = Account {
                    id: AccountId(uuid::Uuid::new_v4().to_string()),
                    username: check_name,
                    email: None,
                    password_hash: None,
                    role: AccountRole::Player,
                    team_id: None,
                    ctftime_id: Some(profile.id),
                    fields: HashMap::new(),
                    created_at: chrono::Utc::now().timestamp(),
                };
                let mut local_team_id = None;
                if let Some(ref ctftime_team) = profile.team {
                    let team = match self.team_repo.find_by_ctftime_id(ctftime_team.id).await? {
                        Some(t) => t,
                        None => {
                            let team_id = TeamId(uuid::Uuid::new_v4().to_string());
                            let new_team = Team {
                                id: team_id.clone(),
                                name: TeamName(ctftime_team.name.clone()),
                                ctftime_id: Some(ctftime_team.id),
                                invite_code: None,
                                captain_id: new_account.id.clone(),
                                member_ids: vec![new_account.id.clone()],
                                fields: HashMap::new(),
                                create_at: chrono::Utc::now().timestamp(),
                            };
                            self.team_repo.save(new_team.clone()).await?;
                            new_team
                        }
                    };
                    local_team_id = Some(team.id.clone());
                    new_account.team_id = Some(team.id.clone());
                }
                self.account_repo.save(new_account.clone()).await?;
                if let Some(t_id) = local_team_id {
                    if let Some(mut team) = self.team_repo.find_by_id(&t_id).await? {
                        if !team.member_ids.contains(&new_account.id) {
                            team.member_ids.push(new_account.id.clone());
                            self.team_repo.update(team).await?;
                        }
                    }
                }
                new_account
            }
        };
        let local_token = jwt::encode(&account.id.0, &self.jwt_secret)
            .map_err(|_| ServiceError::OAuth("auth-token-generation-failed".to_string()))?;
        Ok(local_token)
    }
}

pub struct SolveService<C, S>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub challenge_repo: C,
    pub submission_repo: S,
}

impl<C, S> SolveService<C, S>
where
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub async fn submit_flag(
        &self,
        challenge_id: &str,
        team_id: Option<TeamId>,
        account_id: AccountId,
        submitted_flag: &str,
    ) -> Result<Submission, ServiceError> {
        let challenge = self
            .challenge_repo
            .find_by_id(challenge_id)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-challenge-not-found".to_string()))?;
        if let Some(ref t_id) = team_id {
            let subs = self.submission_repo.find_by_team(t_id).await?;
            if subs
                .iter()
                .any(|s| s.challenge_id == challenge_id && s.is_correct)
            {
                return Err(ServiceError::InvalidRequest(
                    "ctf-already-solved".to_string(),
                ));
            }
        }
        let is_correct = match &challenge.flag {
            FlagValidator::Static(flag) => flag.trim() == submitted_flag.trim(),
            FlagValidator::Regex(pattern) => {
                let re = regex::Regex::new(pattern)
                    .map_err(|_| ServiceError::InvalidRequest("admin-invalid-regex".to_string()))?;
                re.is_match(submitted_flag.trim())
            }
            FlagValidator::Script(_) => false,
            FlagValidator::Instanced => false,
        };

        let _total_solves = self
            .submission_repo
            .find_all()
            .await?
            .iter()
            .filter(|s| s.challenge_id == challenge_id && s.is_correct)
            .count() as u32;

        let points_awarded = if is_correct {
            match challenge.points.mode {
                ScoringMode::PointValue => challenge.points.equation.parse::<u32>().unwrap_or(100),
                ScoringMode::PointAttribution => {
                    challenge.points.equation.parse::<u32>().unwrap_or(100)
                }
            }
        } else {
            0
        };

        let submission = Submission {
            id: SubmissionId(uuid::Uuid::new_v4().to_string()),
            challenge_id: challenge_id.to_string(),
            team_id,
            account_id,
            points: points_awarded,
            provided_flag: submitted_flag.to_string(),
            is_correct,
            submitted_at: chrono::Utc::now().timestamp(),
        };

        self.submission_repo.save(submission.clone()).await?;

        if !is_correct {
            return Err(ServiceError::InvalidRequest(
                "ctf-incorrect-flag".to_string(),
            ));
        }

        Ok(submission)
    }
}

pub struct ScoreboardService<T, C, S>
where
    T: TeamRepo,
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub team_repo: T,
    pub challenge_repo: C,
    pub submission_repo: S,
    pub sort_by_accuracy: bool,
}

impl<T, C, S> ScoreboardService<T, C, S>
where
    T: TeamRepo,
    C: ChallengeRepo,
    S: SubmissionRepo,
{
    pub async fn get_scoreboard(&self) -> Result<Vec<ScoreboardEntry>, ServiceError> {
        let teams = self.team_repo.find_all().await?;
        let submissions = self.submission_repo.find_all().await?;
        let challenges = self.challenge_repo.find_all().await?;
        let challenge_map: HashMap<String, &Challenge> =
            challenges.iter().map(|c| (c.id.clone(), c)).collect();
        let mut solve_counts = HashMap::new();
        for sub in &submissions {
            if sub.is_correct {
                *solve_counts.entry(sub.challenge_id.clone()).or_insert(0) += 1;
            }
        }
        let mut entries = Vec::new();
        for team in teams {
            let team_subs: Vec<&Submission> = submissions
                .iter()
                .filter(|s| s.team_id.as_ref() == Some(&team.id))
                .collect();
            let mut points = 0;
            let mut last_solve_time = None;
            let mut solved_ids = Vec::new();
            for sub in team_subs {
                if sub.is_correct {
                    if let Some(challenge) = challenge_map.get(&sub.challenge_id) {
                        let challenge_points = match challenge.points.mode {
                            ScoringMode::PointValue => {
                                challenge.points.equation.parse::<u32>().unwrap_or(100)
                            }
                            ScoringMode::PointAttribution => sub.points,
                        };
                        points += challenge_points;
                        solved_ids.push(sub.challenge_id.clone());

                        last_solve_time = match last_solve_time {
                            None => Some(sub.submitted_at),
                            Some(t) => Some(t.max(sub.submitted_at)),
                        };
                    }
                }
            }
            entries.push(ScoreboardEntry {
                team_id: team.id,
                team_name: team.name.0,
                points,
                last_solve_time,
                solves: solved_ids,
                rank: 0,
            });
        }
        if self.sort_by_accuracy {
            let get_accuracy = |team_id: &TeamId| -> f64 {
                let subs: Vec<&Submission> = submissions
                    .iter()
                    .filter(|s| s.team_id.as_ref() == Some(team_id))
                    .collect();
                if subs.is_empty() {
                    1.0
                } else {
                    (subs.iter().filter(|s| s.is_correct).count() as f64) / (subs.len() as f64)
                }
            };
            entries.sort_by(|a, b| {
                b.points.cmp(&a.points).then_with(|| {
                    let acc_a = get_accuracy(&a.team_id);
                    let acc_b = get_accuracy(&b.team_id);
                    acc_b
                        .partial_cmp(&acc_a)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
        } else {
            entries.sort_by(|a, b| {
                b.points
                    .cmp(&a.points)
                    .then_with(|| match (a.last_solve_time, b.last_solve_time) {
                        (Some(t1), Some(t2)) => t1.cmp(&t2),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    })
            });
        }
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = (i + 1) as u32;
        }
        Ok(entries)
    }

    pub async fn export_ctftime(&self) -> Result<CtfTimeScoreboardExport, ServiceError> {
        let standings = self.get_scoreboard().await?;
        let submissions = self.submission_repo.find_all().await?;
        let challenges = self.challenge_repo.find_all().await?;
        let challenge_map: HashMap<String, &Challenge> =
            challenges.iter().map(|c| (c.id.clone(), c)).collect();
        let tasks: Vec<String> = challenges.iter().map(|c| c.title.0.clone()).collect();
        let mut ctftime_standings = Vec::new();
        for entry in standings {
            let mut task_stats = HashMap::new();
            let team_solves: Vec<&Submission> = submissions
                .iter()
                .filter(|s| s.team_id.as_ref() == Some(&entry.team_id) && s.is_correct)
                .collect();
            for solve in team_solves {
                if let Some(challenge) = challenge_map.get(&solve.challenge_id) {
                    task_stats.insert(
                        challenge.title.0.clone(),
                        CtfTimeTaskStats {
                            points: solve.points,
                            time: solve.submitted_at,
                        },
                    );
                }
            }
            ctftime_standings.push(CtfTimeStandingsEntry {
                pos: Some(entry.rank),
                team: entry.team_name,
                score: entry.points as f64,
                task_stats,
            });
        }
        Ok(CtfTimeScoreboardExport {
            tasks,
            standings: ctftime_standings,
        })
    }
}

#[derive(Clone)]
pub struct InstancerService {
    pub client: Client,
    pub namespace: String,
    pub db_pool: sqlx::PgPool,
}

fn render_instanced_flag(template: &str, challenge_id: &str, random_part: &str) -> String {
    let mut rendered = template.to_string();
    if rendered.contains("{{random}}") || rendered.contains("{{uuid}}") {
        rendered = rendered.replace("{{random}}", random_part);
        rendered = rendered.replace("{{uuid}}", random_part);
    } else if rendered.ends_with('}') && rendered.len() > 1 {
        let pos = rendered.len() - 1;
        if rendered.chars().nth(pos - 1) == Some('{') {
            rendered.insert_str(pos, random_part);
        } else {
            rendered.insert_str(pos, &format!("_{}", random_part));
        }
    } else {
        rendered.push_str(random_part);
    }
    rendered.replace("{{challenge}}", challenge_id)
}

impl InstancerService {
    pub async fn new(namespace: String, db_pool: sqlx::PgPool) -> Result<Self, kube::Error> {
        let client = Client::try_default().await?;
        Ok(Self {
            client,
            namespace,
            db_pool,
        })
    }
    pub async fn spawn_instance(
        &self,
        challenge_id: &str,
        image: &str,
        container_port: i32,
        team_id: Option<&str>,
        account_id: &str,
        flag_template: &str,
        lifespan_seconds: i64,
    ) -> Result<String, ServiceError> {
        let instance_id = format!("inst-{}", uuid::Uuid::new_v4().simple());
        let random_part = uuid::Uuid::new_v4().simple().to_string();
        let generated_flag = render_instanced_flag(flag_template, challenge_id, &random_part);
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), &self.namespace);
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), instance_id.clone());
        labels.insert("challenge".to_string(), challenge_id.to_string());
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some(instance_id.clone()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "challenge".to_string(),
                    image: Some(image.to_string()),
                    ports: Some(vec![ContainerPort {
                        container_port,
                        ..Default::default()
                    }]),
                    env: Some(vec![k8s_openapi::api::core::v1::EnvVar {
                        name: "FLAG".to_string(),
                        value: Some(generated_flag.clone()),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let service = Service {
            metadata: ObjectMeta {
                name: Some(instance_id.clone()),
                labels: Some(labels.clone()),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                selector: Some(labels.clone()),
                ports: Some(vec![ServicePort {
                    port: container_port,
                    target_port: Some(
                        k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                            container_port,
                        ),
                    ),
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            ..Default::default()
        };
        pods.create(&kube::api::PostParams::default(), &pod).await?;
        services
            .create(&kube::api::PostParams::default(), &service)
            .await?;
        let created_at = chrono::Utc::now().timestamp();
        let expires_at = created_at + lifespan_seconds;
        sqlx::query("INSERT INTO challenge_instances (id, challenge_id, team_id, account_id, flag, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(&instance_id)
            .bind(challenge_id)
            .bind(team_id)
            .bind(account_id)
            .bind(&generated_flag)
            .bind(created_at)
            .bind(expires_at)
            .execute(&self.db_pool)
            .await?;
        self.schedule_reap(instance_id.clone(), lifespan_seconds as u64);
        Ok(instance_id)
    }
    pub async fn destroy_instances(&self, instance_id: &str) -> Result<(), ServiceError> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        let services: Api<Service> = Api::namespaced(self.client.clone(), &self.namespace);
        let _ = services
            .delete(instance_id, &kube::api::DeleteParams::default())
            .await;
        let _ = pods
            .delete(instance_id, &kube::api::DeleteParams::default())
            .await;
        sqlx::query("DELETE FROM challenge_instances WHERE id = $1")
            .bind(instance_id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    pub async fn renew_instance(
        &self,
        instance_id: &str,
        duration_seconds: i64,
    ) -> Result<(), ServiceError> {
        let now = chrono::Utc::now().timestamp();
        let rows_Affected = sqlx::query(
            "UPDATE challenge_instances SET expires_at = $1 WHERE id = $2 AND expires_at > $3",
        )
        .bind(now + duration_seconds)
        .bind(instance_id)
        .bind(now)
        .execute(&self.db_pool)
        .await?
        .rows_affected();
        if rows_Affected == 0 {
            return Err(ServiceError::InvalidRequest(
                "ctf-instance-expired-or-not-found".to_string(),
            ));
        }
        Ok(())
    }
    pub async fn reap_expired_instances(&self) -> Result<(), ServiceError> {
        let now = chrono::Utc::now().timestamp();
        let rows = sqlx::query("SELECT id FROM challenge_instances WHERE expires_at <= $1")
            .bind(now)
            .fetch_all(&self.db_pool)
            .await?;
        for row in rows {
            let id: String = row.try_get("id").map_err(RepoError::from)?;
            let _ = self.destroy_instances(&id).await;
        }
        Ok(())
    }
    pub fn schedule_reap(&self, instance_id: String, delay_seconds: u64) {
        let self_clone = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
            if let Err(e) = self_clone.destroy_instances(&instance_id).await {
                eprintln!("Failed to precise reap instance {}: {:?}", instance_id, e);
            }
        });
    }
    pub async fn init_repaer_schedules(&self) -> Result<(), ServiceError> {
        let now = chrono::Utc::now().timestamp();
        let active: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, expires_at FROM challenge_instances WHERE expires_at > $1")
                .bind(now)
                .fetch_all(&self.db_pool)
                .await?;
        for (id, expires_at) in active {
            let delay = std::cmp::max(0, expires_at - now) as u64;
            self.schedule_reap(id, delay);
        }
        self.reap_expired_instances().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::RwLock;

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
    async fn test_auth_and_submissions() {
        let store = Arc::new(TestStore::default());
        let auth = AuthService {
            account_repo: store.clone(),
            team_repo: store.clone(),
            jwt_secret: b"secret".to_vec(),
        };
        let account = auth
            .register(
                "unittest",
                Some("unittest@example.com"),
                "unittest_password",
            )
            .await
            .unwrap();
        assert_eq!(account.username.0, "unittest");
        let token = auth.login("unittest", "unittest_password").await.unwrap();
        assert!(!token.is_empty());
        let bad_login = auth.login("unittest", "wrong_password").await;
        assert!(bad_login.is_err());
    }
    #[tokio::test]
    async fn test_scoreboard_ranking_and_accuracy() {
        let store = Arc::new(TestStore::default());
        let team_a = Team {
            id: TeamId("team-a".to_string()),
            name: TeamName("Team A".to_string()),
            ctftime_id: None,
            invite_code: None,
            captain_id: AccountId("captain-a".to_string()),
            member_ids: vec![AccountId("captain-a".to_string())],
            fields: HashMap::new(),
            create_at: 0,
        };
        let team_b = Team {
            id: TeamId("team-b".to_string()),
            name: TeamName("Team B".to_string()),
            ctftime_id: None,
            invite_code: None,
            captain_id: AccountId("captain-b".to_string()),
            member_ids: vec![AccountId("captain-b".to_string())],
            fields: HashMap::new(),
            create_at: 0,
        };
        TeamRepo::save(store.as_ref(), team_a).await.unwrap();
        TeamRepo::save(store.as_ref(), team_b).await.unwrap();
        let challenge = Challenge {
            id: "chall-1".to_string(),
            title: crate::libs::types::challenges::ChallengeTitle("Chall 1".to_string()),
            description: crate::libs::types::challenges::ChallengeDescription(
                crate::libs::types::htmlstring::HtmlString("Desc".to_string()),
            ),
            category: crate::libs::types::challenges::ChallengeCategory("Web".to_string()),
            points: crate::libs::types::challenges::ChallengePoints {
                mode: ScoringMode::PointValue,
                equation: "500".to_string(),
            },
            flag: FlagValidator::Static("flag{test}".to_string()),
            author: crate::libs::types::challenges::ChallengeAuthor {
                id: "admin".to_string(),
                username: "admin".to_string(),
            },
            hints: Vec::new(),
            files: Vec::new(),
            tags: Vec::new(),
            requirements: Vec::new(),
        };
        ChallengeRepo::save(store.as_ref(), challenge)
            .await
            .unwrap();
        let solver = SolveService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
        };
        solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-a".to_string())),
                AccountId("user-1".to_string()),
                "flag{test}",
            )
            .await
            .unwrap();
        let fail = solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-b".to_string())),
                AccountId("user-2".to_string()),
                "wrong-flag",
            )
            .await;
        assert!(fail.is_err());
        solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-b".to_string())),
                AccountId("user-2".to_string()),
                "flag{test}",
            )
            .await
            .unwrap();
        {
            let mut subs = store.submissions.write().await;
            if let Some(s) = subs.iter_mut().find(|s| {
                s.team_id.as_ref().map(|t| &t.0) == Some(&"team-a".to_string()) && s.is_correct
            }) {
                s.submitted_at = 100;
            }
            if let Some(s) = subs.iter_mut().find(|s| {
                s.team_id.as_ref().map(|t| &t.0) == Some(&"team-b".to_string()) && s.is_correct
            }) {
                s.submitted_at = 200;
            }
        }
        let scoreboard_service = ScoreboardService {
            team_repo: store.clone(),
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            sort_by_accuracy: false,
        };
        let board = scoreboard_service.get_scoreboard().await.unwrap();
        assert_eq!(board[0].team_name, "Team A");
        assert_eq!(board[1].team_name, "Team B");
        let scoreboard_service_acc = ScoreboardService {
            team_repo: store.clone(),
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            sort_by_accuracy: true,
        };
        let board_acc = scoreboard_service_acc.get_scoreboard().await.unwrap();
        assert_eq!(board_acc[0].team_name, "Team A");
    }
    #[test]
    fn test_render_instanced_flag() {
        assert_eq!(
            render_instanced_flag("hc{chall_{{random}}}", "pwn-1", "abcde123"),
            "hc{chall_abcde123}"
        );
        assert_eq!(
            render_instanced_flag("flag{{{uuid}}}", "web-2", "9999"),
            "flag{9999}"
        );
        assert_eq!(
            render_instanced_flag("flag{{{challenge}}_{{random}}}", "crypto-1", "xyz"),
            "flag{crypto-1_xyz}"
        );
        assert_eq!(
            render_instanced_flag("flag{}", "misc-1", "1234"),
            "flag{1234}"
        );
        assert_eq!(
            render_instanced_flag("flag{static}", "pwn-2", "5678"),
            "flag{static_5678}"
        );
        assert_eq!(
            render_instanced_flag("raw_string", "pwn-3", "999"),
            "raw_string999"
        );
    }
}

use super::ServiceError;
use crate::libs::repos::RepoError;
use k8s_openapi::{
    api::core::v1::{Container, ContainerPort, Pod, PodSpec, Service, ServicePort, ServiceSpec},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{Api, Client};
use sqlx::Row;
use std::collections::BTreeMap;

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

#[derive(Clone)]
pub struct InstancerService {
    pub client: Client,
    pub namespace: String,
    pub db_pool: sqlx::PgPool,
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
    #[allow(clippy::too_many_arguments)]
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
        let created_service = services
            .create(&kube::api::PostParams::default(), &service)
            .await?;
        let cluster_ip = created_service
            .spec
            .and_then(|spec| spec.cluster_ip)
            .ok_or_else(|| ServiceError::Kube("service-cluster-ip-missing".to_string()))?;
        let created_at = chrono::Utc::now().timestamp();
        let expires_at = created_at + lifespan_seconds;
        sqlx::query("INSERT INTO challenge_instances (id, challenge_id, team_id, account_id, flag, cluster_ip, created_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
            .bind(&instance_id)
            .bind(challenge_id)
            .bind(team_id)
            .bind(account_id)
            .bind(&generated_flag)
            .bind(&cluster_ip)
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
        let rows_affected = sqlx::query(
            "UPDATE challenge_instances SET expires_at = $1 WHERE id = $2 AND expires_at > $3",
        )
        .bind(now + duration_seconds)
        .bind(instance_id)
        .bind(now)
        .execute(&self.db_pool)
        .await?
        .rows_affected();
        if rows_affected == 0 {
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
    pub async fn init_reaper_schedules(&self) -> Result<(), ServiceError> {
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
    use crate::libs::repos::{
        AccountRepo, ChallengeRepo, HintUnlockRepo, InstanceRepo, SubmissionRepo, TeamRepo,
    };
    use crate::libs::services::{
        auth::AuthService,
        scoreboard::ScoreboardService,
        solve::{SolveService, calculate_dynamic_points},
    };
    use crate::libs::types::{
        accounts::{Account, AccountId, AccountName},
        challenges::{Challenge, ScoringMode},
        config::HintDeductionMode,
        flags::FlagValidator,
        solves::{HintUnlock, Submission},
        teams::{Team, TeamId, TeamName},
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[derive(Default)]
    struct TestStore {
        accounts: RwLock<HashMap<AccountId, Account>>,
        teams: RwLock<HashMap<TeamId, Team>>,
        challenges: RwLock<HashMap<String, Challenge>>,
        submissions: RwLock<Vec<Submission>>,
        hint_unlocks: RwLock<Vec<HintUnlock>>,
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
            bracket: "Open".to_string(),
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
            bracket: "Open".to_string(),
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
            deployment: crate::libs::types::challenges::ChallengeDeployment::None,
            visibility: crate::libs::types::challenges::ChallengeVisibility::Visible,
            team_consensus: false,
        };
        ChallengeRepo::save(store.as_ref(), challenge)
            .await
            .unwrap();
        let solver = SolveService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
        };
        solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-a".to_string())),
                AccountId("user-1".to_string()),
                "flag{test}",
                "127.0.0.1",
            )
            .await
            .unwrap();
        let fail = solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-b".to_string())),
                AccountId("user-2".to_string()),
                "wrong-flag",
                "127.0.0.1",
            )
            .await;
        assert!(fail.is_err());
        solver
            .submit_flag(
                "chall-1",
                Some(TeamId("team-b".to_string())),
                AccountId("user-2".to_string()),
                "flag{test}",
                "127.0.0.1",
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
            freeze_time: None,
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        };
        let board = scoreboard_service.get_scoreboard(None).await.unwrap();
        assert_eq!(board[0].team_name, "Team A");
        assert_eq!(board[1].team_name, "Team B");
        let scoreboard_service_acc = ScoreboardService {
            team_repo: store.clone(),
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            sort_by_accuracy: true,
            freeze_time: None,
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        };
        let board_acc = scoreboard_service_acc.get_scoreboard(None).await.unwrap();
        assert_eq!(board_acc[0].team_name, "Team A");
    }
    #[test]
    fn test_render_instanced_flag() {
        assert_eq!(
            super::render_instanced_flag("hc{chall_{{random}}}", "pwn-1", "abcde123"),
            "hc{chall_abcde123}"
        );
        assert_eq!(
            super::render_instanced_flag("flag{{{uuid}}}", "web-2", "9999"),
            "flag{9999}"
        );
        assert_eq!(
            super::render_instanced_flag("flag{{{challenge}}_{{random}}}", "crypto-1", "xyz"),
            "flag{crypto-1_xyz}"
        );
        assert_eq!(
            super::render_instanced_flag("flag{}", "misc-1", "1234"),
            "flag{1234}"
        );
        assert_eq!(
            super::render_instanced_flag("flag{static}", "pwn-2", "5678"),
            "flag{static_5678}"
        );
        assert_eq!(
            super::render_instanced_flag("raw_string", "pwn-3", "999"),
            "raw_string999"
        );
    }

    #[tokio::test]
    async fn test_solve_consensus_and_decay() {
        let store = Arc::new(TestStore::default());
        let solver = SolveService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
        };
        let team_a = Team {
            id: TeamId("team-a".to_string()),
            name: TeamName("Team Consensus A".to_string()),
            ctftime_id: None,
            invite_code: None,
            captain_id: AccountId("user-1".to_string()),
            member_ids: vec![
                AccountId("user-1".to_string()),
                AccountId("user-2".to_string()),
            ],
            bracket: "Open".to_string(),
            fields: HashMap::new(),
            create_at: 0,
        };
        TeamRepo::save(store.as_ref(), team_a).await.unwrap();
        let challenge = Challenge {
            id: "chall-consensus".to_string(),
            title: crate::libs::types::challenges::ChallengeTitle("Consensus Chall".to_string()),
            description: crate::libs::types::challenges::ChallengeDescription(
                crate::libs::types::htmlstring::HtmlString("Desc".to_string()),
            ),
            category: crate::libs::types::challenges::ChallengeCategory("Web".to_string()),
            points: crate::libs::types::challenges::ChallengePoints {
                mode: ScoringMode::DynamicDecay {
                    initial: 500,
                    minimum: 100,
                    decay: 5,
                },
                equation: "500,100,5".to_string(),
            },
            flag: FlagValidator::Static("flag{consensus}".to_string()),
            author: crate::libs::types::challenges::ChallengeAuthor {
                id: "admin".to_string(),
                username: "admin".to_string(),
            },
            hints: Vec::new(),
            files: Vec::new(),
            tags: Vec::new(),
            requirements: Vec::new(),
            deployment: crate::libs::types::challenges::ChallengeDeployment::None,
            visibility: crate::libs::types::challenges::ChallengeVisibility::Visible,
            team_consensus: true,
        };
        ChallengeRepo::save(store.as_ref(), challenge)
            .await
            .unwrap();
        let sub1 = solver
            .submit_flag(
                "chall-consensus",
                Some(TeamId("team-a".to_string())),
                AccountId("user-1".to_string()),
                "flag{consensus}",
                "127.0.0.1",
            )
            .await
            .unwrap();
        assert_eq!(sub1.points, 500);
        let scoreboard_service = ScoreboardService {
            team_repo: store.clone(),
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            sort_by_accuracy: false,
            freeze_time: None,
            hint_unlock_repo: store.clone(),
            hint_deduction_mode: HintDeductionMode::FloorZero,
        };
        let board = scoreboard_service.get_scoreboard(None).await.unwrap();
        assert_eq!(board[0].points, 0);
        let sub2 = solver
            .submit_flag(
                "chall-consensus",
                Some(TeamId("team-a".to_string())),
                AccountId("user-2".to_string()),
                "flag{consensus}",
                "127.0.0.1",
            )
            .await
            .unwrap();
        assert_eq!(sub2.points, 500);
        let board = scoreboard_service.get_scoreboard(None).await.unwrap();
        assert_eq!(board[0].points, 500);
        use crate::libs::types::flags::PartialFlag;
        let partial_challenge = Challenge {
            id: "chall-partial".to_string(),
            title: crate::libs::types::challenges::ChallengeTitle("Partial Chall".to_string()),
            description: crate::libs::types::challenges::ChallengeDescription(
                crate::libs::types::htmlstring::HtmlString("Desc".to_string()),
            ),
            category: crate::libs::types::challenges::ChallengeCategory("Web".to_string()),
            points: crate::libs::types::challenges::ChallengePoints {
                mode: ScoringMode::PointValue,
                equation: "200".to_string(),
            },
            flag: FlagValidator::Multi(vec![
                PartialFlag {
                    id: "part-1".to_string(),
                    validator: FlagValidator::Static("flag{part1}".to_string()),
                    weight: 0.25,
                },
                PartialFlag {
                    id: "part-2".to_string(),
                    validator: FlagValidator::Static("flag{part2}".to_string()),
                    weight: 0.75,
                },
            ]),
            author: crate::libs::types::challenges::ChallengeAuthor {
                id: "admin".to_string(),
                username: "admin".to_string(),
            },
            hints: Vec::new(),
            files: Vec::new(),
            tags: Vec::new(),
            requirements: Vec::new(),
            deployment: crate::libs::types::challenges::ChallengeDeployment::None,
            visibility: crate::libs::types::challenges::ChallengeVisibility::Visible,
            team_consensus: false,
        };
        ChallengeRepo::save(store.as_ref(), partial_challenge)
            .await
            .unwrap();
        let psub1 = solver
            .submit_flag(
                "chall-partial",
                Some(TeamId("team-a".to_string())),
                AccountId("user-1".to_string()),
                "flag{part1}",
                "127.0.0.1",
            )
            .await
            .unwrap();
        assert_eq!(psub1.points, 50);
        let board = scoreboard_service.get_scoreboard(None).await.unwrap();
        assert_eq!(board[0].points, 550);
        let psub1_dup = solver
            .submit_flag(
                "chall-partial",
                Some(TeamId("team-a".to_string())),
                AccountId("user-1".to_string()),
                "flag{part1}",
                "127.0.0.1",
            )
            .await;
        assert!(psub1_dup.is_err());
    }

    #[tokio::test]
    async fn test_rhai_script_and_dynamic_decay_curve() {
        let store = Arc::new(TestStore::default());
        let solver = SolveService {
            challenge_repo: store.clone(),
            submission_repo: store.clone(),
            team_repo: store.clone(),
        };

        let rhai_script = r#"
            let is_valid = flag.len() == 8 && flag.ends_with("abc");
            is_valid
        "#;

        let script_challenge = Challenge {
            id: "chall-script".to_string(),
            title: crate::libs::types::challenges::ChallengeTitle("Rhai Script Chall".to_string()),
            description: crate::libs::types::challenges::ChallengeDescription(
                crate::libs::types::htmlstring::HtmlString("Desc".to_string()),
            ),
            category: crate::libs::types::challenges::ChallengeCategory("Web".to_string()),
            points: crate::libs::types::challenges::ChallengePoints {
                mode: ScoringMode::PointValue,
                equation: "100".to_string(),
            },
            flag: FlagValidator::Script(rhai_script.to_string()),
            author: crate::libs::types::challenges::ChallengeAuthor {
                id: "admin".to_string(),
                username: "admin".to_string(),
            },
            hints: Vec::new(),
            files: Vec::new(),
            tags: Vec::new(),
            requirements: Vec::new(),
            deployment: crate::libs::types::challenges::ChallengeDeployment::None,
            visibility: crate::libs::types::challenges::ChallengeVisibility::Visible,
            team_consensus: false,
        };
        ChallengeRepo::save(store.as_ref(), script_challenge)
            .await
            .unwrap();

        let sub_ok = solver
            .submit_flag(
                "chall-script",
                None,
                AccountId("user-1".to_string()),
                "12345abc",
                "127.0.0.1",
            )
            .await
            .unwrap();
        assert!(sub_ok.is_correct);

        let sub_fail_len = solver
            .submit_flag(
                "chall-script",
                None,
                AccountId("user-1".to_string()),
                "123abc",
                "127.0.0.1",
            )
            .await;
        assert!(sub_fail_len.is_err());

        let sub_fail_suffix = solver
            .submit_flag(
                "chall-script",
                None,
                AccountId("user-1".to_string()),
                "12345678",
                "127.0.0.1",
            )
            .await;
        assert!(sub_fail_suffix.is_err());

        assert_eq!(calculate_dynamic_points(1000, 200, 3, 1), 1000);
        assert_eq!(calculate_dynamic_points(1000, 200, 3, 2), 920);
        assert_eq!(calculate_dynamic_points(1000, 200, 3, 4), 600);
    }
}

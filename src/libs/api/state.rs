use super::*;

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
    pub hint_service: Arc<HintService<C, S, T>>,
    pub file_service: Arc<FileService>,
    pub notification_service: Arc<NotificationService>,
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
            hint_service: self.hint_service.clone(),
            file_service: self.file_service.clone(),
            notification_service: self.notification_service.clone(),
        }
    }
}

use super::*;
use crate::libs::types::notifications::{
    Notification, NotificationId, NotificationKind, NotificationTarget,
};
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Deserialiaze)]
pub struct AnnouncePayload {
    pub title: String,
    pub message: HtmlString,
    #[serde(default = "everyone")]
    pub target: NotificationTarget,
}

fn everyone() -> NotificationTarget {
    NotificationTarget::Everyone
}

fn target_visible(
    target: &NotificationTarget,
    account_id: Option<&AccountId>,
    team_id: Option<&Teamid>,
) -> bool {
    match account_id {
        Some(acc) => target.matches(acc, team_id),
        None => matches!(target, NotificationTarget::Everyone),
    }
}

fn kind_event(kind: &NotificationKind) -> &'static str {
    match kind {
        NotificationKind::Announcement => "announcement",
        NotificationKind::Solve => "solve",
        NotificationKind::FirstBlood => "first_blood",
    }
}

async fn viewer_identity<A, T, C, S>(
    state: &AppState<A, T, C, S>,
    account_id: Option<AccountId>,
) -> (Option<AccountId>, Option<TeamId>)
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    match account_id {
        Some(acc) => {
            let team = state
                .auth_service
                .account_repo
                .find_by_id(&acc)
                .await
                .ok()
                .flatten()
                .and_then(|a| a.team_id);
            (Some(acc), team)
        }
        None => (None, None),
    }
}

async fn resolve_target<A, T, C, S>(
    state: &AppState<A, T, C, S>,
    target: NotificationTarget,
) -> NotificationTarget
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let NotificationTarget::Filter(expr) = target else {
        return target;
    };
    let accounts = state
        .auth_service
        .account_repo
        .find_all()
        .await
        .unwrap_or_default();
    let subs = state
        .solve_service
        .submission_repo
        .find_all()
        .await
        .unwrap_or_default();
    let mut matched = Vec::new();
    for account in &accounts {
        let solved: rhai::Array = subs
            .iter()
            .filter(|s| {
                s.is_correct
                    && match &account.team_id {
                        Some(t) => s.team_id.as_ref() == Some(t),
                        None => s.account_id == account_id,
                    }
            })
            .map(|s| rhai::Dynamic::from(s.challenge_id.clone()))
            .collect();
        let mut scope = rhai::Scope::new();
        scope.push("solved", solved);
        scope.push("username", account.username.0.clone());
        scope.push(
            "email",
            account
                .email
                .as_ref()
                .map(|e| e.0.clone())
                .unwrap_or_default(),
        );
        if engine
            .eval_with_scope::<bool>(&mut scope, &expr)
            .unwrap_or(false)
        {
            matched.push(account.id.clone());
        }
    }
    NotificationTarget::Accounts(matched)
}

pub async fn notifications_stream<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    viewer: OptionalUser,
    lang: PreferredLang,
) -> Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    if state.notification_service.config.require_auth && viewer.0.is_none() {
        return LocalizedError {
            status: StatusCode::UNAUTHORIZED,
            message: ServiceError::Unauthorized.localize(&lang.0),
        }
        .into_response();
    }
    let (account_id, team_id) = viewer_identity(&state, viewer.0).await;
    let rx = state.notification_service.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |res| {
        let n = res.ok()?;
        if !target_visible(&n.target, account_id.as_ref(), team_id.as_ref()) {
            return None;
        }
        let event = Event::default()
            .event(kind_event(&n.kind))
            .json_data(&n)
            .ok()?;
        Some(Ok::<Event, std::convert::Infallible>(event))
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub async fn create_notification<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _admin: AdminUser,
    lang: PreferredLang,
    Json(payload): Json<AnnouncePayload>,
) -> Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let target = resolve_target(&state, payload.target).await;
    let now = chrono::Utc::now().timestamp();
    match state
        .notification_service
        .announce(payload.title, payload.message, target, now)
        .await
        .map_localized(&lang.0)
    {
        Ok(n) => (StatusCode::CREATED, Json(n)).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn list_notifications<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    viewer: OptionalUser,
    lang: PreferredLang,
) -> Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    if state.notification_service.config.require_auth && viewer.0.is_none() {
        return LocalizedError {
            status: StatusCode::UNAUTHORIZED,
            message: ServiceError::Unauthorized.localize(&lang.0),
        }
        .into_response();
    }
    let (account_id, team_id) = viewer_identity(&state, viewer.0).await;
    match state
        .notification_service
        .list_recent(100)
        .await
        .map_localized(&lang.0)
    {
        Ok(list) => {
            let visible: Vec<Notification> = list
                .into_iter()
                .filter(|n| target_visible(&n.target, account_id.as_ref(), team_id.as_ref()))
                .collect();
            Json(visible).into_response()
        }
        Err(e) => e.into_response(),
    }
}

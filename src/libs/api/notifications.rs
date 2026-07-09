use super::*;
use crate::libs::types::notifications::{Notification, NotificationId, NotificationKind, NotificationTarget};
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

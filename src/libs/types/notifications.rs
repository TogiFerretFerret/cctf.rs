use crate::libs::types::{accounts::AccountId, htmlstring::HtmlString, teams::TeamId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationKind {
    Announcement,
    Solve,
    FirstBlood,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationTarget {
    Everyone,
    Teams(Vec<TeamId>),
    Accounts(Vec<AccountId>),
    Filter(String),
}

impl NotificationTarget {
    pub fn matches(&self, account_id: &AccountId, team_id: Option<&TeamId>) -> bool {
        match self {
            NotificationTarget::Everyone => true,
            NotificationTarget::Accounts(ids) => ids.contains(account_id),
            NotificationTarget::Teams(ids) => team_id.is_some_and(|t| ids.contains(t)),
            NotificationTarget::Filter(_) => false, // TODO: rhai
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: NotificationId,
    pub kind: NotificationKind,
    pub title: String,
    pub message: HtmlString,
    pub target: NotificationTarget,
    pub created_at: i64,
}

// TODO: add expiry/removing notifications
// TODO: also store per-user if they have viewed the notif or not so they don't get it over and over on diff devices

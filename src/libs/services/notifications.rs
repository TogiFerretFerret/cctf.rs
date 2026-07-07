use super::ServiceError;
use crate::libs::repos::NotificationRepo;
use crate::libs::types::{
    config::NotificationConfig,
    htmlstring::HtmlString,
    notifications::{Notification, NotificationId, NotificationKind, NotificationTarget},
};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct NotificationService {
    pub repo: Arc<dyn NotificationRepo>,
    pub sender: broadcast::Sender<Notification>,
    pub config: NotificationConfig,
}

impl NotificationService {
    pub fn new(repo: Arc<dyn NotificationRepo>, config: NotificationConfig) -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            repo,
            sender,
            config,
        }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.sender.subscribe()
    }
    pub async fn announce(
        &self,
        title: String,
        message: HtmlString,
        target: NotificationTarget,
        now: i64,
    ) -> Result<Notification, ServiceError> {
        let notification = Notification {
            id: NotificationId(uuid::Uuid::new_v4().to_string()),
            kind: NotificationKind::Announcement,
            title,
            message,
            target,
            created_at: now,
        };
        self.repo.save(notification.clone()).await?;
        let _ = self.sender.send(notification.clone());
        Ok(notification)
    }
    pub fn broadcast(&self, notification: Notification) {
        let _ = self.sender.send(notification);
    }
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<Notification>, ServiceError> {
        Ok(self.repo.list_recent(limit).await?)
    }
}

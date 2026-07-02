use super::ServiceError;
use crate::libs::repos::ConfigRepo;
use crate::libs::types::config::CtfConfig;

pub struct ConfigService<C: ConfigRepo> {
    pub config_repo: C,
}

impl<C: ConfigRepo> ConfigService<C> {
    pub async fn get(&self) -> Result<CtfConfig, ServiceError> {
        Ok(self.config_repo.get().await?)
    }
    pub async fn update(&self, config: CtfConfig) -> Result<(), ServiceError> {
        self.config_repo.set(config).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::repos::RepoError;
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct MemConfig {
        inner: Mutex<Option<CtfConfig>>,
    }

    #[async_trait]
    impl ConfigRepo for MemConfig {
        async fn get(&self) -> Result<CtfConfig, RepoError> {
            Ok(self.inner.lock().await.clone().unwrap_or_default())
        }
        async fn set(&self, config: CtfConfig) -> Result<(), RepoError> {
            *self.inner.lock().await = Some(config);
            Ok(())
        }
    }

    #[tokio::test]
    async fn get_defaults_then_persists_update() {
        let svc = ConfigService {
            config_repo: MemConfig::default(),
        };
        let cfg = svc.get().await.unwrap();
        assert!(cfg.registration_open);
        assert_eq!(cfg.freeze_time, None);
        let mut updated = cfg;
        updated.freeze_time = Some(1_700_000_000);
        updated.registration_open = false;
        svc.update(updated).await.unwrap();
        let after = svc.get().await.unwrap();
        assert_eq!(after.freeze_time, Some(1_700_000_000));
        assert!(!after.registration_open);
    }

    #[test]
    fn running_and_frozen_windows() {
        let cfg = CtfConfig {
            start_time: Some(100),
            end_time: Some(200),
            freeze_time: Some(180),
            ..Default::default()
        };
        assert!(!cfg.is_running(50));
        assert!(cfg.is_running(150));
        assert!(!cfg.is_running(250));
        assert!(!cfg.is_frozen(150));
        assert!(cfg.is_frozen(180));
    }
}

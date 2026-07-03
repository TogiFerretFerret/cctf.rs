use super::RepoError;
use crate::libs::types::accounts::{Account, AccountId, AccountName};
use crate::libs::types::challenges::Challenge;
use crate::libs::types::config::CtfConfig;
use crate::libs::types::solves::{HintUnlock, Submission};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use async_trait::async_trait;

#[async_trait]
pub trait AccountRepo: Send + Sync {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError>;
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError>;
    async fn save(&self, account: Account) -> Result<(), RepoError>;
    async fn update(&self, account: Account) -> Result<(), RepoError>;
}

#[async_trait]
pub trait TeamRepo: Send + Sync {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError>;
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError>;
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError>;
    async fn save(&self, team: Team) -> Result<(), RepoError>;
    async fn update(&self, team: Team) -> Result<(), RepoError>;
    async fn find_all(&self) -> Result<Vec<Team>, RepoError>;
}

#[async_trait]
pub trait InstanceRepo: Send + Sync {
    async fn find_active_flag(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Option<String>, RepoError>;

    async fn get_instance_ip(&self, instance_id: &str) -> Result<Option<String>, RepoError>;
}

#[async_trait]
pub trait ChallengeRepo: InstanceRepo + Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError>;
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError>;
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError>;
    async fn update(&self, challenge: Challenge) -> Result<(), RepoError>;
    async fn delete(&self, id: &str, delete_solves: bool) -> Result<(), RepoError>;
}

#[async_trait]
pub trait SubmissionRepo: Send + Sync {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError>;
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError>;
    async fn save(&self, submission: Submission) -> Result<(), RepoError>;
}

#[async_trait]
pub trait HintUnlockRepo: Send + Sync {
    async fn find_all(&self) -> Result<Vec<HintUnlock>, RepoError>;
    async fn find_for(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Vec<HintUnlock>, RepoError>;
    async fn save(&self, unlock: HintUnlock) -> Result<(), RepoError>;
}

#[async_trait]
pub trait ConfigRepo: Send + Sync {
    async fn get(&self) -> Result<CtfConfig, RepoError>;
    async fn set(&self, config: CtfConfig) -> Result<(), RepoError>;
}

#[async_trait]
impl<T: AccountRepo + ?Sized> AccountRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &AccountId) -> Result<Option<Account>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_by_username(&self, name: &AccountName) -> Result<Option<Account>, RepoError> {
        (**self).find_by_username(name).await
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Account>, RepoError> {
        (**self).find_by_ctftime_id(ctftime_id).await
    }
    async fn save(&self, account: Account) -> Result<(), RepoError> {
        (**self).save(account).await
    }
    async fn update(&self, account: Account) -> Result<(), RepoError> {
        (**self).update(account).await
    }
}

#[async_trait]
impl<T: TeamRepo + ?Sized> TeamRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &TeamId) -> Result<Option<Team>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_by_name(&self, name: &TeamName) -> Result<Option<Team>, RepoError> {
        (**self).find_by_name(name).await
    }
    async fn find_by_ctftime_id(&self, ctftime_id: u32) -> Result<Option<Team>, RepoError> {
        (**self).find_by_ctftime_id(ctftime_id).await
    }
    async fn save(&self, team: Team) -> Result<(), RepoError> {
        (**self).save(team).await
    }
    async fn update(&self, team: Team) -> Result<(), RepoError> {
        (**self).update(team).await
    }
    async fn find_all(&self) -> Result<Vec<Team>, RepoError> {
        (**self).find_all().await
    }
}

#[async_trait]
impl<T: InstanceRepo + ?Sized> InstanceRepo for std::sync::Arc<T> {
    async fn find_active_flag(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Option<String>, RepoError> {
        (**self)
            .find_active_flag(challenge_id, team_id, account_id)
            .await
    }

    async fn get_instance_ip(&self, instance_id: &str) -> Result<Option<String>, RepoError> {
        (**self).get_instance_ip(instance_id).await
    }
}

#[async_trait]
impl<T: ChallengeRepo + ?Sized> ChallengeRepo for std::sync::Arc<T> {
    async fn find_by_id(&self, id: &str) -> Result<Option<Challenge>, RepoError> {
        (**self).find_by_id(id).await
    }
    async fn find_all(&self) -> Result<Vec<Challenge>, RepoError> {
        (**self).find_all().await
    }
    async fn save(&self, challenge: Challenge) -> Result<(), RepoError> {
        (**self).save(challenge).await
    }
    async fn update(&self, challenge: Challenge) -> Result<(), RepoError> {
        (**self).update(challenge).await
    }
    async fn delete(&self, id: &str, delete_solves: bool) -> Result<(), RepoError> {
        (**self).delete(id, delete_solves).await
    }
}

#[async_trait]
impl<T: SubmissionRepo + ?Sized> SubmissionRepo for std::sync::Arc<T> {
    async fn find_all(&self) -> Result<Vec<Submission>, RepoError> {
        (**self).find_all().await
    }
    async fn find_by_team(&self, team_id: &TeamId) -> Result<Vec<Submission>, RepoError> {
        (**self).find_by_team(team_id).await
    }
    async fn save(&self, submission: Submission) -> Result<(), RepoError> {
        (**self).save(submission).await
    }
}

#[async_trait]
impl<T: HintUnlockRepo + ?Sized> HintUnlockRepo for std::sync::Arc<T> {
    async fn find_all(&self) -> Result<Vec<HintUnlock>, RepoError> {
        (**self).find_all().await
    }
    async fn find_for(
        &self,
        challenge_id: &str,
        team_id: Option<&TeamId>,
        account_id: &AccountId,
    ) -> Result<Vec<HintUnlock>, RepoError> {
        (**self).find_for(challenge_id, team_id, account_id).await
    }
    async fn save(&self, unlock: HintUnlock) -> Result<(), RepoError> {
        (**self).save(unlock).await
    }
}

#[async_trait]
impl<T: ConfigRepo + ?Sized> ConfigRepo for std::sync::Arc<T> {
    async fn get(&self) -> Result<CtfConfig, RepoError> {
        (**self).get().await
    }
    async fn set(&self, config: CtfConfig) -> Result<(), RepoError> {
        (**self).set(config).await
    }
}

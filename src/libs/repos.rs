use crate::libs::types::accounts::{Account, AccountId, AccountName};
use crate::libs::types::teams::{Team, TeamId, TeamName};
use crate::libs::types::challenges::Challenge;
use crate::libs::types::solves::Solve;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, thiserror::Error)]
pub enum RepoEr

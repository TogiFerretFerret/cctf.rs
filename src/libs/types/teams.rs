use crate::libs::types::accounts::AccountId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TeamId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: TeamId,
    pub name: TeamName,
    pub ctftime_id: Option<u32>,
    pub invite_code: Option<String>,
    pub captain_id: AccountId,
    pub member_ids: Vec<AccountId>,
    pub bracket: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub create_at: i64,
}

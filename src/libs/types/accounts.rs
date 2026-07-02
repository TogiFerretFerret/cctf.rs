use crate::libs::types::teams::TeamId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountEmail(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountRole {
    Admin,
    Player,
    Spectator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: AccountName,
    pub email: Option<AccountEmail>,
    #[serde(skip_serializing, default)]
    pub password_hash: Option<String>,
    pub role: AccountRole,
    pub team_id: Option<TeamId>,
    pub ctftime_id: Option<u32>,
    pub fields: HashMap<String, serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeTeamInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeUserProfile {
    pub id: u32,
    pub username: String,
    pub email: Option<String>,
    pub team: Option<CtfTimeTeamInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeTeamRating {
    pub rating_place: Option<u32>,
    pub rating_points: Option<f64>,
    pub organizers_points: Option<f64>,
    pub year: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeTeamMember {
    pub id: u32,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeFulLTeamProfile {
    pub id: u32,
    pub name: String,
    pub primary_alias: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub logo: Option<String>,
    pub country_code: Option<String>,
    pub rating: Option<CtfTimeTeamRating>,
    pub members: Vec<CtfTimeTeamMember>,
    pub website: Option<String>,
}

use serde::{Serialize, Deserialize};
use crate::libs::types::accounts::AccountId;
use crate::libs::types::teams::TeamId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SubmissionId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Submission {
    pub id: SubmissionId,
    pub challenge_id: String,
    pub team_id: Option<TeamId>,
    pub account_id: AccountId,
    pub points: u32,
    pub provided_flag: String,
    pub solved_at: i64,
}

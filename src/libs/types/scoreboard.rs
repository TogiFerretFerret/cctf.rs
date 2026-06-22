use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use crate::libs::types::teams::TeamId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreboardEntry {
    pub team_id: TeamId,
    pub team_name: String,
    pub points: f64,
    pub last_solve_time: Option<i64>,
    pub solves: Vec<String>, // list of challenge ids
    pub rank: u32,
}

// ctftime scoreboard export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeTaskStats {
    pub points: u32,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeStandingsEntry {
    pub pos: Option<u32>,
    pub team: String,
    pub score: f64,
    #[serde(rename = "taskStats")]
    pub task_stats: HashMap<String, CtfTimeTaskStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfTimeScoreboardExport {
    pub tasks: Vec<String>,
    pub standings: Vec<CtfTimeStandingsEntry>,
}

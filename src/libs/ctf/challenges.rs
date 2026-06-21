use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub title: String,
    pub description: ChallengeDescription,
    pub category: ChallengeCategory,
    pub points: ChallengePoints,
    pub flag: FlagValidator,
    pub author: ChallengeAuthor,
    pub hints: Vec<ChallengeHint>,
    pub files: Vec<ChallengeFile>,
    pub tags: Vec<ChallengeTag>,
    pub requirements: Vec<ChallengeRequirement>,
}

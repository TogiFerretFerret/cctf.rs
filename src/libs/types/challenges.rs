use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeTitle(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeDescription(pub HtmlString);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeCategory(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeTag(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScoringMode {
    PointValue,
    PointAttribution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengePoints {
    pub mode: ScoringMode,
    pub equation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeAuthor {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeHint {
    pub content: HtmlString,
    pub cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeFile {
    pub name: String,
    pub url: String,
    pub checksum_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChallengeRequirement {
    Solve(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub title: ChallengeTitle,
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

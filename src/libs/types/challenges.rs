use crate::libs::types::flags::{FlagValidator, sandboxed_engine};
use crate::libs::types::htmlstring::HtmlString;
use serde::{Deserialize, Serialize};

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
    DynamicDecay {
        initial: u32,
        minimum: u32,
        decay: u32,
    },
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
pub enum HintCost {
    Fixed(u32),
    Script(String),
}

impl HintCost {
    /// Resolve the cost. `Script` runs in the sandboxes rhai engine with `solves`
    /// (challenge solve count) and `now` (unix seconds) in scope, and must return
    /// an int; the result is clamped to `>= 0`. A broken script costs 0 (logged).
    ///
    /// ```
    /// use cctf_rs::libs::types::challenges::HintCost;
    ///
    /// assert_eq!(HintCost::Fixed(50).evaluate(3, 0), 50);
    /// assert_eq!(HintCost::Script("solves * 10".to_string()).evaluate(3, 0), 30);
    /// assert_eq!(HintCost::Script("solves>5?0:100".to_string()).evaluate(9, 0), 0);
    /// ```
    pub fn evaluate(&self, solves: u32, now: i64) -> u32 {
        match self {
            HintCost::Fixed(cost) => *cost,
            HintCost::Script(script) => {
                let engine = sandboxed_engine();
                let mut scope = rhai::Scope::new();
                scope.push("solves", solves as i64);
                scope.push("now", now);
                match engine.eval_with_scope::<i64>(&mut scope, script) {
                    Ok(cost) => cost.max(0) as u32,
                    Err(e) => {
                        eprintln!("hint cost rhai!: {:?}", e);
                        0
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeHint {
    pub content: HtmlString,
    pub cost: HintCost,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ChallengeDeployment {
    #[default]
    None,
    Shared {
        url: String,
    },
    Instanced,
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
    #[serde(default)]
    pub team_consensus: bool,
    #[serde(default)]
    pub deployment: ChallengeDeployment,
    #[serde(default)]
    pub visibility: ChallengeVisibility,
    #[serde(default)]
    pub max_attempts: Option<MaxAttempts>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedReveal {
    pub description: bool,
    pub category: bool,
    pub points: bool,
    pub tags: bool,
    pub files: bool,
    pub hints: bool,
    pub requirements: bool,
    pub connection_info: bool,
}

impl Default for LockedReveal {
    fn default() -> Self {
        Self {
            description: false,
            category: true,
            points: true,
            tags: true,
            files: false,
            hints: false,
            requirements: true,
            connection_info: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ChallengeVisibility {
    #[default]
    Visible,
    Hidden,
    Locked(LockedReveal),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AttemptCountMode {
    #[default]
    All,
    Unique,
    IncorrectOnly,
    UniqueIncorrect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaxAttempts {
    pub limit: u32,
    #[serde(default)]
    pub mode: AttemptCountMode,
}

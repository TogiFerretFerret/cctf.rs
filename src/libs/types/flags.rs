use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlagValidator {
    Static(String),
    Regex(String),
    Script(String), // Python or lua, we shall see
    Instanced,
}

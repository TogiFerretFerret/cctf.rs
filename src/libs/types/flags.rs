use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartialFlag {
    pub id: String,
    pub validator: FlagValidator,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlagValidator {
    Static(String),
    Regex(String),
    Script(String),
    Instanced,
    Multi(Vec<PartialFlag>),
}

impl FlagValidator {
    pub fn is_match(&self, submitted_flag: &str, active_instanced_flag: Option<&str>) -> bool {
        match self {
            FlagValidator::Static(flag) => flag.trim() == submitted_flag.trim(),
            FlagValidator::Regex(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(submitted_flag.trim())
                } else {
                    false
                }
            }
            FlagValidator::Instanced => {
                if let Some(active_flag) = active_instanced_flag {
                    active_flag.trim() == submitted_flag.trim()
                } else {
                    false
                }
            }
            FlagValidator::Script(script_content) => {
                let engine = rhai::Engine::new();
                let mut scope = rhai::Scope::new();
                scope.push("flag", submitted_flag.trim().to_string());
                match engine.eval_with_scope::<bool>(&mut scope, &script_content) {
                    Ok(is_correct) => is_correct,
                    Err(e) => {
                        eprintln!("rhai!: {:?}", e);
                        false
                    }
                }
            },
            FlagValidator::Multi(flags) => flags
                .iter()
                .any(|f| f.validator.is_match(submitted_flag, active_instanced_flag)),
        }
    }

    pub fn find_matching_partial(
        &self,
        submitted_flag: &str,
        active_instanced_flag: Option<&str>,
    ) -> Option<&PartialFlag> {
        match self {
            FlagValidator::Multi(flags) => flags
                .iter()
                .find(|f| f.validator.is_match(submitted_flag, active_instanced_flag)),
            _ => None,
        }
    }
}

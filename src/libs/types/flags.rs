use serde::{Deserialize, Serialize};

/// Constant-time string equality so flag checks don't leak how many leading
/// characters matched via response timing.
fn ct_eq(a: &str, b: &str) -> bool {
    constant_time_eq::constant_time_eq(a.as_bytes(), b.as_bytes())
}

/// A rhai engine with hard resource caps. Without these, a single challenge
/// script with `loop {}` (or just an expensive one) runs on every flag
/// submission and pins a worker thread forever. Scripts are admin-defined, but
/// this is defense-in-depth against a buggy/hostile script.
fn sandboxed_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_string_size(16 * 1024);
    engine.set_max_array_size(1024);
    engine.set_max_map_size(1024);
    engine
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartialFlag {
    pub id: String,
    pub validator: FlagValidator,
    pub weight: f64,
}

/// How a challenge's flag is verified. `Static`/`Instanced` use constant-time
/// comparison; `Regex`/`Script` evaluate a pattern or sandboxed rhai; `Multi`
/// holds weighted [`PartialFlag`]s for partial scoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlagValidator {
    Static(String),
    Regex(String),
    Script(String),
    Instanced,
    Multi(Vec<PartialFlag>),
}

impl FlagValidator {
    /// Check a submitted flag. `active_instanced_flag` is the team's live
    /// per-instance flag, used only by [`FlagValidator::Instanced`]. Surrounding
    /// whitespace is trimmed and comparisons are constant-time.
    ///
    /// ```
    /// use cctf_rs::libs::types::flags::FlagValidator;
    ///
    /// let flag = FlagValidator::Static("cctf{h3llo}".to_string());
    /// assert!(flag.is_match("cctf{h3llo}", None));
    /// assert!(flag.is_match("  cctf{h3llo}  ", None)); // trimmed
    /// assert!(!flag.is_match("cctf{nope}", None));
    ///
    /// let re = FlagValidator::Regex(r"^cctf\{[0-9a-f]{4}\}$".to_string());
    /// assert!(re.is_match("cctf{beef}", None));
    /// assert!(!re.is_match("cctf{zzzz}", None));
    ///
    /// // Instanced flags are checked against the team's live flag:
    /// let inst = FlagValidator::Instanced;
    /// assert!(inst.is_match("cctf{team}", Some("cctf{team}")));
    /// assert!(!inst.is_match("cctf{team}", None));
    /// ```
    pub fn is_match(&self, submitted_flag: &str, active_instanced_flag: Option<&str>) -> bool {
        match self {
            FlagValidator::Static(flag) => ct_eq(flag.trim(), submitted_flag.trim()),
            FlagValidator::Regex(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(submitted_flag.trim())
                } else {
                    false
                }
            }
            FlagValidator::Instanced => {
                if let Some(active_flag) = active_instanced_flag {
                    ct_eq(active_flag.trim(), submitted_flag.trim())
                } else {
                    false
                }
            }
            FlagValidator::Script(script_content) => {
                let engine = sandboxed_engine();
                let mut scope = rhai::Scope::new();
                scope.push("flag", submitted_flag.trim().to_string());
                match engine.eval_with_scope::<bool>(&mut scope, script_content) {
                    Ok(is_correct) => is_correct,
                    Err(e) => {
                        eprintln!("rhai!: {:?}", e);
                        false
                    }
                }
            }
            FlagValidator::Multi(flags) => flags
                .iter()
                .any(|f| f.validator.is_match(submitted_flag, active_instanced_flag)),
        }
    }

    /// For a [`FlagValidator::Multi`], return the first partial flag the
    /// submission satisfies (drives weighted partial scoring). Returns `None`
    /// for non-multi validators or when nothing matches.
    ///
    /// ```
    /// use cctf_rs::libs::types::flags::{FlagValidator, PartialFlag};
    ///
    /// let multi = FlagValidator::Multi(vec![
    ///     PartialFlag { id: "a".into(), validator: FlagValidator::Static("cctf{a}".into()), weight: 0.5 },
    ///     PartialFlag { id: "b".into(), validator: FlagValidator::Static("cctf{b}".into()), weight: 0.5 },
    /// ]);
    ///
    /// assert_eq!(multi.find_matching_partial("cctf{b}", None).unwrap().id, "b");
    /// assert!(multi.find_matching_partial("cctf{z}", None).is_none());
    /// ```
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

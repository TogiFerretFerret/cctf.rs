use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

/// Singleton event configuration: the schedule (start / stop / freeze) plus the
/// registration and verification toggles. Stored as one JSONB row and read
/// wherever the platform needs to know "is the CTF live / frozen right now?".
///
/// ```
/// use cctf_rs::libs::types::config::CtfConfig;
///
/// let cfg = CtfConfig {
///     start_time: Some(1_000),
///     end_time: Some(2_000),
///     freeze_time: Some(1_900),
///     ..Default::default()
/// };
///
/// assert!(!cfg.is_running(500));    // before start
/// assert!(cfg.is_running(1_500));   // during the event
/// assert!(!cfg.is_running(2_500));  // after end
///
/// assert!(!cfg.is_frozen(1_500));   // scoreboard still live
/// assert!(cfg.is_frozen(1_900));    // frozen from here on
///
/// // Defaults: registration open, no schedule set, so it's always "running".
/// let open = CtfConfig::default();
/// assert!(open.registration_open);
/// assert!(open.is_running(0));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfConfig {
    pub ctf_name: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub freeze_time: Option<i64>,
    pub registration_open: bool,
    pub require_email_verification: bool,
    #[serde(default)]
    pub sort_by_accuracy: bool,
    #[serde(default = "default_true")]
    pub hints_deduct_score: bool,
}

impl Default for CtfConfig {
    fn default() -> Self {
        Self {
            ctf_name: "cctf.rs".to_string(),
            start_time: None,
            end_time: None,
            freeze_time: None,
            registration_open: true,
            require_email_verification: false,
            sort_by_accuracy: false,
            hints_deduct_score: true,
        }
    }
}

impl CtfConfig {
    /// True when `now` (unix seconds) is in `[start_time, end_time)`. An unset
    /// bound is open: no start means "already started", no end means "never ends".
    pub fn is_running(&self, now: i64) -> bool {
        self.start_time.is_none_or(|s| now >= s) && self.end_time.is_none_or(|e| now < e)
    }

    /// True once `now` has reached `freeze_time` (scoreboard should stop moving).
    /// Always false when no freeze is scheduled.
    pub fn is_frozen(&self, now: i64) -> bool {
        self.freeze_time.is_some_and(|f| now >= f)
    }
}

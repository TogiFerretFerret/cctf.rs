use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtfConfig {
    pub ctf_name: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub freeze_time: Option<i64>,
    pub registration_open: bool,
    pub require_email_verification: bool,
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
        }
    }
}

impl CtfConfig {
    pub fn is_running(&self, now: i64) -> bool {
        self.start_time.is_none_or(|s| now >= s) && self.end_time.is_none_or(|e| now < e)
    }
    pub fn is_frozen(&self, now: i64) -> bool {
        self.freeze_time.is_some_and(|f| now >= f)
    }
}

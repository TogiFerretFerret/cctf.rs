use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFile {
    pub id: String,
    pub name: String,
    pub checksum_sha256: String,
    pub size: u64,
    pub content_type: String,
    pub uploaded_at: i64,
}



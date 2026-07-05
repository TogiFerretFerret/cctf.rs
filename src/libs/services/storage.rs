use async_trait::async_trait;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug)]
pub enum StorageError {
    Io(String),
    NotFound,
    InvalidId,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "storage io error: {e}"),
            StorageError::NotFound => write!(f, "file not found"),
            StorageError::InvalidId => write!(f, "invalid file id"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Reject anything that could escape the storage root or be read as a flag.
/// Server-minted ids are UUIDs, so this only ever fails on tampering
fn validate_id(id: &str) -> Result<(), StorageError> {
    if id.is_empty()
        || id.len() > 128
        || id.starts_with('-')
        || id.contains("..")
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(StorageError::InvalidId);
    }
    Ok(())
}

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn store(&self, id: &str, bytes: &[u8]) -> Result<(), StorageError>;
    async fn retrieve(&self, id: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, id: &str) -> Result<(), StorageError>;
}

pub struct LocalFileStorage {
    pub dir: PathBuf,
}

impl LocalFileStorage {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    async fn store(&self, id: &str, bytes: &[u8]) -> Result<(), StorageError> {
        validate_id(id)?;
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        tokio::fs::write(self.dir.join(id), bytes)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))
    }
    async fn retrieve(&self, id: &str) -> Result<Vec<u8>, StorageError> {
        validate_id(id)?;
        tokio::fs::read(self.dir.join(id)).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound
            } else {
                StorageError::Io(e.to_string())
            }
        })
    }
    async fn delete(&self, id: &str) -> Result<(), StorageError> {
        validate_id(id)?;
        match tokio::fs::remove_file(self.dir.join(id)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e.to_string())),
        }
    }
}

pub struct RcloneFileStorage {
    pub remote: String,
    pub path: String,
    pub extra_args: Vec<String>,
}

impl RcloneFileStorage {
    fn target(&self, id: &str) -> String {
        if self.path.is_empty() {
            format!("{}:{}", self.remote, id)
        } else {
            format!("{}:{}/{}", self.remote, self.path.trim_end_matches('/'), id)
        }
    }
}

#[async_trait]
impl FileStorage for RcloneFileStorage {
    async fn store(&self, id: &str, bytes: &[u8]) -> Result<(), StorageError> {
        validate_id(id)?;
        let mut child = Command::new("rclone")
            .args(&self.extra_args)
            .arg("rcat")
            .arg("--")
            .arg(self.target(id))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| StorageError::Io(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(bytes)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
            stdin
                .shutdown()
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        if !out.status.success() {
            return Err(StorageError::Io(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        Ok(())
    }
    async fn retrieve(&self, id: &str) -> Result<Vec<u8>, StorageError> {
        validate_id(id)?;
        let out = Command::new("rclone")
            .args(&self.extra_args)
            .arg("cat")
            .arg("--")
            .arg(self.target(id))
            .output()
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        if !out.status.success() {
            return Err(StorageError::NotFound);
        }
        Ok(out.stdout)
    }
    async fn delete(&self, id: &str) -> Result<(), StorageError> {
        validate_id(id)?;
        let _ = Command::new("rclone")
            .args(&self.extra_args)
            .arg("deletefile")
            .arg("--")
            .arg(self.target(id))
            .output()
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        Ok(())
    }
}

use crate::libs::types::config::StorageBackend;
use std::sync::Arc;

/// Build the configured storage backend. Add a match arm per new backend
/// (e.g. a future `RcloneLibStorage` using the librclone crate).
pub fn build_storage(backend: &StorageBackend) -> Arc<dyn FileStorage> {
    match backend {
        StorageBackend::Local { dir } => Arc::new(LocalFileStorage::new(dir.clone())),
        StorageBackend::Rclone { remote, path } => {
            let mut extra = Vec::new();
            if let Ok(id) = std::env::var("RCLONE_DRIVE_ROOT_FOLDER_ID")
                && !id.is_empty()
            {
                extra.push("--drive-root-folder-id".to_string());
                extra.push(id);
            }
            Arc::new(RcloneFileStorage {
                remote: remote.clone(),
                path: path.clone(),
                extra_args: extra,
            })
        }
    }
}

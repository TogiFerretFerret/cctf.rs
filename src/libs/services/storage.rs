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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cctf-storage-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn validate_id_accepts_uuids_and_plain_names() {
        assert!(validate_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_id("file_1.bin").is_ok());
        assert!(validate_id("AbC.123-x_y").is_ok());
    }

    #[test]
    fn validate_id_rejects_traversal_flags_and_separators() {
        assert!(validate_id("").is_err());
        assert!(validate_id("../etc/passwd").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("-rf").is_err());
        assert!(validate_id("a b").is_err());
        assert!(validate_id("name;rm").is_err());
        assert!(validate_id(&"x".repeat(200)).is_err());
    }

    #[tokio::test]
    async fn local_store_retrieve_delete_roundtrip() {
        let dir = temp_dir();
        let storage = LocalFileStorage::new(&dir);
        storage.store("abc123", b"hello cctf").await.unwrap();
        assert_eq!(storage.retrieve("abc123").await.unwrap(), b"hello cctf");
        storage.delete("abc123").await.unwrap();
        assert!(matches!(
            storage.retrieve("abc123").await,
            Err(StorageError::NotFound)
        ));
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn local_missing_read_is_not_found_delete_is_ok() {
        let dir = temp_dir();
        let storage = LocalFileStorage::new(&dir);
        assert!(matches!(
            storage.retrieve("nope").await,
            Err(StorageError::NotFound)
        ));
        assert!(storage.delete("ghost").await.is_ok());
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn local_rejects_unsafe_ids_before_touching_fs() {
        let storage = LocalFileStorage::new(temp_dir());
        assert!(matches!(
            storage.store("../evil", b"x").await,
            Err(StorageError::InvalidId)
        ));
        assert!(matches!(
            storage.retrieve("a/b").await,
            Err(StorageError::InvalidId)
        ));
    }

    #[tokio::test]
    #[ignore = "requires rclone + RCLONE_TEST_REMOTE (remote:path); run with --ignored"]
    async fn rclone_store_retrieve_delete_roundtrip() {
        let spec = std::env::var("RCLONE_TEST_REMOTE")
            .expect("set RCLONE_TEST_REMOTE like 'gdedit:cctf-test'");
        let (remote, path) = spec.split_once(':').expect("expected remote:path");
        let mut extra = Vec::new();
        if let Ok(id) = std::env::var("RCLONE_DRIVE_ROOT_FOLDER_ID")
            && !id.is_empty()
        {
            extra.push("--drive-root-folder-id".to_string());
            extra.push(id);
        }
        let storage = RcloneFileStorage {
            remote: remote.to_string(),
            path: path.to_string(),
            extra_args: extra,
        };
        let id = format!("test-{}", uuid::Uuid::new_v4());
        storage.store(&id, b"rclone roundtrip").await.unwrap();
        assert_eq!(storage.retrieve(&id).await.unwrap(), b"rclone roundtrip");
        storage.delete(&id).await.unwrap();
        assert!(matches!(
            storage.retrieve(&id).await,
            Err(StorageError::NotFound)
        ));
    }
}

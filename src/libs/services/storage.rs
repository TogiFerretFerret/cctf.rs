use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StorageError {
    Io(String),
    NotFound,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Io(err) => write!(f, "storage io error: {e}"),
            StorageError::NotFound => write!(f, "file not found"),
        }
    }
}

impl std::error::Error for StorageError {}

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
    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }
}

#[async_trait]
impl FileStorage for LocalFileStore {
    async fn store(&self, id: &str, bytes: &[u8]) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))?;
        tokio::fs::write(self.path(id), bytes)
            .await
            .map_err(|e| StorageError::Io(e.to_string()))
    }
    async fn retrieve(&self, id: &str) -> Result<Vec<u8>, StorageError> {
        tokio::fs::read(self.path(id)).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound
            } else {
                StorageError::Io(e.to_string())
            }
        })
    }
    async fn delete(&self, id: &str) -> Result<(), StorageError> {
        match tokio::fs::remove_file(self.path(id)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e.to_string())),
        }
    }
}

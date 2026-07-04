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



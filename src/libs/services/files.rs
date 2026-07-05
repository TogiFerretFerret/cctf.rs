use super::ServiceError;
use super::storage::{FileStorage, StorageError};
use crate::libs::repos::{FileRepo, RepoError};
use crate::libs::types::challenges::ChallengeFile;
use crate::libs::types::files::StoredFile;
use sha2::{Digest, Sha256};
use std::sync::Arc;

impl From<StorageError> for ServiceError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound => {
                ServiceError::InvalidRequest("ctf-file-not-found".to_string())
            }
            StorageError::InvalidId => {
                ServiceError::InvalidRequest("ctf-file-invalid-id".to_string())
            }
            StorageError::Io(msg) => ServiceError::Repo(RepoError::Internal(msg)),
        }
    }
}

pub struct FileService {
    pub storage: Arc<dyn FileStorage>,
    pub repo: Arc<dyn FileRepo>,
    pub max_bytes: u64,
}

impl FileService {
    pub async fn upload(
        &self,
        name: &str,
        content_type: &str,
        bytes: &[u8],
        now: i64,
    ) -> Result<ChallengeFile, ServiceError> {
        if bytes.len() as u64 > self.max_bytes {
            return Err(ServiceError::InvalidRequest(
                "ctf-file-too-large".to_string(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let checksum: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.storage.store(&id, bytes).await?;
        let stored = StoredFile {
            id: id.clone(),
            name: name.to_string(),
            checksum_sha256: checksum.clone(),
            size: bytes.len() as u64,
            content_type: content_type.to_string(),
            uploaded_at: now,
        };
        self.repo.save(stored).await?;
        Ok(ChallengeFile {
            name: name.to_string(),
            url: format!("/api/v1/files/{id}"),
            checksum_sha256: Some(checksum),
        })
    }

    pub async fn download(&self, id: &str) -> Result<(StoredFile, Vec<u8>), ServiceError> {
        let meta = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ServiceError::InvalidRequest("ctf-file-not-found".to_string()))?;
        let bytes = self.storage.retrieve(id).await?;
        Ok((meta, bytes))
    }

    pub async fn delete(&self, id: &str) -> Result<(), ServiceError> {
        self.storage.delete(id).await?;
        self.repo.delete(id).await?;
        Ok(())
    }
}

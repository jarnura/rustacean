use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::{BlobError, BlobRef, store::BlobStore};

/// Filesystem-backed blob store.
///
/// Layout: `<base_path>/<tenant_id>/<sha256>`
///
/// Tenant isolation is enforced by the path prefix. On a GET miss, the store
/// scans sibling tenant directories for the same SHA-256 and returns
/// [`BlobError::TenantMismatch`] if found under a different tenant.
pub struct FilesystemStore {
    base_path: PathBuf,
}

impl FilesystemStore {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self { base_path: base_path.into() }
    }

    /// Reads `RB_BLOB_BASE_PATH` from the environment (default: `/var/lib/rustbrain/blobs`).
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::Io`] if the base directory cannot be created.
    pub fn from_env() -> Result<Self, BlobError> {
        let path = std::env::var("RB_BLOB_BASE_PATH")
            .unwrap_or_else(|_| "/var/lib/rustbrain/blobs".to_string());
        Ok(Self::new(path))
    }

    fn blob_path(&self, blob_ref: &BlobRef) -> PathBuf {
        self.base_path
            .join(blob_ref.tenant_id.to_string())
            .join(&blob_ref.sha256)
    }

    /// Returns `true` when `sha256` exists under any tenant directory OTHER
    /// than the one in `blob_ref`. Used to distinguish `TenantMismatch` from
    /// `NotFound`.
    async fn sha256_exists_other_tenant(&self, blob_ref: &BlobRef) -> bool {
        let Ok(mut dir) = tokio::fs::read_dir(&self.base_path).await else {
            return false;
        };
        let owner_str = blob_ref.tenant_id.to_string();
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name();
            if name.to_string_lossy() == owner_str {
                continue;
            }
            if tokio::fs::metadata(entry.path().join(&blob_ref.sha256))
                .await
                .is_ok()
            {
                return true;
            }
        }
        false
    }
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[async_trait]
impl BlobStore for FilesystemStore {
    #[instrument(skip(self, data), fields(tenant=%blob_ref.tenant_id, sha256=%blob_ref.sha256))]
    async fn put(&self, blob_ref: &BlobRef, data: Bytes) -> Result<(), BlobError> {
        let actual_size = data.len() as u64;
        if actual_size != blob_ref.size {
            return Err(BlobError::SizeMismatch {
                expected: blob_ref.size,
                got: actual_size,
            });
        }
        let actual_sha256 = compute_sha256(&data);
        if actual_sha256 != blob_ref.sha256 {
            return Err(BlobError::Sha256Mismatch {
                expected: blob_ref.sha256.clone(),
                got: actual_sha256,
            });
        }
        let path = self.blob_path(blob_ref);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant=%blob_ref.tenant_id, sha256=%blob_ref.sha256))]
    async fn get(&self, blob_ref: &BlobRef) -> Result<Bytes, BlobError> {
        let path = self.blob_path(blob_ref);
        match tokio::fs::read(&path).await {
            Ok(data) => Ok(Bytes::from(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.sha256_exists_other_tenant(blob_ref).await {
                    Err(BlobError::TenantMismatch)
                } else {
                    Err(BlobError::NotFound {
                        tenant_id: blob_ref.tenant_id,
                        sha256: blob_ref.sha256.clone(),
                    })
                }
            }
            Err(e) => Err(BlobError::Io(e)),
        }
    }

    #[instrument(skip(self), fields(tenant=%blob_ref.tenant_id, sha256=%blob_ref.sha256))]
    async fn delete(&self, blob_ref: &BlobRef) -> Result<(), BlobError> {
        let path = self.blob_path(blob_ref);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.sha256_exists_other_tenant(blob_ref).await {
                    Err(BlobError::TenantMismatch)
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(BlobError::Io(e)),
        }
    }

    async fn exists(&self, blob_ref: &BlobRef) -> Result<bool, BlobError> {
        Ok(tokio::fs::metadata(self.blob_path(blob_ref)).await.is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_blob_ref(tenant_id: Uuid, data: &[u8]) -> BlobRef {
        let sha256 = compute_sha256(data);
        BlobRef::new(tenant_id, sha256, "application/octet-stream", data.len() as u64)
    }

    #[tokio::test]
    async fn fs_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FilesystemStore::new(dir.path());
        let tenant = Uuid::new_v4();
        let data = b"hello, rustbrain blob store";
        let blob_ref = make_blob_ref(tenant, data);

        store.put(&blob_ref, Bytes::from_static(data)).await.expect("put");
        assert!(store.exists(&blob_ref).await.expect("exists"));

        let retrieved = store.get(&blob_ref).await.expect("get");
        assert_eq!(retrieved.as_ref(), data);
    }

    #[tokio::test]
    async fn tenant_isolation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FilesystemStore::new(dir.path());

        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let data = b"shared content different owners";
        let ref_a = make_blob_ref(tenant_a, data);

        // Tenant A stores the blob.
        store.put(&ref_a, Bytes::from_static(data)).await.expect("put as tenant_a");

        // Tenant B tries to read the same SHA-256 — must be rejected.
        let ref_b = BlobRef::new(tenant_b, ref_a.sha256.clone(), "application/octet-stream", ref_a.size);
        let err = store.get(&ref_b).await.expect_err("cross-tenant read must fail");
        assert!(
            matches!(err, BlobError::TenantMismatch),
            "expected TenantMismatch, got {err:?}"
        );
    }

    #[tokio::test]
    async fn delete_removes_blob() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FilesystemStore::new(dir.path());
        let tenant = Uuid::new_v4();
        let data = b"to be deleted";
        let blob_ref = make_blob_ref(tenant, data);

        store.put(&blob_ref, Bytes::from_static(data)).await.expect("put");
        store.delete(&blob_ref).await.expect("delete");
        assert!(!store.exists(&blob_ref).await.expect("exists after delete"));
    }

    #[tokio::test]
    async fn sha256_mismatch_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FilesystemStore::new(dir.path());
        let tenant = Uuid::new_v4();
        let data = b"real data";
        let mut blob_ref = make_blob_ref(tenant, data);
        blob_ref.sha256 = "0".repeat(64); // wrong hash

        let err = store
            .put(&blob_ref, Bytes::from_static(data))
            .await
            .expect_err("sha256 mismatch should fail");
        assert!(matches!(err, BlobError::Sha256Mismatch { .. }));
    }

    #[tokio::test]
    async fn large_blob() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FilesystemStore::new(dir.path());
        let tenant = Uuid::new_v4();

        // 100 MiB
        let size: usize = 100 * 1024 * 1024;
        let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let blob_ref = make_blob_ref(tenant, &data);

        store.put(&blob_ref, Bytes::from(data.clone())).await.expect("put 100 MiB");
        let retrieved = store.get(&blob_ref).await.expect("get 100 MiB");
        assert_eq!(retrieved.len(), size);
        assert_eq!(retrieved.as_ref(), data.as_slice());
    }
}

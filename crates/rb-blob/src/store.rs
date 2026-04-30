use async_trait::async_trait;
use bytes::Bytes;

use crate::{BlobError, BlobRef};

/// Abstraction over content-addressed blob storage backends.
///
/// Implementations must enforce tenant isolation: a blob stored by tenant A
/// must not be readable by tenant B even if the SHA-256 is identical.
#[async_trait]
pub trait BlobStore: Send + Sync + 'static {
    /// Store a blob. Validates that the data matches `blob_ref.sha256` and
    /// `blob_ref.size` before writing.
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::Sha256Mismatch`] or [`BlobError::SizeMismatch`]
    /// on integrity failures, or backend-specific errors on I/O failures.
    async fn put(&self, blob_ref: &BlobRef, data: Bytes) -> Result<(), BlobError>;

    /// Retrieve a blob by reference.
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::TenantMismatch`] when the SHA-256 exists but
    /// belongs to a different tenant. Returns [`BlobError::NotFound`] when
    /// the blob does not exist at all.
    async fn get(&self, blob_ref: &BlobRef) -> Result<Bytes, BlobError>;

    /// Delete a blob. No-op if the blob does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::TenantMismatch`] if the blob belongs to a
    /// different tenant.
    async fn delete(&self, blob_ref: &BlobRef) -> Result<(), BlobError>;

    /// Check whether a blob exists for the given tenant.
    ///
    /// Does **not** check whether the SHA-256 exists under a different tenant.
    async fn exists(&self, blob_ref: &BlobRef) -> Result<bool, BlobError>;
}

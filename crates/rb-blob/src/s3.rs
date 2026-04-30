//! S3-backed blob store. Requires the `s3` feature.
//!
//! Object layout: `<tenant_id>/<sha256>`
//!
//! A small owner-manifest object is written at `_manifest/<sha256>` so that
//! cross-tenant access can be detected without listing all tenant prefixes.

use async_trait::async_trait;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::head_object::HeadObjectError;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tracing::instrument;

use crate::{BlobError, BlobRef, store::BlobStore};

/// S3-backed blob store.
///
/// Configure via:
/// - `RB_BLOB_S3_BUCKET` — bucket name (default: `rb-blobs`)
/// - `RB_BLOB_S3_ENDPOINT` — override endpoint URL (e.g., for localstack)
/// - `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` — standard AWS env vars
pub struct S3Store {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3Store {
    /// Build from environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`BlobError::S3`] if the AWS config cannot be loaded.
    pub async fn from_env() -> Result<Self, BlobError> {
        let bucket = std::env::var("RB_BLOB_S3_BUCKET")
            .unwrap_or_else(|_| "rb-blobs".to_string());

        let sdk_config = aws_config::from_env().load().await;
        let mut s3_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

        if let Ok(endpoint) = std::env::var("RB_BLOB_S3_ENDPOINT") {
            s3_builder = s3_builder
                .endpoint_url(endpoint)
                .force_path_style(true);
        }

        let client = aws_sdk_s3::Client::from_conf(s3_builder.build());
        Ok(Self { client, bucket })
    }

    fn object_key(blob_ref: &BlobRef) -> String {
        format!("{}/{}", blob_ref.tenant_id, blob_ref.sha256)
    }

    fn manifest_key(sha256: &str) -> String {
        format!("_manifest/{sha256}")
    }
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[async_trait]
impl BlobStore for S3Store {
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

        let key = Self::object_key(blob_ref);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type(&blob_ref.content_type)
            .body(aws_sdk_s3::primitives::ByteStream::from(data))
            .send()
            .await
            .map_err(|e| BlobError::S3(e.to_string()))?;

        // Write owner manifest so cross-tenant mismatches can be detected.
        let manifest_key = Self::manifest_key(&blob_ref.sha256);
        let owner_bytes = blob_ref.tenant_id.to_string().into_bytes();
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(manifest_key)
            .body(aws_sdk_s3::primitives::ByteStream::from(Bytes::from(owner_bytes)))
            .send()
            .await
            .map_err(|e| BlobError::S3(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant=%blob_ref.tenant_id, sha256=%blob_ref.sha256))]
    async fn get(&self, blob_ref: &BlobRef) -> Result<Bytes, BlobError> {
        let key = Self::object_key(blob_ref);
        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(resp) => {
                let data = resp
                    .body
                    .collect()
                    .await
                    .map_err(|e| BlobError::S3(e.to_string()))?
                    .into_bytes();
                Ok(data)
            }
            Err(SdkError::ServiceError(e)) if matches!(e.err(), GetObjectError::NoSuchKey(_)) => {
                // Check manifest to distinguish TenantMismatch from NotFound.
                let manifest_key = Self::manifest_key(&blob_ref.sha256);
                match self
                    .client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(manifest_key)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let body = resp
                            .body
                            .collect()
                            .await
                            .map_err(|e| BlobError::S3(e.to_string()))?
                            .into_bytes();
                        let owner = std::str::from_utf8(&body)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if owner != blob_ref.tenant_id.to_string() {
                            Err(BlobError::TenantMismatch)
                        } else {
                            Err(BlobError::NotFound {
                                tenant_id: blob_ref.tenant_id,
                                sha256: blob_ref.sha256.clone(),
                            })
                        }
                    }
                    Err(_) => Err(BlobError::NotFound {
                        tenant_id: blob_ref.tenant_id,
                        sha256: blob_ref.sha256.clone(),
                    }),
                }
            }
            Err(e) => Err(BlobError::S3(e.to_string())),
        }
    }

    #[instrument(skip(self), fields(tenant=%blob_ref.tenant_id, sha256=%blob_ref.sha256))]
    async fn delete(&self, blob_ref: &BlobRef) -> Result<(), BlobError> {
        // Verify ownership before deleting.
        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(Self::object_key(blob_ref))
            .send()
            .await;

        match head {
            Ok(_) => {
                self.client
                    .delete_object()
                    .bucket(&self.bucket)
                    .key(Self::object_key(blob_ref))
                    .send()
                    .await
                    .map_err(|e| BlobError::S3(e.to_string()))?;
                Ok(())
            }
            Err(SdkError::ServiceError(e)) if matches!(e.err(), HeadObjectError::NotFound(_)) => {
                // Check manifest for cross-tenant detection.
                let manifest_key = Self::manifest_key(&blob_ref.sha256);
                if let Ok(resp) = self
                    .client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(manifest_key)
                    .send()
                    .await
                {
                    let body = resp
                        .body
                        .collect()
                        .await
                        .map_err(|e| BlobError::S3(e.to_string()))?
                        .into_bytes();
                    let owner = std::str::from_utf8(&body).unwrap_or("").trim().to_string();
                    if owner != blob_ref.tenant_id.to_string() {
                        return Err(BlobError::TenantMismatch);
                    }
                }
                Ok(())
            }
            Err(e) => Err(BlobError::S3(e.to_string())),
        }
    }

    async fn exists(&self, blob_ref: &BlobRef) -> Result<bool, BlobError> {
        let result = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(Self::object_key(blob_ref))
            .send()
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(SdkError::ServiceError(e)) if matches!(e.err(), HeadObjectError::NotFound(_)) => {
                Ok(false)
            }
            Err(e) => Err(BlobError::S3(e.to_string())),
        }
    }
}

#[cfg(all(test, feature = "s3"))]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_blob_ref(tenant_id: Uuid, data: &[u8]) -> BlobRef {
        let sha256 = compute_sha256(data);
        BlobRef::new(tenant_id, sha256, "application/octet-stream", data.len() as u64)
    }

    /// Requires localstack running at `RB_BLOB_S3_ENDPOINT` with a pre-created bucket.
    /// Run with: `cargo test -p rb-blob s3_roundtrip --features s3`
    #[tokio::test]
    async fn s3_roundtrip() {
        let store = S3Store::from_env().await.expect("S3Store::from_env");
        let tenant = Uuid::new_v4();
        let data = b"hello s3 blob store";
        let blob_ref = make_blob_ref(tenant, data);

        store.put(&blob_ref, Bytes::from_static(data)).await.expect("put");
        assert!(store.exists(&blob_ref).await.expect("exists"));

        let retrieved = store.get(&blob_ref).await.expect("get");
        assert_eq!(retrieved.as_ref(), data);

        store.delete(&blob_ref).await.expect("delete");
        assert!(!store.exists(&blob_ref).await.expect("exists after delete"));
    }

    #[tokio::test]
    async fn s3_tenant_isolation() {
        let store = S3Store::from_env().await.expect("S3Store::from_env");
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let data = b"shared content different owners";
        let ref_a = make_blob_ref(tenant_a, data);

        store.put(&ref_a, Bytes::from_static(data)).await.expect("put as tenant_a");

        let ref_b = BlobRef::new(
            tenant_b,
            ref_a.sha256.clone(),
            "application/octet-stream",
            ref_a.size,
        );
        let err = store.get(&ref_b).await.expect_err("cross-tenant must fail");
        assert!(
            matches!(err, BlobError::TenantMismatch),
            "expected TenantMismatch, got {err:?}"
        );
    }
}

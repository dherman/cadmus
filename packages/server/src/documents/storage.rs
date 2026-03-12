use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use uuid::Uuid;

use crate::errors::AppError;

/// Wraps the S3 client for document snapshot storage.
///
/// Snapshots are stored as `snapshots/{doc_id}/latest.yrs` — a single compact
/// binary encoding of the full Yrs document state.
#[derive(Clone)]
pub struct SnapshotStorage {
    client: S3Client,
    bucket: String,
}

impl SnapshotStorage {
    pub async fn new(bucket: &str, endpoint: Option<&str>) -> Self {
        let mut config_loader =
            aws_config::defaults(BehaviorVersion::latest()).region("us-east-1");
        if let Some(endpoint) = endpoint {
            config_loader = config_loader.endpoint_url(endpoint);
        }
        let config = config_loader.load().await;
        let client = S3Client::new(&config);
        Self {
            client,
            bucket: bucket.to_string(),
        }
    }

    /// Upload a document snapshot to S3.
    /// Returns the S3 key where the snapshot was stored.
    pub async fn upload_snapshot(&self, doc_id: Uuid, data: &[u8]) -> Result<String, AppError> {
        let key = format!("snapshots/{}/latest.yrs", doc_id);
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(data.to_vec().into())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 upload failed: {}", e)))?;
        Ok(key)
    }

    /// Download a document snapshot from S3.
    /// Returns `None` if the key doesn't exist (NoSuchKey).
    pub async fn download_snapshot(&self, key: &str) -> Result<Option<Vec<u8>>, AppError> {
        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(output) => {
                let bytes = output
                    .body
                    .collect()
                    .await
                    .map_err(|e| AppError::Internal(format!("S3 download failed: {}", e)))?;
                Ok(Some(bytes.to_vec()))
            }
            Err(sdk_err) => {
                if sdk_err
                    .as_service_error()
                    .map_or(false, |e| e.is_no_such_key())
                {
                    Ok(None)
                } else {
                    Err(AppError::Internal(format!("S3 download failed: {}", sdk_err)))
                }
            }
        }
    }

    /// Delete a document's snapshot from S3.
    pub async fn delete_snapshot(&self, doc_id: Uuid) -> Result<(), AppError> {
        let key = format!("snapshots/{}/latest.yrs", doc_id);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("S3 delete failed: {}", e)))?;
        Ok(())
    }
}

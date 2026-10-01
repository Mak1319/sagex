//! Object storage behind a trait: MinIO (S3 SigV4 presigned URLs) in
//! production, an in-memory fake for tests and MinIO-less environments.
//!
//! Upload credential flow (never touches bytes server-side):
//! 1. `POST /api/v1/media/presign` validates type/size, returns a time-boxed
//!    PUT URL + object key.
//! 2. Client PUTs bytes straight to MinIO.
//! 3. Client posts the message referencing the key; the server best-effort
//!    verifies existence (HeadObject) before accepting non-text kinds.

use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::Mutex;

/// Object-storage operations the chat server needs.
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Ensure the bucket exists (idempotent; called once at boot).
    async fn ensure_bucket(&self) -> anyhow::Result<()>;
    /// Mint a time-boxed PUT URL for `key` with the given content type.
    async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        ttl: Duration,
    ) -> anyhow::Result<String>;
    /// Mint a time-boxed GET URL for `key`.
    async fn presign_get(&self, key: &str, ttl: Duration) -> anyhow::Result<String>;
    /// Best-effort existence check (used before accepting media messages).
    async fn exists(&self, key: &str) -> bool;
}

pub type SharedStorage = Arc<dyn Storage>;

/// Live MinIO/S3 backend over SigV4.
pub struct S3Storage {
    client: s3::Client,
    bucket: String,
}

impl S3Storage {
    pub fn new(
        endpoint: &str,
        bucket: &str,
        access_key: &str,
        secret_key: &str,
    ) -> anyhow::Result<Self> {
        let creds = s3::Credentials::new(access_key, secret_key)?;
        let auth = s3::Auth::Static(creds);
        let client = s3::Client::builder(endpoint)?
            .region("us-east-1")
            .auth(auth)
            .build()?;
        Ok(Self {
            client,
            bucket: bucket.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl Storage for S3Storage {
    async fn ensure_bucket(&self) -> anyhow::Result<()> {
        // HeadBucket first: avoids noisy errors when it already exists.
        if self
            .client
            .buckets()
            .head(&self.bucket)
            .send()
            .await
            .is_ok()
        {
            return Ok(());
        }
        self.client
            .buckets()
            .create(&self.bucket)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("create bucket {}: {e:?}", self.bucket))?;
        Ok(())
    }

    async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        ttl: Duration,
    ) -> anyhow::Result<String> {
        let req = self
            .client
            .objects()
            .presign_put(&self.bucket, key)
            .expires_in(ttl)?
            .header(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_str(content_type)?,
            )?
            .build()?;
        Ok(req.url.to_string())
    }

    async fn presign_get(&self, key: &str, ttl: Duration) -> anyhow::Result<String> {
        let req = self
            .client
            .objects()
            .presign_get(&self.bucket, key)
            .expires_in(ttl)?
            .build()?;
        Ok(req.url.to_string())
    }

    async fn exists(&self, key: &str) -> bool {
        self.client
            .objects()
            .head(&self.bucket, key)
            .send()
            .await
            .is_ok()
    }
}

/// In-memory fake: deterministic URLs, everything "exists" once put.
/// Used in tests and when MinIO is unreachable.
#[derive(Default)]
pub struct FakeStorage {
    objects: Mutex<HashMap<String, String>>,
}

#[async_trait::async_trait]
impl Storage for FakeStorage {
    async fn ensure_bucket(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        ttl: Duration,
    ) -> anyhow::Result<String> {
        self.objects.lock().await.insert(
            key.to_string(),
            format!("{content_type};ttl={}", ttl.as_secs()),
        );
        Ok(format!("fake://put/{key}"))
    }

    async fn presign_get(&self, key: &str, ttl: Duration) -> anyhow::Result<String> {
        if !self.exists(key).await {
            anyhow::bail!("no such object: {key}");
        }
        let _ = ttl;
        Ok(format!("fake://get/{key}"))
    }

    async fn exists(&self, key: &str) -> bool {
        self.objects.lock().await.contains_key(key)
    }
}

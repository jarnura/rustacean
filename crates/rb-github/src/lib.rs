mod app_jwt;
mod client;
mod error;
mod secret;
mod state_token;
mod token_cache;
mod webhook;

pub use client::{AppIdentity, AppOwner};
pub use error::GhError;
pub use secret::Secret;
pub use state_token::hash_token;
pub use token_cache::CachedToken;
pub use webhook::{ReplayCache, verify_signature};

use std::sync::Arc;
use std::time::Duration;

use jsonwebtoken::EncodingKey;
use moka::future::Cache;
use reqwest::Client;

/// The central handle for all GitHub App operations.
///
/// Constructed once at startup, stored in `AppState`, and cloned cheaply
/// because all inner fields are `Arc`-wrapped.
#[derive(Clone)]
pub struct GhApp {
    pub app_id: i64,
    /// Opaque RS256 key; never exposed beyond this crate.
    pub(crate) encoding_key: EncodingKey,
    /// Raw webhook secret bytes. Wrapped in Secret so it never logs.
    pub(crate) webhook_secret: Secret<Vec<u8>>,
    /// Replay protection cache for `X-GitHub-Delivery` UUIDs.
    pub replay_cache: ReplayCache,
    /// Cached response from `GET /app` (60 s TTL) to avoid hammering GitHub
    /// from k8s liveness probes.
    identity_cache: Arc<Cache<(), AppIdentity>>,
    /// Shared HTTP client.
    http: Client,
}

impl GhApp {
    /// # Panics
    ///
    /// Does not panic in practice; `reqwest::Client::builder().build()` only
    /// fails with an invalid TLS configuration, which cannot occur with the
    /// default builder.
    #[must_use]
    pub fn new(app_id: i64, encoding_key: EncodingKey, webhook_secret: Secret<Vec<u8>>) -> Self {
        let identity_cache = Cache::builder()
            .max_capacity(1)
            .time_to_live(Duration::from_secs(60))
            .build();

        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client init is infallible with valid config");

        Self {
            app_id,
            encoding_key,
            webhook_secret,
            replay_cache: ReplayCache::new(),
            identity_cache: Arc::new(identity_cache),
            http,
        }
    }

    /// Verifies a GitHub webhook signature header against the raw body.
    ///
    /// # Errors
    ///
    /// Returns [`GhError::BadSignatureFormat`] or [`GhError::SignatureMismatch`].
    pub fn verify_webhook(&self, body: &[u8], sig_header: &str) -> Result<(), GhError> {
        webhook::verify::verify_signature(body, sig_header, &self.webhook_secret)
    }

    /// Returns the GitHub App's identity, using a 60-second cache so
    /// `/v1/health/github-app` is safe to call from liveness probes.
    ///
    /// # Errors
    ///
    /// Returns [`GhError`] if the App JWT cannot be minted or the GitHub API
    /// call fails.
    pub async fn check_identity(&self) -> Result<AppIdentity, GhError> {
        if let Some(cached) = self.identity_cache.get(&()).await {
            return Ok(cached);
        }
        let identity = client::fetch_app_identity(self, &self.http).await?;
        self.identity_cache.insert((), identity).await;
        self.identity_cache
            .get(&())
            .await
            .ok_or_else(|| GhError::ApiError {
                status: 500,
                body: "identity cache miss immediately after insert".to_owned(),
            })
    }
}

impl std::fmt::Debug for GhApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GhApp")
            .field("app_id", &self.app_id)
            .field("encoding_key", &"[REDACTED]")
            .field("webhook_secret", &self.webhook_secret)
            .finish_non_exhaustive()
    }
}

// AppIdentity must be Clone for the moka cache to store it.
impl Clone for AppIdentity {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            slug: self.slug.clone(),
            owner: AppOwner {
                login: self.owner.login.clone(),
            },
        }
    }
}

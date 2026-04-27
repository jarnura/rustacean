// Replay protection cache — implemented in RUSAA-50 (REQ-GH-06).
//
// Moka TTL cache keyed by X-GitHub-Delivery UUID. insert_if_new returns false
// if the delivery ID was already seen, enabling idempotent processing.

use std::sync::Arc;

use moka::future::Cache;

const MAX_CAPACITY: u64 = 10_000;
const TTL_SECS: u64 = 600; // 10 minutes

/// A TTL-bounded cache of processed `X-GitHub-Delivery` UUIDs.
#[derive(Clone, Debug)]
pub struct ReplayCache {
    inner: Arc<Cache<String, ()>>,
}

impl ReplayCache {
    #[must_use]
    pub fn new() -> Self {
        let cache = Cache::builder()
            .max_capacity(MAX_CAPACITY)
            .time_to_live(std::time::Duration::from_secs(TTL_SECS))
            .build();
        Self {
            inner: Arc::new(cache),
        }
    }

    /// Attempts to insert `delivery_id`. Returns `true` if it was new
    /// (caller should process), `false` if it was already seen (replay, skip).
    pub async fn insert_if_new(&self, delivery_id: &str) -> bool {
        if self.inner.contains_key(delivery_id) {
            return false;
        }
        self.inner.insert(delivery_id.to_owned(), ()).await;
        true
    }
}

impl Default for ReplayCache {
    fn default() -> Self {
        Self::new()
    }
}

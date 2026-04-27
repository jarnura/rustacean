//! Replay protection for GitHub webhook deliveries.
//!
//! GitHub re-delivers webhooks on transient failures, and the same delivery
//! UUID may legitimately arrive multiple times. The webhook handler treats
//! repeated deliveries as a no-op based on this cache.
//!
//! Concurrency: `try_insert_new` is atomic via `moka::future::Cache::entry()`,
//! so two simultaneous deliveries with the same `X-GitHub-Delivery` UUID
//! cannot both observe a "new" insert.

use std::sync::Arc;

use moka::future::Cache;

const MAX_CAPACITY: u64 = 10_000;
const TTL_SECS: u64 = 600; // 10 minutes — matches PRD §3.3

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

    /// Atomically records `delivery_id`. Returns `true` if the caller is the
    /// first observer (must process), `false` if another caller already
    /// observed the same id (replay; skip).
    ///
    /// Implemented with `moka::future::Cache::entry().or_insert_with()` so the
    /// existence check and insert are not racy. Concurrent calls with the
    /// same id are deduplicated to exactly one insert event by moka.
    pub async fn try_insert_new(&self, delivery_id: &str) -> bool {
        self.inner
            .entry(delivery_id.to_owned())
            .or_insert_with(async {})
            .await
            .is_fresh()
    }
}

impl Default for ReplayCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc as StdArc;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn first_insert_is_fresh() {
        let cache = ReplayCache::new();
        assert!(cache.try_insert_new("delivery-1").await);
    }

    #[tokio::test]
    async fn second_insert_is_replay() {
        let cache = ReplayCache::new();
        assert!(cache.try_insert_new("delivery-2").await);
        assert!(!cache.try_insert_new("delivery-2").await);
    }

    #[tokio::test]
    async fn distinct_ids_are_independent() {
        let cache = ReplayCache::new();
        assert!(cache.try_insert_new("a").await);
        assert!(cache.try_insert_new("b").await);
        assert!(!cache.try_insert_new("a").await);
        assert!(!cache.try_insert_new("b").await);
    }

    /// Drives 64 concurrent inserts of the same delivery id. Exactly one of
    /// them must observe `true` (the fresh insert); the other 63 must
    /// observe `false` (replay). This proves the TOCTOU race is closed.
    #[tokio::test]
    async fn concurrent_same_id_races_to_one_winner() {
        let cache = StdArc::new(ReplayCache::new());
        let mut tasks: JoinSet<bool> = JoinSet::new();
        for _ in 0..64 {
            let cache = StdArc::clone(&cache);
            tasks.spawn(async move { cache.try_insert_new("racy-delivery").await });
        }
        let mut fresh_count = 0_usize;
        while let Some(res) = tasks.join_next().await {
            if res.expect("task did not panic") {
                fresh_count += 1;
            }
        }
        assert_eq!(
            fresh_count, 1,
            "exactly one concurrent caller must win the insert race"
        );
    }

    #[tokio::test]
    async fn default_constructs_empty_cache() {
        let cache = ReplayCache::default();
        assert!(cache.try_insert_new("first").await);
    }
}

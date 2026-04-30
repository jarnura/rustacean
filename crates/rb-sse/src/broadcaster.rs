use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use dashmap::DashMap;
use rb_schemas::TenantId;
use tokio::sync::broadcast;

use crate::{
    config::SseConfig,
    envelope::{EventId, SseEnvelope},
};

// ---------------------------------------------------------------------------
// Ring buffer
// ---------------------------------------------------------------------------

pub(crate) struct RingBuffer {
    cap: usize,
    ttl: Duration,
    buf: VecDeque<Arc<SseEnvelope>>,
}

impl RingBuffer {
    pub(crate) fn new(cap: usize, ttl: Duration) -> Self {
        Self {
            cap,
            ttl,
            buf: VecDeque::with_capacity(cap.min(64)),
        }
    }

    pub(crate) fn push(&mut self, env: Arc<SseEnvelope>) {
        self.evict_expired();
        if self.buf.len() >= self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(env);
    }

    /// Return all events after `last_id`.
    ///
    /// Returns `None` if `last_id` is not found in the ring (too old, or unknown) —
    /// the caller should emit a `stream-reset` advisory instead.
    pub(crate) fn replay_after(&mut self, last_id: &EventId) -> Option<Vec<Arc<SseEnvelope>>> {
        self.evict_expired();
        let pos = self.buf.iter().position(|e| e.id == *last_id)?;
        Some(self.buf.range(pos + 1..).map(Arc::clone).collect())
    }

    /// Returns a snapshot of the current ring contents (for tests / initial sync).
    #[cfg(test)]
    pub(crate) fn snapshot(&mut self) -> Vec<Arc<SseEnvelope>> {
        self.evict_expired();
        self.buf.iter().map(Arc::clone).collect()
    }

    fn evict_expired(&mut self) {
        let ttl_chrono = chrono::Duration::from_std(self.ttl)
            .unwrap_or(chrono::Duration::seconds(300));
        let cutoff = Utc::now() - ttl_chrono;
        while let Some(front) = self.buf.front() {
            if front.created_at < cutoff {
                self.buf.pop_front();
            } else {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Per-tenant state
// ---------------------------------------------------------------------------

pub(crate) struct TenantState {
    pub(crate) sender: broadcast::Sender<Arc<SseEnvelope>>,
    pub(crate) ring: Mutex<RingBuffer>,
}

// ---------------------------------------------------------------------------
// PerTenantBroadcaster
// ---------------------------------------------------------------------------

pub struct PerTenantBroadcaster {
    states: DashMap<TenantId, Arc<TenantState>>,
    cfg: SseConfig,
}

impl PerTenantBroadcaster {
    #[must_use]
    pub fn new(cfg: SseConfig) -> Self {
        Self {
            states: DashMap::new(),
            cfg,
        }
    }

    fn get_or_create(&self, tenant_id: &TenantId) -> Arc<TenantState> {
        if let Some(s) = self.states.get(tenant_id) {
            return Arc::clone(&*s);
        }
        let entry = self.states.entry(*tenant_id).or_insert_with(|| {
            let (tx, _) = broadcast::channel(self.cfg.channel_capacity);
            Arc::new(TenantState {
                sender: tx,
                ring: Mutex::new(RingBuffer::new(self.cfg.ring_capacity, self.cfg.ring_ttl)),
            })
        });
        Arc::clone(&*entry)
    }

    /// Publish a pre-serialised JSON event to all subscribers of `tenant_id`.
    pub fn publish(&self, tenant_id: &TenantId, event_name: &str, data: String) {
        let env = Arc::new(SseEnvelope::new(event_name, data));
        let state = self.get_or_create(tenant_id);

        if let Ok(mut ring) = state.ring.lock() {
            ring.push(Arc::clone(&env));
        }

        // Error means no live subscribers — ring storage above is sufficient.
        let _ = state.sender.send(Arc::clone(&env));

        metrics::counter!(
            "rb_sse_events_dispatched_total",
            "tenant" => tenant_id.to_string()
        )
        .increment(1);
    }

    /// Subscribe to live events for `tenant_id`, optionally replaying from
    /// `last_event_id` (ring buffer lookup; `None` if ID not found or absent).
    #[must_use]
    pub fn subscribe_raw(
        &self,
        tenant_id: &TenantId,
        last_event_id: Option<&EventId>,
    ) -> (broadcast::Receiver<Arc<SseEnvelope>>, Vec<Arc<SseEnvelope>>) {
        let state = self.get_or_create(tenant_id);

        // Subscribe BEFORE reading the ring so we don't miss events published
        // in the window between ring-read and first channel receive.
        let rx = state.sender.subscribe();

        metrics::gauge!("rb_sse_clients", "tenant" => tenant_id.to_string()).increment(1.0);

        let replay = last_event_id
            .and_then(|id| state.ring.lock().ok()?.replay_after(id))
            .unwrap_or_default();

        (rx, replay)
    }

    /// Decrement the live-client gauge when a stream is dropped.
    pub fn release_client(&self, tenant_id: &TenantId) {
        metrics::gauge!("rb_sse_clients", "tenant" => tenant_id.to_string()).decrement(1.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_broadcaster() -> PerTenantBroadcaster {
        PerTenantBroadcaster::new(SseConfig::default())
    }

    #[test]
    fn ring_buffer_evicts_oldest_when_full() {
        let mut ring = RingBuffer::new(3, Duration::from_secs(300));
        for i in 0..4u8 {
            ring.push(Arc::new(SseEnvelope::new("ev", format!("{i}"))));
        }
        let snap = ring.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].data, "1");
        assert_eq!(snap[2].data, "3");
    }

    #[test]
    fn ring_buffer_replay_after_returns_correct_tail() {
        let mut ring = RingBuffer::new(10, Duration::from_secs(300));
        let mut ids = Vec::new();
        for i in 0..5u8 {
            let env = Arc::new(SseEnvelope::new("ev", format!("{i}")));
            ids.push(env.id.clone());
            ring.push(env);
        }
        let tail = ring.replay_after(&ids[2]).unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].data, "3");
        assert_eq!(tail[1].data, "4");
    }

    #[test]
    fn ring_buffer_replay_after_returns_none_for_unknown_id() {
        let mut ring = RingBuffer::new(10, Duration::from_secs(300));
        ring.push(Arc::new(SseEnvelope::new("ev", "x")));
        let unknown = EventId::new();
        assert!(ring.replay_after(&unknown).is_none());
    }

    #[tokio::test]
    async fn broadcaster_publish_reaches_subscriber() {
        let b = make_broadcaster();
        let tenant = TenantId::new();
        let (mut rx, _) = b.subscribe_raw(&tenant, None);

        b.publish(&tenant, "test.ev", r#"{"x":1}"#.to_owned());

        let env = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("channel closed");
        assert_eq!(env.event, "test.ev");
        assert_eq!(env.data, r#"{"x":1}"#);
    }

    #[tokio::test]
    async fn broadcaster_tenant_isolation() {
        let b = make_broadcaster();
        let t1 = TenantId::new();
        let t2 = TenantId::new();
        let (mut rx1, _) = b.subscribe_raw(&t1, None);
        let (mut rx2, _) = b.subscribe_raw(&t2, None);

        b.publish(&t1, "ev", "for-t1".to_owned());

        let env1 = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx1.recv(),
        )
        .await
        .expect("t1 timeout")
        .expect("t1 channel closed");
        assert_eq!(env1.data, "for-t1");

        // rx2 must not receive t1's event.
        let t2_result = tokio::time::timeout(
            std::time::Duration::from_millis(30),
            rx2.recv(),
        )
        .await;
        assert!(t2_result.is_err(), "t2 should not receive t1 events");
    }
}

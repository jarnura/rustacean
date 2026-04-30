use std::time::Duration;

/// Configuration for the SSE event bus.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Per-tenant broadcast channel capacity (events).
    /// When a slow client lags past this many events, it receives a
    /// `stream-reset` advisory and the connection is closed.
    pub channel_capacity: usize,
    /// Maximum events kept in the per-tenant ring buffer.
    pub ring_capacity: usize,
    /// How long events are kept in the ring buffer for `Last-Event-Id` replay.
    pub ring_ttl: Duration,
    /// Interval between SSE keep-alive comments (prevents proxy timeouts).
    pub keepalive_interval: Duration,
}

impl Default for SseConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1_024,
            ring_capacity: 1_024,
            ring_ttl: Duration::from_secs(300), // 5 min
            keepalive_interval: Duration::from_secs(15),
        }
    }
}

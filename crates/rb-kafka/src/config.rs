use std::time::Duration;

/// Producer configuration loaded from environment.
///
/// Note: `acks` and `enable_idempotence` are intentionally absent — they are
/// hardcoded to ADR-006 §3.1 invariants (`acks=all`, `enable.idempotence=true`)
/// in [`crate::producer::Producer::new`] and cannot be overridden by callers.
#[derive(Debug, Clone)]
pub struct ProducerCfg {
    pub bootstrap_servers: String,
    pub compression_type: String,
    pub linger_ms: u64,
    pub delivery_timeout_ms: u64,
    pub queue_buffering_max_kbytes: u32,
}

impl Default for ProducerCfg {
    fn default() -> Self {
        Self {
            bootstrap_servers: std::env::var("KAFKA_BOOTSTRAP_SERVERS")
                .unwrap_or_else(|_| "kafka:9092".to_owned()),
            compression_type: "lz4".to_owned(),
            linger_ms: 20,
            delivery_timeout_ms: 120_000,
            queue_buffering_max_kbytes: 131_072,
        }
    }
}

/// Consumer configuration loaded from environment.
#[derive(Debug, Clone)]
pub struct ConsumerCfg {
    pub bootstrap_servers: String,
    pub group_id: String,
    pub enable_auto_commit: bool,
    pub isolation_level: String,
    pub auto_offset_reset: String,
    pub max_poll_interval: Duration,
    pub session_timeout: Duration,
}

impl ConsumerCfg {
    #[must_use]
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            bootstrap_servers: std::env::var("KAFKA_BOOTSTRAP_SERVERS")
                .unwrap_or_else(|_| "kafka:9092".to_owned()),
            group_id: group_id.into(),
            enable_auto_commit: false,
            isolation_level: "read_committed".to_owned(),
            auto_offset_reset: "earliest".to_owned(),
            max_poll_interval: Duration::from_secs(300),
            session_timeout: Duration::from_secs(30),
        }
    }
}

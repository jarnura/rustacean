use chrono::Utc;

use crate::envelope::{HEADER_DLQ_AT, HEADER_DLQ_REASON};

/// DLQ topic name convention: `{original_topic}.dlq`
#[must_use]
pub fn dlq_topic(original: &str) -> String {
    format!("{original}.dlq")
}

/// DLQ topic name convention: `{original_topic}.retry`
#[must_use]
pub fn retry_topic(original: &str) -> String {
    format!("{original}.retry")
}

/// Headers added to a message routed to the DLQ.
#[derive(Debug, Clone)]
pub struct DlqHeaders {
    pub reason: String,
    pub at_ms: i64,
}

impl DlqHeaders {
    #[must_use]
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_owned(),
            at_ms: Utc::now().timestamp_millis(),
        }
    }

    /// Converts to a list of `(key, value)` header pairs.
    #[must_use]
    pub fn to_pairs(&self) -> Vec<(String, String)> {
        vec![
            (HEADER_DLQ_REASON.to_owned(), self.reason.clone()),
            (HEADER_DLQ_AT.to_owned(), self.at_ms.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlq_topic_appends_suffix() {
        assert_eq!(dlq_topic("rb.ingest.parse.commands"), "rb.ingest.parse.commands.dlq");
    }

    #[test]
    fn retry_topic_appends_suffix() {
        assert_eq!(retry_topic("rb.ingest.parse.commands"), "rb.ingest.parse.commands.retry");
    }

    #[test]
    fn dlq_headers_include_reason_and_timestamp() {
        let h = DlqHeaders::new("deserialization failure");
        let pairs = h.to_pairs();
        assert!(pairs.iter().any(|(k, v)| k == HEADER_DLQ_REASON && v == "deserialization failure"));
        assert!(pairs.iter().any(|(k, _)| k == HEADER_DLQ_AT));
    }
}

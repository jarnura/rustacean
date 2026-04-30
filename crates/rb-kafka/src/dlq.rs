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

/// Routes a source topic to its DLQ and retry siblings (ADR-006 §3.1).
///
/// Wave-5 stage workers use `DlqRouter::new(topic)` to obtain the correct
/// sink topic without hard-coding the `.dlq` / `.retry` suffix convention.
#[derive(Debug, Clone)]
pub struct DlqRouter {
    source: String,
}

impl DlqRouter {
    #[must_use]
    pub fn new(source_topic: impl Into<String>) -> Self {
        Self {
            source: source_topic.into(),
        }
    }

    /// Returns the DLQ topic name for this source topic.
    #[must_use]
    pub fn dlq_topic(&self) -> String {
        dlq_topic(&self.source)
    }

    /// Returns the retry topic name for this source topic.
    #[must_use]
    pub fn retry_topic(&self) -> String {
        retry_topic(&self.source)
    }

    /// Returns the original source topic name.
    #[must_use]
    pub fn source_topic(&self) -> &str {
        &self.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlq_topic_appends_suffix() {
        assert_eq!(
            dlq_topic("rb.ingest.parse.commands"),
            "rb.ingest.parse.commands.dlq"
        );
    }

    #[test]
    fn retry_topic_appends_suffix() {
        assert_eq!(
            retry_topic("rb.ingest.parse.commands"),
            "rb.ingest.parse.commands.retry"
        );
    }

    #[test]
    fn dlq_router_delegates_to_helpers() {
        let router = DlqRouter::new("rb.ingest.clone.commands");
        assert_eq!(router.source_topic(), "rb.ingest.clone.commands");
        assert_eq!(router.dlq_topic(), "rb.ingest.clone.commands.dlq");
        assert_eq!(router.retry_topic(), "rb.ingest.clone.commands.retry");
    }
}

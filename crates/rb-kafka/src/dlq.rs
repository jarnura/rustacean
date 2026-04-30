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
}

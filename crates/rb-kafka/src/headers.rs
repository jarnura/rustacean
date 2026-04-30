use opentelemetry::propagation::{Extractor, Injector};
use rdkafka::message::{Header, OwnedHeaders};

/// `OTel` `Injector` wrapping an owned `OwnedHeaders` (append-only rdkafka type).
pub struct KafkaHeaderInjector(pub OwnedHeaders);

impl Injector for KafkaHeaderInjector {
    fn set(&mut self, key: &str, value: String) {
        let current = std::mem::replace(&mut self.0, OwnedHeaders::new());
        self.0 = current.insert(Header { key, value: Some(value.as_bytes()) });
    }
}

/// `OTel` `Extractor` backed by decoded Kafka headers (`Vec<(key, value)>`).
pub struct KafkaHeaderExtractor<'a>(pub &'a [(String, String)]);

impl Extractor for KafkaHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|(k, _)| k.as_str()).collect()
    }
}

/// Returns `true` iff `tp` is a structurally valid W3C traceparent.
///
/// Required format: `{2hex}-{32hex}-{16hex}-{2hex}` (version-traceId-parentId-flags).
/// Does not enforce the all-zeros prohibition from the spec; that is left to the `OTel` SDK.
pub fn is_valid_traceparent(tp: &str) -> bool {
    let mut it = tp.splitn(5, '-');
    let version = it.next().unwrap_or("");
    let trace_id = it.next().unwrap_or("");
    let parent_id = it.next().unwrap_or("");
    let flags = it.next().unwrap_or("");
    // A 5th segment means extra dashes — invalid.
    if it.next().is_some() {
        return false;
    }
    is_hex_len(version, 2)
        && is_hex_len(trace_id, 32)
        && is_hex_len(parent_id, 16)
        && is_hex_len(flags, 2)
}

fn is_hex_len(s: &str, expected: usize) -> bool {
    s.len() == expected && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_w3c_traceparent_accepted() {
        assert!(is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        ));
    }

    #[test]
    fn too_few_segments_rejected() {
        assert!(!is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7"
        ));
    }

    #[test]
    fn extra_segment_rejected() {
        assert!(!is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01-extra"
        ));
    }

    #[test]
    fn short_trace_id_rejected() {
        assert!(!is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e47-00f067aa0ba902b7-01"
        ));
    }

    #[test]
    fn short_parent_id_rejected() {
        assert!(!is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902-01"
        ));
    }

    #[test]
    fn non_hex_flags_rejected() {
        assert!(!is_valid_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-zz"
        ));
    }

    #[test]
    fn free_form_string_rejected() {
        assert!(!is_valid_traceparent("not-a-valid-traceparent"));
    }

    #[test]
    fn empty_string_rejected() {
        assert!(!is_valid_traceparent(""));
    }
}

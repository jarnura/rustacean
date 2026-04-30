use opentelemetry::propagation::{Extractor, Injector};
use rdkafka::message::{BorrowedHeaders, Header, Headers as RdHeaders, OwnedHeaders};

/// OTel `Injector` wrapping an owned `OwnedHeaders` (append-only rdkafka type).
pub struct KafkaHeaderInjector(pub OwnedHeaders);

impl Injector for KafkaHeaderInjector {
    fn set(&mut self, key: &str, value: String) {
        let current = std::mem::replace(&mut self.0, OwnedHeaders::new());
        self.0 = current.insert(Header { key, value: Some(value.as_bytes()) });
    }
}

/// OTel `Extractor` over borrowed rdkafka message headers.
pub struct KafkaHeaderExtractor<'a>(pub &'a BorrowedHeaders);

impl<'a> Extractor for KafkaHeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find_map(|h| {
            if h.key == key {
                h.value.and_then(|v| std::str::from_utf8(v).ok())
            } else {
                None
            }
        })
    }

    fn keys(&self) -> Vec<&str> {
        self.0.iter().map(|h| h.key).collect()
    }
}

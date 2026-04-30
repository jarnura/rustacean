use opentelemetry::propagation::Injector;
use rdkafka::message::{Header, OwnedHeaders};

/// `OTel` `Injector` wrapping an owned `OwnedHeaders` (append-only rdkafka type).
pub struct KafkaHeaderInjector(pub OwnedHeaders);

impl Injector for KafkaHeaderInjector {
    fn set(&mut self, key: &str, value: String) {
        let current = std::mem::replace(&mut self.0, OwnedHeaders::new());
        self.0 = current.insert(Header { key, value: Some(value.as_bytes()) });
    }
}

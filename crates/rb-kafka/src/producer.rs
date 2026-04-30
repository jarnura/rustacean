use std::{marker::PhantomData, str::FromStr as _, time::Duration};

use prost::Message as ProstMessage;
use rdkafka::{
    message::{Header, OwnedHeaders},
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use tracing::instrument;

use crate::{
    config::ProducerCfg,
    envelope::{
        DeliveryReport, EnvelopeMeta, EventEnvelope, SchemaVersion, TraceContext, HEADER_ATTEMPT,
        HEADER_BLOB_REF, HEADER_EVENT_ID, HEADER_PROCESS_AFTER_MS, HEADER_SCHEMA_VERSION,
        HEADER_TENANT_ID, HEADER_TRACEPARENT, HEADER_TRACESTATE,
    },
    errors::KafkaError,
    headers::KafkaHeaderInjector,
};

pub struct Producer<E: ProstMessage> {
    inner: FutureProducer,
    _phantom: PhantomData<E>,
}

impl<E: ProstMessage> Producer<E> {
    pub fn new(cfg: &ProducerCfg) -> Result<Self, KafkaError> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.bootstrap_servers)
            .set("acks", &cfg.acks)
            .set("enable.idempotence", cfg.enable_idempotence.to_string())
            .set("compression.type", &cfg.compression_type)
            .set("linger.ms", cfg.linger_ms.to_string())
            .set("delivery.timeout.ms", cfg.delivery_timeout_ms.to_string())
            .set("queue.buffering.max.kbytes", cfg.queue_buffering_max_kbytes.to_string())
            .set("max.in.flight.requests.per.connection", "5")
            .create()?;
        Ok(Self { inner: producer, _phantom: PhantomData })
    }

    #[instrument(skip(self, envelope), fields(topic, event_id = %envelope.event_id))]
    pub async fn publish(
        &self,
        topic: &str,
        key: &[u8],
        envelope: EventEnvelope<E>,
    ) -> Result<DeliveryReport, KafkaError> {
        let headers = build_headers(&envelope);
        let payload = envelope.payload.encode_to_vec();

        let record = FutureRecord::to(topic)
            .key(key)
            .payload(payload.as_slice())
            .headers(headers);

        let (partition, offset) = self
            .inner
            .send(record, Duration::from_secs(30))
            .await
            .map_err(|(e, _)| KafkaError::from(e))?;

        Ok(DeliveryReport { topic: topic.to_owned(), partition, offset })
    }
}

fn build_headers<E: ProstMessage>(envelope: &EventEnvelope<E>) -> OwnedHeaders {
    let tenant_str = envelope.tenant_id.to_string();
    let event_id_str = envelope.event_id.to_string();
    let attempt_str = envelope._meta.attempt.to_string();

    let mut h = OwnedHeaders::new();
    h = h.insert(Header { key: HEADER_TENANT_ID, value: Some(tenant_str.as_bytes()) });
    h = h.insert(Header { key: HEADER_EVENT_ID, value: Some(event_id_str.as_bytes()) });
    h = h.insert(Header {
        key: HEADER_SCHEMA_VERSION,
        value: Some(envelope.schema_version.as_str().as_bytes()),
    });
    h = h.insert(Header { key: HEADER_ATTEMPT, value: Some(attempt_str.as_bytes()) });

    if let Some(ref blob_ref) = envelope.blob_ref {
        h = h.insert(Header { key: HEADER_BLOB_REF, value: Some(blob_ref.as_bytes()) });
    }

    if let Some(ref tc) = envelope.trace_context {
        if !tc.traceparent.is_empty() {
            h = h.insert(Header {
                key: HEADER_TRACEPARENT,
                value: Some(tc.traceparent.as_bytes()),
            });
        }
        if !tc.tracestate.is_empty() {
            h = h.insert(Header {
                key: HEADER_TRACESTATE,
                value: Some(tc.tracestate.as_bytes()),
            });
        }
    }

    // Inject current OTel context (no-op if no propagator is installed).
    let mut injector = KafkaHeaderInjector(h);
    opentelemetry::global::get_text_map_propagator(|prop| {
        prop.inject(&mut injector);
    });
    injector.0
}

/// Decode an [`EventEnvelope<E>`] from raw Kafka message bytes + flattened headers.
pub fn decode_envelope<E: ProstMessage + Default>(
    payload: &[u8],
    headers: &[(String, String)],
    topic: &str,
    partition: i32,
    offset: i64,
) -> Result<EventEnvelope<E>, KafkaError> {
    let get = |key: &'static str| -> Option<String> {
        headers.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    };

    let tenant_str =
        get(HEADER_TENANT_ID).ok_or(KafkaError::MissingHeader(HEADER_TENANT_ID))?;
    let tenant_id = tenant_str
        .parse::<rb_schemas::TenantId>()
        .map_err(|e| KafkaError::InvalidHeaderUuid { header: HEADER_TENANT_ID, source: e })?;

    let event_id_str =
        get(HEADER_EVENT_ID).ok_or(KafkaError::MissingHeader(HEADER_EVENT_ID))?;
    let event_id = event_id_str
        .parse::<uuid::Uuid>()
        .map_err(|e| KafkaError::InvalidHeaderUuid { header: HEADER_EVENT_ID, source: e })?;

    let schema_str =
        get(HEADER_SCHEMA_VERSION).ok_or(KafkaError::MissingHeader(HEADER_SCHEMA_VERSION))?;
    let schema_version = SchemaVersion::from_str(&schema_str).map_err(|e| {
        KafkaError::SchemaMismatch {
            expected: SchemaVersion::V1.as_str().to_owned(),
            got: e,
        }
    })?;

    let blob_ref = get(HEADER_BLOB_REF);
    let traceparent = get(HEADER_TRACEPARENT).unwrap_or_default();
    let tracestate = get(HEADER_TRACESTATE).unwrap_or_default();
    let trace_context = if traceparent.is_empty() {
        None
    } else {
        Some(TraceContext { traceparent, tracestate })
    };

    let attempt = get(HEADER_ATTEMPT)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // Check process_after gate for retry backoff.
    if let Some(due_ms) = get(HEADER_PROCESS_AFTER_MS).and_then(|s| s.parse::<u64>().ok()) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if now_ms < due_ms {
            return Err(KafkaError::Broker(
                "message not yet eligible (process_after_ms)".to_owned(),
            ));
        }
    }

    let payload_msg =
        E::decode(payload).map_err(|e| KafkaError::Deserialization(e.to_string()))?;

    Ok(EventEnvelope {
        tenant_id,
        event_id,
        schema_version,
        trace_context,
        blob_ref,
        created_at: chrono::Utc::now(),
        payload: payload_msg,
        _meta: EnvelopeMeta { topic: topic.to_owned(), partition, offset, attempt },
    })
}

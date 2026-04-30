use rb_kafka::{testing::InProcessBus, EventEnvelope, SchemaVersion, TraceContext};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};

fn make_status_event(tenant_id: TenantId) -> IngestStatusEvent {
    IngestStatusEvent {
        ingest_request_id: "req-1".to_owned(),
        tenant_id: tenant_id.to_string(),
        status: IngestStatus::Processing as i32,
        error_message: String::new(),
        occurred_at_ms: 1_700_000_001_000,
    }
}

#[tokio::test]
async fn envelope_round_trip_headers() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.envelope");

    let tenant_id = TenantId::new();
    let env = EventEnvelope::new(tenant_id, make_status_event(tenant_id));
    let original_event_id = env.event_id;

    producer.publish("test.envelope", &[], env).await.unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    assert_eq!(received.tenant_id, tenant_id);
    assert_eq!(received.event_id, original_event_id);
    assert_eq!(received.schema_version, SchemaVersion::V1);
    assert_eq!(received.payload.ingest_request_id, "req-1");
}

#[tokio::test]
async fn envelope_round_trip_with_trace_context() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.trace.envelope");

    let tenant_id = TenantId::new();
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned();
    let env = EventEnvelope::new(tenant_id, make_status_event(tenant_id))
        .with_trace_context(TraceContext {
            traceparent: traceparent.clone(),
            tracestate: String::new(),
        });

    producer.publish("test.trace.envelope", &[], env).await.unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    let tc = received.trace_context.unwrap();
    assert_eq!(tc.traceparent, traceparent);
}

#[tokio::test]
async fn envelope_round_trip_with_blob_ref() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.blob.envelope");

    let tenant_id = TenantId::new();
    let blob_uri = format!("rb-blob://tenant_{tenant_id}/abcdef1234567890abcdef");
    let env = EventEnvelope::new(tenant_id, make_status_event(tenant_id))
        .with_blob_ref(blob_uri.clone());

    producer.publish("test.blob.envelope", &[], env).await.unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    assert_eq!(received.blob_ref, Some(blob_uri));
}

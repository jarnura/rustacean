use rb_kafka::{EventEnvelope, SchemaVersion, dlq_topic, testing::InProcessBus};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};

fn poison_event(tenant_id: TenantId) -> EventEnvelope<IngestStatusEvent> {
    EventEnvelope::new(
        tenant_id,
        IngestStatusEvent {
            ingest_request_id: "req-poison".to_owned(),
            tenant_id: tenant_id.to_string(),
            status: IngestStatus::Failed as i32,
            error_message: "simulated failure".to_owned(),
            occurred_at_ms: 0,
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn terminal_error_routes_to_dlq() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("rb.ingest.parse.commands");

    // Subscribe DLQ consumer before publishing.
    let dlq_name = dlq_topic("rb.ingest.parse.commands");
    let dlq_consumer = bus.consumer::<IngestStatusEvent>(&dlq_name);

    let tenant_id = TenantId::new();
    let env = poison_event(tenant_id);
    let event_id = env.event_id;

    producer
        .publish("rb.ingest.parse.commands", &[], env)
        .await
        .unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    assert_eq!(received.event_id, event_id);

    // Simulate terminal processing failure → route to DLQ.
    consumer
        .nack_to_dlq(&received, "deserialization failure")
        .await
        .unwrap();

    let dlq_msg = dlq_consumer.next().await.unwrap().unwrap();
    assert_eq!(dlq_msg.event_id, event_id);
}

#[tokio::test]
async fn dlq_message_preserves_original_payload() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("rb.ingest.graph.commands");

    let dlq_name = dlq_topic("rb.ingest.graph.commands");
    let dlq_consumer = bus.consumer::<IngestStatusEvent>(&dlq_name);

    let tenant_id = TenantId::new();
    let env = poison_event(tenant_id);

    producer
        .publish("rb.ingest.graph.commands", &[], env.clone())
        .await
        .unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    consumer
        .nack_to_dlq(&received, "schema version mismatch")
        .await
        .unwrap();

    let dlq_msg = dlq_consumer.next().await.unwrap().unwrap();
    // Payload fields.
    assert_eq!(dlq_msg.payload.ingest_request_id, "req-poison");
    assert_eq!(dlq_msg.payload.error_message, "simulated failure");
    // Envelope headers must be preserved (K-HIGH-2).
    assert_eq!(dlq_msg.tenant_id, tenant_id);
    assert_eq!(dlq_msg.event_id, env.event_id);
    assert_eq!(dlq_msg.schema_version, SchemaVersion::V1);
}

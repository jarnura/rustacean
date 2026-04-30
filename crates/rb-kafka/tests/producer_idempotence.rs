use rb_kafka::{EventEnvelope, testing::InProcessBus};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};
use uuid::Uuid;

fn make_event(tenant_id: TenantId, event_id: Uuid) -> EventEnvelope<IngestStatusEvent> {
    EventEnvelope::new(
        tenant_id,
        IngestStatusEvent {
            ingest_request_id: "req-idem".to_owned(),
            tenant_id: tenant_id.to_string(),
            status: IngestStatus::Processing as i32,
            error_message: String::new(),
            occurred_at_ms: 0,
        },
    )
    .with_event_id(event_id)
}

#[tokio::test]
async fn duplicate_event_id_is_deduplicated() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.idempotent");

    let tenant_id = TenantId::new();
    let event_id = Uuid::new_v4();

    // Same event_id published twice.
    let report1 = producer
        .publish("test.idempotent", &[], make_event(tenant_id, event_id))
        .await
        .unwrap();
    let report2 = producer
        .publish("test.idempotent", &[], make_event(tenant_id, event_id))
        .await
        .unwrap();

    // First delivery has offset 0; duplicate is synthetic (offset = -1).
    assert_eq!(report1.offset, 0);
    assert_eq!(
        report2.offset, -1,
        "duplicate should return synthetic dedup report"
    );

    // Only one message delivered to the topic.
    let received = consumer.next().await.unwrap().unwrap();
    assert_eq!(received.event_id, event_id);
}

#[tokio::test]
async fn distinct_event_ids_both_delivered() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.distinct");

    let tenant_id = TenantId::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    assert_ne!(id_a, id_b);

    producer
        .publish("test.distinct", &[], make_event(tenant_id, id_a))
        .await
        .unwrap();
    producer
        .publish("test.distinct", &[], make_event(tenant_id, id_b))
        .await
        .unwrap();

    let first = consumer.next().await.unwrap().unwrap();
    let second = consumer.next().await.unwrap().unwrap();

    let ids: std::collections::HashSet<_> = [first.event_id, second.event_id].into();
    assert!(ids.contains(&id_a));
    assert!(ids.contains(&id_b));
}

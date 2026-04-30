//! K-HIGH-1: Assert that all required `rb_kafka_*` metric names are emitted.
//!
//! Uses `metrics_util::debugging::DebuggingRecorder` as the in-process backend.
//! Only metric *names* are checked — label values are intentionally loose so that
//! the test does not break if label sets are extended later.

use metrics_util::debugging::DebuggingRecorder;
use rb_kafka::{EventEnvelope, dlq_topic, testing::InProcessBus};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};

fn make_event(tenant_id: TenantId) -> EventEnvelope<IngestStatusEvent> {
    EventEnvelope::new(
        tenant_id,
        IngestStatusEvent {
            ingest_request_id: "req-metrics-test".to_owned(),
            tenant_id: tenant_id.to_string(),
            status: IngestStatus::Processing as i32,
            error_message: String::new(),
            occurred_at_ms: 0,
        },
    )
}

#[tokio::test]
async fn all_required_metric_names_are_emitted() {
    // Install the debugging recorder as the global metrics recorder.
    // `DebuggingRecorder::per_thread()` is preferred in tests to avoid
    // cross-test pollution, but the global install is fine here because each
    // test binary runs metrics collection independently.
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    // Install; ignore error if another test already set a recorder in this process.
    let _ = recorder.install();

    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let topic = "rb.metrics.test";
    let dlq_name = dlq_topic(topic);
    let consumer = bus.consumer::<IngestStatusEvent>(topic);
    let dlq_consumer = bus.consumer::<IngestStatusEvent>(&dlq_name);

    let tenant_id = TenantId::new();
    let env = make_event(tenant_id);

    // 1. Publish → emits rb_kafka_messages_total{op=produce, outcome=ok}
    producer.publish(topic, &[], env).await.unwrap();

    // 2. Consume → emits rb_kafka_messages_total{op=consume, outcome=ok}
    //              and rb_kafka_consume_lag_seconds
    let received = consumer.next().await.unwrap().unwrap();

    // 3. nack_to_dlq → emits rb_kafka_dlq_total
    consumer
        .nack_to_dlq(&received, "test-reason")
        .await
        .unwrap();

    // 4. Read back the DLQ message so dlq_consumer is exercised.
    let _dlq_msg = dlq_consumer.next().await.unwrap().unwrap();

    // Collect all metric keys from the snapshot.
    let snapshot = snapshotter.snapshot();
    let metric_names: Vec<String> = snapshot
        .into_vec()
        .into_iter()
        .map(|(composite_key, _, _, _)| composite_key.key().name().to_owned())
        .collect();

    // All required metric names must appear at least once.
    let required = [
        "rb_kafka_messages_total",
        "rb_kafka_consume_lag_seconds",
        "rb_kafka_dlq_total",
        "rb_kafka_e2e_latency_seconds",
    ];

    for name in &required {
        assert!(
            metric_names.iter().any(|n| n == *name),
            "metric '{name}' was not emitted; found: {metric_names:?}"
        );
    }
}

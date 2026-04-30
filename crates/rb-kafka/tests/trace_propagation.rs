use rb_kafka::{testing::InProcessBus, EventEnvelope, TraceContext};
use rb_schemas::{IngestStatus, IngestStatusEvent, TenantId};

fn make_event(tenant_id: TenantId, seq: u32) -> EventEnvelope<IngestStatusEvent> {
    EventEnvelope::new(
        tenant_id,
        IngestStatusEvent {
            ingest_request_id: format!("req-{seq}"),
            tenant_id: tenant_id.to_string(),
            status: IngestStatus::Processing as i32,
            error_message: String::new(),
            occurred_at_ms: 0,
        },
    )
}

#[tokio::test]
async fn traceparent_survives_round_trip() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.trace");

    let tenant_id = TenantId::new();
    // W3C trace-context test vector (§4.2.1 of the spec).
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_owned();
    let tracestate = "congo=t61rcWkgMzE,rojo=00f067aa0ba902b7".to_owned();

    let env = make_event(tenant_id, 0).with_trace_context(TraceContext {
        traceparent: traceparent.clone(),
        tracestate: tracestate.clone(),
    });

    producer.publish("test.trace", &[], env).await.unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    let tc = received.trace_context.expect("trace context must survive round-trip");
    assert_eq!(tc.traceparent, traceparent, "traceparent must match");
    assert_eq!(tc.tracestate, tracestate, "tracestate must match");
}

#[tokio::test]
async fn message_without_trace_context_has_none() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.notrace");

    let tenant_id = TenantId::new();
    let env = make_event(tenant_id, 0);

    producer.publish("test.notrace", &[], env).await.unwrap();

    let received = consumer.next().await.unwrap().unwrap();
    assert!(
        received.trace_context.is_none(),
        "no trace context → must remain None after round-trip"
    );
}

#[tokio::test]
async fn trace_ids_are_distinct_across_messages() {
    let bus = InProcessBus::new();
    let producer = bus.producer::<IngestStatusEvent>();
    let consumer = bus.consumer::<IngestStatusEvent>("test.multi_trace");

    let tenant_id = TenantId::new();

    let contexts = [
        TraceContext {
            traceparent: "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01".to_owned(),
            tracestate: String::new(),
        },
        TraceContext {
            traceparent: "00-cccccccccccccccccccccccccccccccc-dddddddddddddddd-01".to_owned(),
            tracestate: String::new(),
        },
    ];

    for (i, tc) in contexts.into_iter().enumerate() {
        let env = make_event(tenant_id, i as u32).with_trace_context(tc);
        producer.publish("test.multi_trace", &[], env).await.unwrap();
    }

    let msg_a = consumer.next().await.unwrap().unwrap();
    let msg_b = consumer.next().await.unwrap().unwrap();

    let tp_a = msg_a.trace_context.unwrap().traceparent;
    let tp_b = msg_b.trace_context.unwrap().traceparent;
    assert_ne!(tp_a, tp_b, "distinct messages must carry distinct trace contexts");
}

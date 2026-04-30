use futures::StreamExt as _;
use rb_sse::{EventBus, SseConfig, TenantId, testing};

#[tokio::test]
async fn single_client_receives_published_event() {
    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();

    let (mut rx, _) = testing::raw_subscribe(&bus, &tenant, None);

    bus.publish_raw(&tenant, "ingest.status", r#"{"status":"processing"}"#.to_owned());

    let env = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("channel closed");

    assert_eq!(env.event, "ingest.status");
    assert!(env.data.contains("processing"));
    assert!(!env.id.as_str().is_empty());
}

#[tokio::test]
async fn single_client_receives_events_in_order() {
    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();

    let (mut rx, _) = testing::raw_subscribe(&bus, &tenant, None);

    for i in 0..5u32 {
        bus.publish_raw(&tenant, "ev", format!(r#"{{"n":{i}}}"#));
    }

    for expected in 0..5u32 {
        let env = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");
        let v: serde_json::Value = serde_json::from_str(&env.data).unwrap();
        assert_eq!(v["n"], expected, "events must arrive in publish order");
    }
}

#[tokio::test]
async fn event_stream_implements_stream_trait() {

    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();

    // EventStream implements Stream — we can call .next() on it directly.
    let mut stream = bus.subscribe(&tenant, None);

    // Publish from a background task so the stream has something to receive.
    let bus2 = bus.clone();
    let tenant2 = tenant;
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        bus2.publish_raw(&tenant2, "ping", "{}".to_owned());
    });

    let item = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
        .await
        .expect("timeout")
        .expect("stream ended")
        .expect("infallible error");

    // axum SSE Event doesn't expose its fields directly, but we can verify
    // it was produced without error (Ok-variant from the Result).
    let _ = item;
}

#[tokio::test]
async fn event_id_is_unique_across_events() {
    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();
    let (mut rx, _) = testing::raw_subscribe(&bus, &tenant, None);

    bus.publish_raw(&tenant, "ev", "1".to_owned());
    bus.publish_raw(&tenant, "ev", "2".to_owned());

    let e1 = rx.recv().await.unwrap();
    let e2 = rx.recv().await.unwrap();
    assert_ne!(e1.id, e2.id, "every event must have a unique ID");
}

#[tokio::test]
async fn publish_with_no_subscribers_does_not_panic() {
    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();
    // No subscriber — should not panic.
    bus.publish_raw(&tenant, "ev", "{}".to_owned());
    bus.publish_raw(&tenant, "ev", "{}".to_owned());
}

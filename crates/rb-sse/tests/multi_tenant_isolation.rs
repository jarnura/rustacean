use rb_sse::{EventBus, SseConfig, TenantId, testing};

/// Events published for tenant A must NOT reach tenant B's subscriber,
/// and vice versa — even when the same EventBus instance is used.
#[tokio::test]
async fn tenant_a_events_not_visible_to_tenant_b() {
    let bus = EventBus::new(SseConfig::default());
    let t_a = TenantId::new();
    let t_b = TenantId::new();

    let (mut rx_a, _) = testing::raw_subscribe(&bus, &t_a, None);
    let (mut rx_b, _) = testing::raw_subscribe(&bus, &t_b, None);

    bus.publish_raw(&t_a, "ingest.status", r#"{"tenant":"A"}"#.to_owned());

    // Tenant A must receive its event.
    let ev_a = tokio::time::timeout(std::time::Duration::from_millis(200), rx_a.recv())
        .await
        .expect("A timed out")
        .expect("A channel closed");
    assert!(ev_a.data.contains("\"A\""));

    // Tenant B must NOT receive tenant A's event.
    let t_b_result =
        tokio::time::timeout(std::time::Duration::from_millis(50), rx_b.recv()).await;
    assert!(
        t_b_result.is_err(),
        "tenant B should not receive tenant A's events"
    );
}

#[tokio::test]
async fn tenant_b_events_not_visible_to_tenant_a() {
    let bus = EventBus::new(SseConfig::default());
    let t_a = TenantId::new();
    let t_b = TenantId::new();

    let (mut rx_a, _) = testing::raw_subscribe(&bus, &t_a, None);
    let (mut rx_b, _) = testing::raw_subscribe(&bus, &t_b, None);

    bus.publish_raw(&t_b, "ingest.status", r#"{"tenant":"B"}"#.to_owned());

    let ev_b = tokio::time::timeout(std::time::Duration::from_millis(200), rx_b.recv())
        .await
        .expect("B timed out")
        .expect("B channel closed");
    assert!(ev_b.data.contains("\"B\""));

    let t_a_result =
        tokio::time::timeout(std::time::Duration::from_millis(50), rx_a.recv()).await;
    assert!(
        t_a_result.is_err(),
        "tenant A should not receive tenant B's events"
    );
}

#[tokio::test]
async fn multiple_subscribers_for_same_tenant_all_receive() {
    let bus = EventBus::new(SseConfig::default());
    let tenant = TenantId::new();

    let (mut rx1, _) = testing::raw_subscribe(&bus, &tenant, None);
    let (mut rx2, _) = testing::raw_subscribe(&bus, &tenant, None);
    let (mut rx3, _) = testing::raw_subscribe(&bus, &tenant, None);

    bus.publish_raw(&tenant, "ev", r#"{"n":99}"#.to_owned());

    for (label, rx) in [("rx1", &mut rx1), ("rx2", &mut rx2), ("rx3", &mut rx3)] {
        let env = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .unwrap_or_else(|_| panic!("{label} timed out"))
            .unwrap_or_else(|_| panic!("{label} channel closed"));
        let v: serde_json::Value = serde_json::from_str(&env.data).unwrap();
        assert_eq!(v["n"], 99, "{label} must receive the event");
    }
}

#[tokio::test]
async fn interleaved_events_for_two_tenants_stay_isolated() {
    let bus = EventBus::new(SseConfig::default());
    let t_a = TenantId::new();
    let t_b = TenantId::new();

    let (mut rx_a, _) = testing::raw_subscribe(&bus, &t_a, None);
    let (mut rx_b, _) = testing::raw_subscribe(&bus, &t_b, None);

    bus.publish_raw(&t_a, "ev", r#"{"t":"A","n":1}"#.to_owned());
    bus.publish_raw(&t_b, "ev", r#"{"t":"B","n":1}"#.to_owned());
    bus.publish_raw(&t_a, "ev", r#"{"t":"A","n":2}"#.to_owned());
    bus.publish_raw(&t_b, "ev", r#"{"t":"B","n":2}"#.to_owned());

    for expected_n in [1u32, 2] {
        let ev =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx_a.recv())
                .await
                .expect("A timed out")
                .expect("A closed");
        let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
        assert_eq!(v["t"], "A");
        assert_eq!(v["n"], expected_n);
    }

    for expected_n in [1u32, 2] {
        let ev =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx_b.recv())
                .await
                .expect("B timed out")
                .expect("B closed");
        let v: serde_json::Value = serde_json::from_str(&ev.data).unwrap();
        assert_eq!(v["t"], "B");
        assert_eq!(v["n"], expected_n);
    }
}

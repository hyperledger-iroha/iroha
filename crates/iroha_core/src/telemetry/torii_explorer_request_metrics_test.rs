#[test]
fn torii_explorer_metrics_are_recorded_via_telemetry_wrapper() {
    let metrics = Arc::new(Metrics::default());
    let telemetry = Telemetry::new(metrics.clone(), true);
    telemetry.record_torii_explorer_request(
        "/v1/explorer/transactions",
        "ok",
        Duration::from_millis(25),
    );
    assert_eq!(
        metrics
            .torii_explorer_requests_total
            .with_label_values(&["/v1/explorer/transactions", "ok"])
            .get(),
        1,
        "explorer request counter should increment for each wrapper call"
    );
    assert_eq!(
        metrics
            .torii_explorer_request_duration_seconds
            .with_label_values(&["/v1/explorer/transactions", "ok"])
            .get_sample_count(),
        1,
        "explorer request latency histogram should record wrapper observations"
    );
}

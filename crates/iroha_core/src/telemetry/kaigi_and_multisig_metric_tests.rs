#[test]
fn multisig_direct_sign_rejection_metrics_recorded() {
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);

    record_social_rejection(&telemetry, "multisig_direct_sign");
    assert_eq!(
        metrics
            .social_rejections_total
            .with_label_values(&["multisig_direct_sign"])
            .get(),
        1
    );
    assert_eq!(metrics.multisig_direct_sign_reject_total.get(), 1);

    record_social_rejection(&telemetry, "other_reason");
    assert_eq!(
        metrics
            .social_rejections_total
            .with_label_values(&["other_reason"])
            .get(),
        1
    );
    assert_eq!(
        metrics.multisig_direct_sign_reject_total.get(),
        1,
        "counter should not increment for unrelated social rejections"
    );
}

#[test]
fn kaigi_domain_aggregate_counters_track_dimensional_events() {
    let metrics = Arc::new(Metrics::default());
    let telemetry = StateTelemetry::new(Arc::clone(&metrics), true);
    let domain = DomainId::try_new("kaigi", "universal").expect("Kaigi domain fixture");
    let relay = AccountId::new(checked_keypair().public_key().clone());
    let call: Name = "diagnostic".parse().expect("Kaigi call fixture");
    let domain_label = domain.to_string();

    telemetry.record_kaigi_manifest_update(&domain, "rotate", 2);
    telemetry.record_kaigi_failover(&domain, &call, 2);
    telemetry.record_kaigi_relay_health(&domain, &relay, KaigiRelayHealthStatus::Degraded);

    assert_eq!(
        metrics
            .kaigi_relay_manifest_updates_by_domain_total
            .with_label_values(&[domain_label.as_str()])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .kaigi_relay_failovers_by_domain_total
            .with_label_values(&[domain_label.as_str()])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .kaigi_relay_health_reports_by_domain_total
            .with_label_values(&[domain_label.as_str()])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .kaigi_relay_manifest_updates_total
            .with_label_values(&[domain_label.as_str(), "rotate"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .kaigi_relay_failover_total
            .with_label_values(&[domain_label.as_str(), call.as_ref()])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .kaigi_relay_health_reports_total
            .with_label_values(&[domain_label.as_str(), "degraded"])
            .get(),
        1
    );
}

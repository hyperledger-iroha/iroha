// Runtime dependency retention, including separate hardware and observer clients.
#[test]
fn runtime_dependency_builders_retain_injected_instances() {
    let acme_client: Arc<dyn sorafs::gateway::AcmeClient> = Arc::new(TestAcmeClient);
    let compliance_transport: Arc<dyn sorafs::gateway::GatewayComplianceFeedTransport> =
        Arc::new(TestComplianceFeedTransport);
    let fixture = sorafs::hardware_test_support::SignedFixture::new(
        5,
        sorafs::hardware_test_support::TestSignerMode::Sign,
    );
    let hardware: Arc<dyn sorafs::StreamTokenHardwareClientV1> = fixture.clone();
    let observer: Arc<dyn sorafs::StreamTokenStateObserverClientV1> = Arc::new(
        sorafs::hardware_test_support::SignedObserver(fixture.clone()),
    );
    let approved = sorafs::StreamTokenApprovedCustodyAnchorV1::new(
        fixture.pins.config_digest(),
        sorafs::hardware_test_support::anchor(100),
    )
    .expect("independent approved public pin");
    let dependencies = ToriiRuntimeDeps::new(
        crate::build_identity_test_fixture::build_identity(),
        routing::MaybeTelemetry::disabled(),
    )
        .with_sorafs_stream_token_hardware_client(Arc::clone(&hardware))
        .with_sorafs_stream_token_state_observer(Arc::clone(&observer))
        .with_sorafs_stream_token_approved_anchor(approved)
        .with_sorafs_gateway_acme_client(Arc::clone(&acme_client))
        .with_sorafs_gateway_compliance_feed_transport(Arc::clone(&compliance_transport));
    assert!(Arc::ptr_eq(
        dependencies
            .sorafs_stream_token_hardware_client
            .as_ref()
            .expect("stream-token hardware client retained"),
        &hardware
    ));
    assert!(Arc::ptr_eq(
        dependencies
            .sorafs_stream_token_state_observer
            .as_ref()
            .expect("stream-token observer retained"),
        &observer
    ));
    let retained = dependencies
        .sorafs_stream_token_approved_anchor
        .expect("approved anchor retained");
    assert_eq!(retained.config_digest(), approved.config_digest());
    assert_eq!(retained.anchor(), approved.anchor());
    assert!(Arc::ptr_eq(
        dependencies
            .sorafs_gateway_acme_client
            .as_ref()
            .expect("ACME client retained"),
        &acme_client
    ));
    assert!(Arc::ptr_eq(
        dependencies
            .sorafs_gateway_compliance_feed_transport
            .as_ref()
            .expect("compliance transport retained"),
        &compliance_transport
    ));
}

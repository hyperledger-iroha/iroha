#[test]
fn privacy_release_anchor_binding_is_exact_and_drift_checked() {
    let binding = privacy_release_anchor_runtime_binding();
    assert_eq!(validate_wire_binding(&binding), Ok(()));
    assert_eq!(
        validate_observation(&binding, &observation(&binding)),
        Ok(())
    );

    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_privacy_release_anchor(Arc::new(ServerTestPrivacyReleaseAnchor::exact()));
    assert_eq!(
        validate_exact_backend_set(std::slice::from_ref(&binding), &backends),
        Ok(())
    );
    make_server_observation(&binding, &backends)
        .expect("stable exact finalized release anchor qualifies twice");
    assert_eq!(
        validate_exact_backend_set(&[], &backends),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );
    assert_eq!(
        validate_exact_backend_set(
            std::slice::from_ref(&binding),
            &RuntimeProviderBrokerBackendsV1::new(),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );

    let mut confused = binding.clone();
    confused.governance_request_ingress_binding =
        Some(governance_request_ingress_binding_to_wire(
            server_test_request_ingress_binding(TEST_SIGNER_KEY),
        ));
    assert_eq!(
        validate_wire_binding(&confused),
        Err(BrokerError::BindingMismatch)
    );

    let substituted = RuntimeProviderBrokerBackendsV1::new()
        .with_privacy_release_anchor(Arc::new(
            ServerTestPrivacyReleaseAnchor::substituted(),
        ));
    assert_eq!(
        make_server_observation(&binding, &substituted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );

    let drifted = RuntimeProviderBrokerBackendsV1::new().with_privacy_release_anchor(
        Arc::new(ServerTestPrivacyReleaseAnchor::drifting()),
    );
    assert_eq!(
        make_server_observation(&binding, &drifted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
}

#[test]
fn privacy_release_anchor_operations_are_canonical_and_read_back_cas() {
    assert!(operation_is_known(
        OPERATION_PRIVACY_RELEASE_ANCHOR_FINALIZED_HEAD_V1
    ));
    assert!(operation_is_known(
        OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1
    ));
    assert_eq!(
        operation_frame_limit(OPERATION_PRIVACY_RELEASE_ANCHOR_FINALIZED_HEAD_V1),
        MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1
    );
    assert_eq!(
        operation_frame_limit(OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1),
        MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1
    );

    let binding = privacy_release_anchor_runtime_binding();
    let provider = Arc::new(ServerTestPrivacyReleaseAnchor::exact());
    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_privacy_release_anchor(provider.clone());
    let observed = make_server_observation(&binding, &backends)
        .expect("qualify stable finalized release anchor");
    let state = BrokerServerStateV1 {
        chain_id: "privacy-release-anchor-test-chain".to_owned(),
        network_id: server_test_network_id(),
        catalog: vec![binding.clone()],
        observations: vec![observed.clone()],
        backends,
    };
    let query_id = [0x19; 32];
    let window = sorafs_node::PrivacyAggregateCycleWindow {
        cycle_start_unix: 1_000,
        cycle_end_unix: 2_000,
        due_at_unix: 2_000,
    };
    let scope = sorafs_node::TransparencyLeaderLeaseScopeV1::try_new(
        query_id, window, [0x29; 32],
    )
    .expect("canonical release-anchor lease scope");
    let genesis = sorafs_node::PrivacyReleaseAnchorHeadV1::genesis(query_id);
    let next = sorafs_node::PrivacyReleaseAnchorHeadV1::try_from_parts(
        query_id,
        1,
        scope.cycle_id(),
        [0x39; 32],
        Some([0x49; 32]),
    )
    .expect("canonical direct successor");
    let lease_binding = sorafs_node::TransparencyRuntimeProviderBindingV1::try_new(
        "sealed-cas://sorafs/transparency/leader-primary",
        9,
        [0x59; 32],
    )
    .expect("canonical leader-lease binding");
    let lease = sorafs_node::TransparencyLeaderLeaseGrantV1::try_new(
        [0x69; 32],
        scope,
        1,
        2_000,
        3_000,
        lease_binding,
    )
    .expect("canonical leader-lease grant");

    let finalized_request = make_operation_request(
        [0xB1; 32],
        1,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_PRIVACY_RELEASE_ANCHOR_FINALIZED_HEAD_V1,
        encode_canonical(
            &PrivacyReleaseAnchorFinalizedHeadRequestWireV1 { query_id },
            MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
        )
        .expect("encode finalized-head request"),
    )
    .expect("construct finalized-head operation");
    validate_operation_request(&finalized_request)
        .expect("validate canonical finalized-head operation");
    let finalized_result = dispatch_server_operation(&state, &finalized_request)
        .expect("dispatch finalized-head operation");
    let decoded = decode_canonical::<PrivacyReleaseAnchorHeadWireV1>(
        &finalized_result,
        MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1,
    )
    .and_then(PrivacyReleaseAnchorHeadWireV1::to_head)
    .expect("decode canonical finalized head");
    assert_eq!(decoded, genesis);

    let compare = PrivacyReleaseAnchorCompareAndSetRequestWireV1 {
        expected: PrivacyReleaseAnchorHeadWireV1::from_head(genesis),
        next: PrivacyReleaseAnchorHeadWireV1::from_head(next),
        lease: TransparencyLeaderLeaseGrantWireV1::from_grant(&lease),
    };
    let compare_request = make_operation_request(
        [0xB2; 32],
        2,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1,
        encode_canonical(&compare, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
            .expect("encode compare-and-set request"),
    )
    .expect("construct compare-and-set operation");
    validate_operation_request(&compare_request)
        .expect("validate canonical compare-and-set operation");
    let compare_result = dispatch_server_operation(&state, &compare_request)
        .expect("dispatch compare-and-set operation");
    decode_canonical::<()>(&compare_result, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
        .expect("decode payload-free compare-and-set result");
    assert_eq!(provider.compare_and_set_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sorafs_node::PrivacyReleaseAnchorV1::finalized_head(
            provider.as_ref(),
            query_id,
        )
        .expect("read back committed finalized head"),
        next
    );

    let mut noncanonical = compare.clone();
    noncanonical.next.sequence = 3;
    let invalid_request = make_operation_request(
        [0xB3; 32],
        3,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_PRIVACY_RELEASE_ANCHOR_COMPARE_AND_SET_V1,
        encode_canonical(&noncanonical, MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
            .expect("encode noncanonical compare-and-set request"),
    )
    .expect("construct noncanonical compare-and-set operation");
    assert_eq!(
        validate_operation_request(&invalid_request),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        provider.compare_and_set_calls.load(Ordering::SeqCst),
        1,
        "noncanonical requests fail before provider evaluation"
    );

    let no_readback = Arc::new(ServerTestPrivacyReleaseAnchor::without_readback());
    let no_readback_backends = RuntimeProviderBrokerBackendsV1::new()
        .with_privacy_release_anchor(no_readback.clone());
    let no_readback_observed = make_server_observation(&binding, &no_readback_backends)
        .expect("qualify non-persisting test release anchor");
    let no_readback_state = BrokerServerStateV1 {
        chain_id: "privacy-release-anchor-no-readback-test-chain".to_owned(),
        network_id: server_test_network_id(),
        catalog: vec![binding],
        observations: vec![no_readback_observed],
        backends: no_readback_backends,
    };
    assert_eq!(
        dispatch_server_operation(&no_readback_state, &compare_request),
        Err(BrokerError::Ambiguous)
    );
    assert_eq!(no_readback.compare_and_set_calls.load(Ordering::SeqCst), 1);

    let unit = encode_canonical(&(), MAX_PRIVACY_RELEASE_ANCHOR_FRAME_BYTES_V1)
        .expect("encode payload-free failure");
    assert!(
        make_operation_response(
            &compare_request,
            STATUS_AMBIGUOUS_V1,
            unit.clone(),
            &state.network_id,
        )
        .is_ok()
    );
    assert!(
        make_operation_response(
            &compare_request,
            STATUS_CONFLICT_V1,
            unit.clone(),
            &state.network_id,
        )
        .is_ok()
    );
    assert_eq!(
        make_operation_response(
            &finalized_request,
            STATUS_AMBIGUOUS_V1,
            unit,
            &state.network_id,
        ),
        Err(BrokerError::Protocol)
    );

    let zero_query =
        PrivacyReleaseAnchorFinalizedHeadRequestWireV1 { query_id: [0; 32] };
    assert_eq!(
        validate_privacy_release_anchor_query(zero_query),
        Err(BrokerError::Rejected)
    );
}

#[test]
fn transparency_leader_lease_binding_is_exact_and_drift_checked() {
    let binding = transparency_leader_lease_runtime_binding();
    assert_eq!(validate_wire_binding(&binding), Ok(()));
    assert_eq!(
        validate_observation(&binding, &observation(&binding)),
        Ok(())
    );

    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_transparency_leader_lease_provider(Arc::new(
            ServerTestTransparencyLeaderLeaseProvider::exact(),
        ));
    assert_eq!(
        validate_exact_backend_set(std::slice::from_ref(&binding), &backends),
        Ok(())
    );
    make_server_observation(&binding, &backends)
        .expect("stable exact leader-lease provider qualifies twice");
    assert_eq!(
        validate_exact_backend_set(&[], &backends),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );
    assert_eq!(
        validate_exact_backend_set(
            std::slice::from_ref(&binding),
            &RuntimeProviderBrokerBackendsV1::new(),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );

    let mut confused = binding.clone();
    confused.governance_request_ingress_binding =
        Some(governance_request_ingress_binding_to_wire(
            server_test_request_ingress_binding(TEST_SIGNER_KEY),
        ));
    assert_eq!(
        validate_wire_binding(&confused),
        Err(BrokerError::BindingMismatch)
    );

    let substituted = RuntimeProviderBrokerBackendsV1::new()
        .with_transparency_leader_lease_provider(Arc::new(
            ServerTestTransparencyLeaderLeaseProvider::substituted(),
        ));
    assert_eq!(
        make_server_observation(&binding, &substituted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );

    let drifted = RuntimeProviderBrokerBackendsV1::new()
        .with_transparency_leader_lease_provider(Arc::new(
            ServerTestTransparencyLeaderLeaseProvider::drifting(),
        ));
    assert_eq!(
        make_server_observation(&binding, &drifted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
}

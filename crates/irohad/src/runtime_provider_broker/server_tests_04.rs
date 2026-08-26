use crate::runtime_provider_broker::api::{
    ConsensusSignerProviderQualificationV1, GlobalBeaconPartialSignerBrokerBackendErrorV1,
    GlobalBeaconPartialSignerBrokerBackendV1,
    ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    ParliamentTlePartialReleaseSignerBrokerBackendV1,
};

#[test]
fn fenced_privacy_head_reader_binding_is_exact_and_drift_checked() {
    let binding = privacy_reader_binding();
    let publisher_binding = privacy_publisher_binding();
    assert_ne!(binding.slot, publisher_binding.slot);
    assert_eq!(binding.handle, publisher_binding.handle);
    assert_eq!(binding.revision, publisher_binding.revision);
    assert_eq!(binding.policy_digest, publisher_binding.policy_digest);
    assert_eq!(validate_wire_binding(&binding), Ok(()));
    assert_eq!(
        validate_observation(&binding, &observation(&binding)),
        Ok(())
    );
    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_fenced_privacy_head_reader(Arc::new(ServerTestFencedPrivacyHeadReader::exact()));
    assert_backend_fixture(
        &binding,
        &backends,
        "stable exact fenced privacy head reader qualifies twice",
    );
    let substituted = RuntimeProviderBrokerBackendsV1::new().with_fenced_privacy_head_reader(
        Arc::new(ServerTestFencedPrivacyHeadReader::substituted()),
    );
    assert_eq!(
        make_server_observation(&binding, &substituted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
    let drifted = RuntimeProviderBrokerBackendsV1::new()
        .with_fenced_privacy_head_reader(Arc::new(ServerTestFencedPrivacyHeadReader::drifting()));
    assert_eq!(
        make_server_observation(&binding, &drifted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
}
#[test]
fn fenced_privacy_head_reader_operation_is_canonical_bounded_and_exact() {
    thread::Builder::new()
        .name("fenced-privacy-head-reader-operation".to_owned())
        .stack_size(8 * 1024 * 1024)
        .spawn(fenced_privacy_head_reader_operation_is_canonical_bounded_and_exact_inner)
        .expect("spawn fenced privacy head-reader test thread")
        .join()
        .expect("join fenced privacy head-reader test thread");
}
fn fenced_privacy_head_reader_operation_is_canonical_bounded_and_exact_inner() {
    assert!(operation_is_known(
        OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1
    ));
    assert_eq!(
        operation_frame_limit(OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1),
        MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1
    );
    let binding = privacy_reader_binding();
    let reader = Arc::new(ServerTestFencedPrivacyHeadReader::exact());
    let backends =
        RuntimeProviderBrokerBackendsV1::new().with_fenced_privacy_head_reader(reader.clone());
    let observed = make_server_observation(&binding, &backends)
        .expect("qualify stable fenced privacy head reader");
    let state = singleton_state(
        "fenced-privacy-head-reader-test-chain",
        binding.clone(),
        observed.clone(),
        backends,
    );
    let (head, publication) = sample_fenced_privacy_head_evidence();
    let required_ancestors = vec![head];
    let required_publications = vec![publication];
    let wire = FencedPrivacyHeadReadRequestWireV1::from_required_evidence(
        &required_ancestors,
        &required_publications,
    );
    assert_eq!(
        wire.to_required_evidence()
            .expect("reconstruct exact fenced head evidence"),
        (required_ancestors.clone(), required_publications.clone())
    );
    let request = make_operation_request(
        [0xE1; 32],
        1,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1,
        encode_canonical(&wire, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
            .expect("encode fenced head read"),
    )
    .expect("construct fenced head read operation");
    validate_operation_request(&request).expect("validate fenced head read");
    let result = dispatch_server_operation(&state, &request).expect("dispatch fenced head read");
    let proof = decode_canonical::<FencedTransparencyHeadAncestryProofWireV1>(
        &result,
        MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1,
    )
    .and_then(|wire| wire.to_proof(&required_ancestors, &required_publications))
    .expect("decode exact authenticated ancestry proof");
    assert_eq!(proof.authoritative_head(), Some(head));
    assert_eq!(proof.verified_ancestors(), required_ancestors);
    assert_eq!(proof.verified_publications(), required_publications);
    assert_eq!(reader.read_calls.load(Ordering::SeqCst), 1);
    let genesis_wire = FencedPrivacyHeadReadRequestWireV1::from_required_evidence(&[], &[]);
    let genesis_request = make_operation_request(
        [0xE2; 32],
        2,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1,
        encode_canonical(&genesis_wire, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
            .expect("encode authenticated genesis read"),
    )
    .expect("construct authenticated genesis operation");
    let genesis_result = dispatch_server_operation(&state, &genesis_request)
        .expect("dispatch authenticated genesis read");
    let genesis_proof = decode_canonical::<FencedTransparencyHeadAncestryProofWireV1>(
        &genesis_result,
        MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1,
    )
    .and_then(|wire| wire.to_proof(&[], &[]))
    .expect("decode authenticated genesis proof");
    assert_eq!(genesis_proof.authoritative_head(), None);
    assert_eq!(reader.read_calls.load(Ordering::SeqCst), 2);
    let mut malformed = wire.clone();
    malformed.version = node::FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1.wrapping_add(1);
    let malformed_request = make_operation_request(
        [0xE3; 32],
        3,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1,
        encode_canonical(&malformed, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
            .expect("encode malformed fenced head read"),
    )
    .expect("construct malformed fenced head read operation");
    assert_eq!(
        validate_operation_request(&malformed_request),
        Err(BrokerError::Rejected)
    );
    assert_eq!(reader.read_calls.load(Ordering::SeqCst), 2);
    let excessive = FencedPrivacyHeadReadRequestWireV1 {
        version: node::FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
        required_ancestors: vec![
            FencedTransparencyTargetHeadWireV1::from_head(head);
            MAX_FENCED_PRIVACY_HEAD_EVIDENCE_ITEMS_V1 + 1
        ],
        required_publications: Vec::new(),
    };
    let excessive_request = make_operation_request(
        [0xE4; 32],
        4,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_FENCED_PRIVACY_READ_HEAD_WITH_ANCESTRY_V1,
        encode_canonical(&excessive, MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
            .expect("encode excessive fenced head evidence"),
    )
    .expect("construct excessive fenced head operation");
    assert_eq!(
        validate_operation_request(&excessive_request),
        Err(BrokerError::Rejected)
    );
    assert_eq!(reader.read_calls.load(Ordering::SeqCst), 2);
    let substituted_reader = Arc::new(ServerTestFencedPrivacyHeadReader::substituted_proof());
    let substituted_backends = RuntimeProviderBrokerBackendsV1::new()
        .with_fenced_privacy_head_reader(substituted_reader.clone());
    let substituted_observed = make_server_observation(&binding, &substituted_backends)
        .expect("qualify head reader that substitutes proof evidence");
    let substituted_state = singleton_state(
        "fenced-privacy-substituted-head-proof-test-chain",
        binding.clone(),
        substituted_observed,
        substituted_backends,
    );
    assert_eq!(
        dispatch_server_operation(&substituted_state, &request),
        Err(BrokerError::Rejected)
    );
    assert_eq!(substituted_reader.read_calls.load(Ordering::SeqCst), 1);
    let drift_reader = Arc::new(ServerTestFencedPrivacyHeadReader::drifting_after_read());
    let drift_backends = RuntimeProviderBrokerBackendsV1::new()
        .with_fenced_privacy_head_reader(drift_reader.clone());
    let drift_observed = make_server_observation(&binding, &drift_backends)
        .expect("head reader is stable before its authenticated read");
    let drift_state = singleton_state(
        "fenced-privacy-head-read-drift-test-chain",
        binding,
        drift_observed,
        drift_backends,
    );
    assert_eq!(
        dispatch_server_operation(&drift_state, &request),
        Err(BrokerError::StaleOrRevoked)
    );
    assert_eq!(drift_reader.read_calls.load(Ordering::SeqCst), 1);
    let unit = encode_canonical(&(), MAX_FENCED_PRIVACY_HEAD_FRAME_BYTES_V1)
        .expect("encode payload-free head-read failure");
    assert_eq!(
        make_operation_response(
            &request,
            STATUS_CONFLICT_V1,
            unit.clone(),
            &state.network_id,
        ),
        Err(BrokerError::Protocol)
    );
    assert_eq!(
        make_operation_response(
            &request,
            STATUS_AMBIGUOUS_V1,
            unit.clone(),
            &state.network_id,
        ),
        Err(BrokerError::Protocol)
    );
    assert!(
        make_operation_response(&request, STATUS_UNAVAILABLE_V1, unit, &state.network_id,).is_ok()
    );
}
#[test]
fn por_replay_archive_binding_is_exact_bounded_and_drift_checked() {
    let binding = replay_archive_binding();
    let exact = por_replay_archive_exact_binding(&binding).expect("exact replay-archive binding");
    let (limits, bounds) = por_replay_archive_configured_proof_bounds(&binding)
        .expect("bounded replay-archive proof policy");
    assert_eq!(limits.max_successor_receipts, 1_024);
    assert_eq!(limits.max_successor_proof_bytes, 1_048_576);
    assert_eq!(bounds.max_successor_receipts(), 1_024);
    assert_eq!(bounds.max_successor_proof_bytes(), 1_048_576);
    assert_eq!(validate_wire_binding(&binding), Ok(()));
    assert_eq!(
        validate_observation(&binding, &observation(&binding)),
        Ok(())
    );
    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_por_finalized_replay_archive(Arc::new(ServerTestPorReplayArchive::exact(exact)));
    assert_eq!(
        validate_exact_backend_set(std::slice::from_ref(&binding), &backends),
        Ok(())
    );
    make_server_observation(&binding, &backends)
        .expect("stable exact replay archive qualifies twice");
    assert_eq!(
        validate_exact_backend_set(&[], &backends),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );
    let mut confused = binding.clone();
    confused.pop_credential_runtime_binding = pop_runtime_binding().pop_credential_runtime_binding;
    assert!(validate_wire_binding(&confused).is_err());
    let mut missing_limits = binding.clone();
    missing_limits.por_replay_archive_proof_limits = None;
    assert!(validate_wire_binding(&missing_limits).is_err());
    let mut zero_limits = binding.clone();
    zero_limits
        .por_replay_archive_proof_limits
        .as_mut()
        .expect("proof limits")
        .max_successor_receipts = 0;
    assert!(validate_wire_binding(&zero_limits).is_err());
    let mut excessive_limits = binding.clone();
    excessive_limits
        .por_replay_archive_proof_limits
        .as_mut()
        .expect("proof limits")
        .max_successor_proof_bytes = u64::try_from(MAX_POR_REPLAY_ARCHIVE_SUCCESSOR_PROOF_BYTES_V1)
        .expect("proof ceiling fits u64")
        + 1;
    assert!(validate_wire_binding(&excessive_limits).is_err());
    let later = node::PorFinalizedReplayArchiveBindingV1::try_new(
        exact.archive_id,
        exact.revision + 1,
        exact.policy_digest,
        exact.signing_public_key,
    )
    .expect("valid drifted replay-archive binding");
    let drifted = RuntimeProviderBrokerBackendsV1::new().with_por_finalized_replay_archive(
        Arc::new(ServerTestPorReplayArchive::drifting(exact, later)),
    );
    assert_eq!(
        make_server_observation(&binding, &drifted),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
}
#[test]
fn por_replay_archive_lookup_results_round_trip_found_and_absent() {
    let binding = replay_archive_binding();
    let exact = por_replay_archive_exact_binding(&binding).expect("exact replay-archive binding");
    let (limits, bounds) = por_replay_archive_configured_proof_bounds(&binding)
        .expect("bounded replay-archive proof policy");
    let record = por_replay_archive_record_fixture();
    let receipt_digest =
        node::PorFinalizedReplayArchiveReceiptV1::signing_digest(exact, &record, None)
            .expect("derive replay-archive receipt digest");
    let receipt = node::PorFinalizedReplayArchiveReceiptV1::try_new(
        exact,
        &record,
        None,
        test_signature(&receipt_digest),
    )
    .expect("authenticate replay-archive receipt");
    let found_request = PorReplayArchiveLookupRequestWireV1 {
        challenge_id: record.challenge_id(),
        expected_checkpoint_head: receipt,
        max_successor_receipts: limits.max_successor_receipts,
        max_successor_proof_bytes: limits.max_successor_proof_bytes,
    };
    let found = node::PorFinalizedReplayArchiveLookupV1::Found(Box::new(
        node::PorFinalizedReplayArchiveReadbackV1 {
            record: record.clone(),
            receipt,
            successor_receipts: Vec::new(),
        },
    ));
    let found_wire =
        por_replay_archive_lookup_to_wire(found.clone(), &found_request, exact, bounds)
            .expect("validate and encode found lookup");
    let found_bytes = encode_canonical(&found_wire, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
        .expect("encode found lookup wire");
    let found_decoded = decode_canonical::<PorReplayArchiveLookupOutcomeWireV1>(
        &found_bytes,
        MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1,
    )
    .expect("decode found lookup wire");
    assert_eq!(found_decoded, found_wire);
    assert_eq!(
        por_replay_archive_lookup_from_wire(&found_decoded, &found_request, &binding,),
        Ok(found)
    );
    let absent_challenge_id = [0xFA; 32];
    let absence_digest = node::PorFinalizedReplayArchiveAbsenceProofV1::signing_digest(
        exact,
        absent_challenge_id,
        receipt,
    )
    .expect("derive replay-archive absence digest");
    let absence = node::PorFinalizedReplayArchiveAbsenceProofV1::try_new(
        exact,
        absent_challenge_id,
        receipt,
        test_signature(&absence_digest),
    )
    .expect("authenticate replay-archive absence proof");
    let absent_request = PorReplayArchiveLookupRequestWireV1 {
        challenge_id: absent_challenge_id,
        expected_checkpoint_head: receipt,
        max_successor_receipts: limits.max_successor_receipts,
        max_successor_proof_bytes: limits.max_successor_proof_bytes,
    };
    let absent = node::PorFinalizedReplayArchiveLookupV1::Absent(Box::new(absence));
    let absent_wire =
        por_replay_archive_lookup_to_wire(absent.clone(), &absent_request, exact, bounds)
            .expect("validate and encode absent lookup");
    let absent_bytes = encode_canonical(&absent_wire, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
        .expect("encode absent lookup wire");
    let absent_decoded = decode_canonical::<PorReplayArchiveLookupOutcomeWireV1>(
        &absent_bytes,
        MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1,
    )
    .expect("decode absent lookup wire");
    assert_eq!(absent_decoded, absent_wire);
    assert_eq!(
        por_replay_archive_lookup_from_wire(&absent_decoded, &absent_request, &binding,),
        Ok(absent)
    );
    let mut substituted_request = absent_request;
    substituted_request.challenge_id = [0xFB; 32];
    assert_eq!(
        por_replay_archive_lookup_from_wire(&absent_decoded, &substituted_request, &binding,),
        Err(BrokerError::Rejected)
    );
}
#[test]
fn por_replay_archive_operations_are_bounded_and_append_is_ambiguous() {
    for operation in [
        OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1,
        OPERATION_POR_REPLAY_ARCHIVE_CURRENT_HEAD_V1,
        OPERATION_POR_REPLAY_ARCHIVE_APPEND_V1,
        OPERATION_POR_REPLAY_ARCHIVE_LOOKUP_V1,
    ] {
        assert!(operation_is_known(operation));
    }
    assert_eq!(
        operation_frame_limit(OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1),
        MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1
    );
    assert_eq!(
        operation_frame_limit(OPERATION_POR_REPLAY_ARCHIVE_CURRENT_HEAD_V1),
        MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1
    );
    assert_eq!(
        operation_frame_limit(OPERATION_POR_REPLAY_ARCHIVE_APPEND_V1),
        MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1
    );
    assert_eq!(
        operation_frame_limit(OPERATION_POR_REPLAY_ARCHIVE_LOOKUP_V1),
        MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1
    );
    let binding = replay_archive_binding();
    let exact = por_replay_archive_exact_binding(&binding).expect("exact replay-archive binding");
    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_por_finalized_replay_archive(Arc::new(ServerTestPorReplayArchive::exact(exact)));
    let observed =
        make_server_observation(&binding, &backends).expect("qualify stable replay archive");
    let state = singleton_state(
        "por-replay-archive-test-chain",
        binding.clone(),
        observed.clone(),
        backends,
    );
    for (request_id, operation) in [
        (1, OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1),
        (2, OPERATION_POR_REPLAY_ARCHIVE_CURRENT_HEAD_V1),
    ] {
        let request = make_operation_request(
            [0xA1; 32],
            request_id,
            binding.clone(),
            observed.metadata_digest,
            operation,
            encode_canonical(&(), MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
                .expect("encode replay-archive control request"),
        )
        .expect("construct replay-archive request");
        validate_operation_request(&request).expect("validate replay-archive control request");
        let result = dispatch_server_operation(&state, &request)
            .expect("dispatch replay-archive control request");
        if operation == OPERATION_POR_REPLAY_ARCHIVE_READINESS_V1 {
            decode_canonical::<()>(&result, MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1)
                .expect("decode readiness");
        } else {
            assert_eq!(
                decode_canonical::<Option<node::PorFinalizedReplayArchiveReceiptV1>>(
                    &result,
                    MAX_POR_REPLAY_ARCHIVE_CONTROL_FRAME_BYTES_V1
                )
                .expect("decode current head"),
                None
            );
        }
    }
    let invalid_append = PorReplayArchiveAppendRequestWireV1 {
        canonical_record: Vec::new(),
        expected_previous_head: None,
    };
    let append_request = make_operation_request(
        [0xA2; 32],
        3,
        binding.clone(),
        observed.metadata_digest,
        OPERATION_POR_REPLAY_ARCHIVE_APPEND_V1,
        encode_canonical(&invalid_append, MAX_POR_REPLAY_ARCHIVE_FRAME_BYTES_V1)
            .expect("encode malformed append request"),
    )
    .expect("construct malformed append request");
    assert_eq!(
        validate_operation_request(&append_request),
        Err(BrokerError::Rejected)
    );
    let unit =
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).expect("encode payload-free failure");
    assert!(
        make_operation_response(
            &append_request,
            STATUS_AMBIGUOUS_V1,
            unit.clone(),
            &state.network_id,
        )
        .is_ok(),
        "append is the only replay-archive operation allowed to be ambiguous"
    );
    let lookup_request = make_operation_request(
        [0xA3; 32],
        4,
        binding,
        observed.metadata_digest,
        OPERATION_POR_REPLAY_ARCHIVE_LOOKUP_V1,
        unit.clone(),
    )
    .expect("construct lookup envelope");
    assert_eq!(
        make_operation_response(
            &lookup_request,
            STATUS_AMBIGUOUS_V1,
            unit,
            &state.network_id,
        ),
        Err(BrokerError::Protocol)
    );
}
#[test]
fn gateway_and_pop_wires_enforce_bounds_and_mutation_ambiguity() {
    let acme_binding = runtime_binding(
        IrohaRuntimeProviderSlotV1::GatewayAcmeClient,
        SERVER_TEST_ACME_HANDLE,
    );
    let acme_order = GatewayAcmeOrderRequestWireV1 {
        hostnames: vec!["gateway.example.com".to_owned()],
        account_email: Some("ops@example.com".to_owned()),
        directory_url: "https://acme.example.com/directory".to_owned(),
        dns_provider_id: Some("route53:production".to_owned()),
        dns01: true,
        tls_alpn_01: true,
    };
    assert_eq!(validate_gateway_acme_order(&acme_order), Ok(()));
    let oversized_wildcard = format!(
        "*.{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(60),
    );
    let mut invalid_acme_order = acme_order.clone();
    invalid_acme_order.hostnames = vec![oversized_wildcard];
    assert_eq!(
        validate_gateway_acme_order(&invalid_acme_order),
        Err(BrokerError::Rejected)
    );
    let acme_request = validated_operation(
        acme_binding,
        OPERATION_GATEWAY_ACME_ORDER_CERTIFICATE_V1,
        encode_canonical(&acme_order, MAX_GATEWAY_ACME_FRAME_BYTES_V1)
            .expect("encode valid ACME order"),
    );
    let ambiguous = make_operation_response(
        &acme_request,
        STATUS_AMBIGUOUS_V1,
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).expect("encode empty ambiguous result"),
        &network_id(),
    )
    .expect("construct ACME ambiguous response");
    assert_eq!(
        validate_operation_response(&acme_request, &ambiguous, &network_id(),),
        Ok(())
    );
    let compliance_binding = runtime_binding(
        IrohaRuntimeProviderSlotV1::GatewayComplianceFeedTransport,
        SERVER_TEST_COMPLIANCE_HANDLE,
    );
    let compliance_request_wire = GatewayComplianceFetchRequestWireV1 {
        url: "https://feeds.example.com/catalog".to_owned(),
        pinned_addresses: vec![
            IpAddressWireV1::from("8.8.8.8".parse::<std::net::IpAddr>().expect("public IPv4")),
            IpAddressWireV1::from("9.9.9.9".parse::<std::net::IpAddr>().expect("public IPv4")),
        ],
        connect_timeout: DurationWireV1::from_duration(Duration::from_secs(5)),
        total_timeout: DurationWireV1::from_duration(Duration::from_secs(10)),
        max_encoded_bytes: 1024,
    };
    validate_gateway_compliance_fetch_request(&compliance_request_wire)
        .expect("canonical pinned public compliance request");
    let mut private_address = compliance_request_wire.clone();
    private_address.pinned_addresses = vec![IpAddressWireV1::from(
        "127.0.0.1".parse::<std::net::IpAddr>().expect("loopback"),
    )];
    assert_eq!(
        validate_gateway_compliance_fetch_request(&private_address),
        Err(BrokerError::Rejected)
    );
    let compliance_request = validated_operation(
        compliance_binding,
        OPERATION_GATEWAY_COMPLIANCE_FETCH_V1,
        encode_canonical(
            &compliance_request_wire,
            MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1,
        )
        .expect("encode valid compliance request"),
    );
    let invalid_ambiguity = operation_response(
        &compliance_request,
        STATUS_AMBIGUOUS_V1,
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).expect("encode empty ambiguous result"),
    );
    let invalid_frame = encode_frame(
        FRAME_KIND_OPERATION_RESPONSE_V1,
        &invalid_ambiguity,
        MAX_GATEWAY_COMPLIANCE_FRAME_BYTES_V1,
    );
    assert!(
        invalid_frame.is_ok(),
        "the canonical invalid-status fixture is within the compliance wire cap"
    );
    assert_eq!(
        validate_operation_response(&compliance_request, &invalid_ambiguity, &network_id(),),
        Err(BrokerError::Protocol)
    );
    let pop_binding = pop_runtime_binding();
    let exact = pop_binding
        .pop_credential_runtime_binding
        .as_ref()
        .expect("PoP exact metadata");
    let pop_resolve = validated_operation(
        pop_binding.clone(),
        OPERATION_POP_RUNTIME_OPEN_V1,
        encode_canonical(exact, MAX_POP_RUNTIME_FRAME_BYTES_V1).expect("encode exact PoP binding"),
    );
    let ambiguous_resolve = make_operation_response(
        &pop_resolve,
        STATUS_AMBIGUOUS_V1,
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).expect("encode empty ambiguous result"),
        &network_id(),
    )
    .expect("construct PoP resolve ambiguity");
    assert_eq!(
        validate_operation_response(&pop_resolve, &ambiguous_resolve, &network_id(),),
        Ok(())
    );
    let pop_wrap = validated_operation(
        pop_binding,
        OPERATION_POP_WALLET_WRAP_DEK_V1,
        encode_canonical(
            &PopWalletWrapDekRequestWireV1 {
                context: [0x91; 32],
                dek: [0x92; 32],
            },
            MAX_POP_RUNTIME_FRAME_BYTES_V1,
        )
        .expect("encode PoP KMS wrap request"),
    );
    let ambiguous_wrap = make_operation_response(
        &pop_wrap,
        STATUS_AMBIGUOUS_V1,
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1).expect("encode empty ambiguous result"),
        &network_id(),
    )
    .expect("construct PoP wrap ambiguity");
    assert_eq!(
        validate_operation_response(&pop_wrap, &ambiguous_wrap, &network_id(),),
        Ok(())
    );
}
#[test]
fn reputation_runtime_bindings_and_observations_are_exactly_slot_shaped() {
    let slots = [
        (
            IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter,
            32,
        ),
        (IrohaRuntimeProviderSlotV1::ReputationThresholdSigner, 33),
        (IrohaRuntimeProviderSlotV1::ReputationGovernanceDag, 34),
        (IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint, 50),
    ];
    for (slot, wire_id) in slots {
        assert_eq!(slot.wire_id(), wire_id);
        let catalog = reputation_catalog(slot);
        let binding = reputation_binding(slot);
        assert_eq!(validate_wire_binding(&binding), Ok(()), "{slot:?}");
        let observed = observation(&binding);
        assert_eq!(
            validate_observation(&binding, &observed),
            Ok(()),
            "{slot:?}"
        );
        assert!(matches!(
            prepare_server_state(&catalog, RuntimeProviderBrokerBackendsV1::new()),
            Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
        ));
        prepare_server_state(&catalog, reputation_backends(slot))
            .unwrap_or_else(|error| panic!("accept exact {slot:?} backend: {error:?}"));
        assert!(matches!(
            prepare_server_state(&catalog, reputation_runtime_substituted_backends(slot),),
            Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
        ));
        let mut zero_revision = binding.clone();
        zero_revision.revision = Some(0);
        assert_eq!(
            validate_wire_binding(&zero_revision),
            Err(BrokerError::BindingMismatch),
            "{slot:?}"
        );
        let mut zero_digest = binding.clone();
        zero_digest.policy_digest = Some([0; 32]);
        assert_eq!(
            validate_wire_binding(&zero_digest),
            Err(BrokerError::BindingMismatch),
            "{slot:?}"
        );
        let mut role_confused = binding;
        role_confused.stream_token_signer_public_key = Some(TEST_SIGNER_KEY);
        assert_eq!(
            validate_wire_binding(&role_confused),
            Err(BrokerError::BindingMismatch),
            "{slot:?}"
        );
    }
}
#[test]
fn reputation_checkpoint_load_is_bounded_and_slot_exact() {
    assert_eq!(OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1, 101);
    assert_eq!(
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_COMPARE_AND_SWAP_V1,
        102
    );
    for operation in [
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_COMPARE_AND_SWAP_V1,
    ] {
        assert!(operation_is_known(operation));
        assert_eq!(
            operation_frame_limit(operation),
            MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1
        );
        assert_eq!(
            operation_decode_policy(operation),
            REPUTATION_DECODE_POLICY_V1
        );
    }
    let slot = IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint;
    let catalog = reputation_catalog(slot);
    let state = prepare_server_state(&catalog, reputation_backends(slot))
        .expect("prepare exact reputation checkpoint backend");
    let binding = state.catalog[0].clone();
    let mut wrong_profile = binding.clone();
    wrong_profile.revision = Some(2);
    assert_eq!(
        validate_wire_binding(&wrong_profile),
        Err(BrokerError::BindingMismatch)
    );
    let payload = encode_canonical(
        &CHECKPOINT_LOAD_REQUEST_VERSION_V1,
        MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
    )
    .expect("encode checkpoint load request");
    assert_eq!(
        payload.len(),
        norito::core::Header::SIZE + core::mem::size_of::<u8>()
    );
    let request = make_operation_request(
        TEST_SESSION_ID,
        1,
        binding.clone(),
        state.observations[0].metadata_digest,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1,
        payload,
    )
    .expect("seal checkpoint load request");
    assert_ne!(request.session_id, [0; 32]);
    assert_ne!(request.request_id, 0);
    assert_ne!(request.provider_metadata_digest, [0; 32]);
    assert_eq!(
        operation_payload_digest(&request.payload),
        request.payload_digest,
        "checkpoint load payload digest"
    );
    validate_wire_binding(&request.binding).expect("validate checkpoint load request binding");
    let request_fields = OperationRequestFieldsV1 {
        session_id: request.session_id,
        request_id: request.request_id,
        binding: request.binding.clone(),
        provider_metadata_digest: request.provider_metadata_digest,
        operation: request.operation,
        payload_digest: request.payload_digest,
        payload_len: u64::try_from(request.payload.len())
            .expect("checkpoint load payload length fits u64"),
    };
    assert_eq!(
        operation_request_digest(&request_fields).expect("digest checkpoint load request fields"),
        request.request_digest,
        "checkpoint load request digest"
    );
    assert_eq!(
        decode_canonical::<u8>(
            &request.payload,
            MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
        )
        .expect("decode checkpoint load payload"),
        CHECKPOINT_LOAD_REQUEST_VERSION_V1
    );
    validate_operation_payload(&request, None, &network_id())
        .expect("validate checkpoint load operation payload");
    validate_operation_request(&request).expect("validate checkpoint load request");
    let result = dispatch_server_operation(&state, &request).expect("dispatch checkpoint load");
    assert_eq!(
        decode_canonical::<Option<Vec<u8>>>(
            &result,
            MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
        )
        .expect("decode empty checkpoint head"),
        None
    );
    validate_operation_result(&request, STATUS_OK_V1, &result, &state.network_id)
        .expect("validate checkpoint load result");
    let unsupported_version = CHECKPOINT_LOAD_REQUEST_VERSION_V1 ^ u8::MAX;
    let alternate_version = make_operation_request(
        TEST_SESSION_ID,
        2,
        binding.clone(),
        state.observations[0].metadata_digest,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1,
        encode_canonical(
            &unsupported_version,
            MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
        )
        .expect("encode alternate checkpoint load version"),
    )
    .expect("seal alternate checkpoint load version");
    assert_eq!(
        validate_operation_request(&alternate_version),
        Err(BrokerError::Rejected)
    );
    let mut trailing_payload = request.payload.clone();
    trailing_payload.push(0);
    let trailing = make_operation_request(
        TEST_SESSION_ID,
        3,
        binding.clone(),
        state.observations[0].metadata_digest,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1,
        trailing_payload,
    )
    .expect("seal trailing checkpoint load payload");
    assert_eq!(
        validate_operation_request(&trailing),
        Err(BrokerError::Protocol)
    );
    let malformed_compare = encode_canonical(
        &ReputationJournalCheckpointCompareAndSwapRequestWireV1 {
            expected_revision: Some([0; 32]),
            next_record: vec![0],
        },
        MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
    )
    .expect("encode malformed checkpoint CAS");
    let malformed_compare = make_operation_request(
        TEST_SESSION_ID,
        4,
        binding,
        state.observations[0].metadata_digest,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_COMPARE_AND_SWAP_V1,
        malformed_compare,
    )
    .expect("seal malformed checkpoint CAS");
    assert_eq!(
        validate_operation_request(&malformed_compare),
        Err(BrokerError::Rejected)
    );
    let ambiguous_compare = make_operation_response(
        &malformed_compare,
        STATUS_AMBIGUOUS_V1,
        encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
            .expect("encode empty ambiguous checkpoint result"),
        &state.network_id,
    )
    .expect("construct checkpoint CAS ambiguity");
    assert_eq!(
        validate_operation_response(&malformed_compare, &ambiguous_compare, &state.network_id,),
        Ok(())
    );
    let journal_binding =
        reputation_binding(IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter);
    let cross_slot = make_operation_request(
        TEST_SESSION_ID,
        5,
        journal_binding.clone(),
        observation(&journal_binding).metadata_digest,
        OPERATION_REPUTATION_JOURNAL_CHECKPOINT_LOAD_V1,
        encode_canonical(
            &CHECKPOINT_LOAD_REQUEST_VERSION_V1,
            MAX_REPUTATION_JOURNAL_CHECKPOINT_FRAME_BYTES_V1,
        )
        .expect("encode cross-slot checkpoint load"),
    )
    .expect("seal cross-slot checkpoint load");
    assert_eq!(
        validate_operation_request(&cross_slot),
        Err(BrokerError::BindingMismatch)
    );
}
#[test]
fn reputation_runtime_operations_are_strict_and_reconcile_exact_keys() {
    use test_reputation::ReputationJournalTransactionSubmitOutcomeV1;

    assert_eq!(OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1, 16);
    assert_eq!(OPERATION_REPUTATION_JOURNAL_SUBMIT_V1, 17);
    assert_eq!(OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1, 18);
    assert_eq!(OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1, 19);
    for operation in 16..=19 {
        assert!(operation_is_known(operation));
        assert_eq!(
            operation_frame_limit(operation),
            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1
        );
    }
    let authority_keypair = KeyPair::try_from_seed(vec![0x88; 32], Algorithm::Ed25519)
        .expect("derive reputation broker authority");
    let authority = AccountId::new(authority_keypair.public_key().clone());
    let supports_payload = encode_canonical(
        &ReputationJournalSupportsAuthorityRequestWireV1 {
            authority: authority.clone(),
        },
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )
    .expect("encode supports-authority request");
    let journal_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter),
        reputation_backends(IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter),
    )
    .expect("prepare exact reputation journal backend");
    assert_eq!(
        ensure_reputation_session_network(&network_id_from(0x16), &journal_state.network_id,),
        Err(BrokerError::BindingMismatch),
        "the journal submitter cannot be used as a cross-network deputy"
    );
    let supports = reputation_request(
        &journal_state,
        1,
        OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1,
        supports_payload.clone(),
    );
    validate_operation_request(&supports).expect("validate supports-authority request");
    let supports_result = dispatch_server_operation(&journal_state, &supports)
        .expect("dispatch supports-authority request");
    validate_operation_result(
        &supports,
        STATUS_OK_V1,
        &supports_result,
        &journal_state.network_id,
    )
    .expect("validate supports-authority result");
    assert!(
        decode_canonical::<bool>(&supports_result, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,)
            .expect("decode supports-authority result")
    );
    let drifting_journal_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter),
        RuntimeProviderBrokerBackendsV1::new().with_reputation_journal_transaction_submitter(
            Arc::new(ServerTestReputationJournalSubmitter::drifting_after_operation()),
        ),
    )
    .expect("prepare drifting reputation journal backend");
    let drifting_supports = reputation_request(
        &drifting_journal_state,
        1,
        OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1,
        supports_payload.clone(),
    );
    validate_operation_request(&drifting_supports)
        .expect("validate drifting supports-authority request");
    assert_eq!(
        dispatch_server_operation(&drifting_journal_state, &drifting_supports),
        Err(BrokerError::StaleOrRevoked),
        "read-only post-operation qualification drift is definitive"
    );
    let mut trailing_supports = supports_payload.clone();
    trailing_supports.push(0);
    let trailing = reputation_request(
        &journal_state,
        2,
        OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1,
        trailing_supports,
    );
    assert!(validate_operation_request(&trailing).is_err());
    let malformed_submit = ReputationJournalTransactionRequestWireV1 {
        sequence: 0,
        network_id: network_id(),
        authority: authority.clone(),
        event_id: iroha_data_model::sorafs::reputation::ReputationJournalEventIdV1::ZERO,
        source_id: iroha_data_model::sorafs::reputation::ReputationJournalSourceIdV1::ZERO,
        attempt: 0,
        idempotency_key: [0; 32],
        instruction_kind: 0,
        canonical_instruction: Vec::new(),
    };
    let malformed_submit = reputation_request(
        &journal_state,
        3,
        OPERATION_REPUTATION_JOURNAL_SUBMIT_V1,
        encode_canonical(&malformed_submit, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
            .expect("encode malformed reputation journal submit"),
    );
    assert_eq!(
        validate_operation_request(&malformed_submit),
        Err(BrokerError::Rejected)
    );
    for outcome in [
        ReputationJournalTransactionSubmitOutcomeV1::Queued {
            receipt: [0x71; 32],
        },
        ReputationJournalTransactionSubmitOutcomeV1::NotQueued {
            receipt: [0x72; 32],
        },
        ReputationJournalTransactionSubmitOutcomeV1::Ambiguous {
            receipt: [0x73; 32],
        },
    ] {
        let wire = reputation_journal_submit_result_to_wire(outcome)
            .expect("encode reputation journal result");
        assert_eq!(
            reputation_journal_submit_result_from_wire(wire),
            Ok(outcome)
        );
    }
    assert_eq!(
        reputation_journal_submit_result_from_wire(
            ReputationJournalTransactionSubmitResultWireV1 {
                outcome: 1,
                receipt: [0; 32],
            },
        ),
        Err(BrokerError::Rejected)
    );
    let threshold_backend = Arc::new(ServerTestReputationThresholdSigner::exact());
    let threshold_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationThresholdSigner),
        RuntimeProviderBrokerBackendsV1::new()
            .with_reputation_threshold_signer(threshold_backend.clone()),
    )
    .expect("prepare exact reputation threshold signer");
    let threshold_request = threshold_request();
    let threshold_payload = encode_canonical(
        &reputation_threshold_request_to_wire(&threshold_request)
            .expect("project reputation threshold request"),
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )
    .expect("encode reputation threshold request");
    let mut cross_network_threshold_request = threshold_request.clone();
    cross_network_threshold_request.material.network_id = network_id_from(0x16);
    cross_network_threshold_request.material_digest = reputation_hash_canonical(
        b"sorafs-reputation-unsigned-material-delivery-v1",
        &cross_network_threshold_request.material,
    )
    .expect("digest cross-network reputation threshold material");
    cross_network_threshold_request.idempotency_key = reputation_publication_idempotency_key(
        b"sorafs-reputation-threshold-signing-operation-v1",
        cross_network_threshold_request.sequence,
        cross_network_threshold_request.material_digest,
        None,
    )
    .expect("derive cross-network threshold idempotency key");
    let cross_network_threshold = reputation_request(
        &threshold_state,
        3,
        OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1,
        encode_canonical(
            &reputation_threshold_request_to_wire(&cross_network_threshold_request)
                .expect("project cross-network reputation threshold request"),
            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
        )
        .expect("encode cross-network reputation threshold request"),
    );
    validate_operation_request(&cross_network_threshold)
        .expect("cross-network threshold request is otherwise canonical");
    assert_eq!(
        dispatch_server_operation(&threshold_state, &cross_network_threshold),
        Err(BrokerError::BindingMismatch),
        "the threshold signer cannot be used as a cross-network deputy"
    );
    for request_id in [1, 2] {
        let operation = reputation_request(
            &threshold_state,
            request_id,
            OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1,
            threshold_payload.clone(),
        );
        validate_operation_request(&operation).expect("validate reputation threshold request");
        let result = dispatch_server_operation(&threshold_state, &operation)
            .expect("reconcile reputation threshold request");
        validate_operation_result(
            &operation,
            STATUS_OK_V1,
            &result,
            &threshold_state.network_id,
        )
        .expect("validate pending threshold result");
        assert_eq!(
            decode_canonical::<ReputationReconcileResultWireV1>(
                &result,
                MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
            )
            .expect("decode pending threshold result"),
            ReputationReconcileResultWireV1 {
                outcome: 0,
                canonical_result: Vec::new(),
                failure_receipt: [0; 32],
            }
        );
    }
    assert_eq!(
        threshold_backend
            .reconciled_keys
            .lock()
            .expect("threshold reconciliation keys")
            .as_slice(),
        &[
            threshold_request.idempotency_key,
            threshold_request.idempotency_key,
        ],
        "reconciliation retries preserve the exact operation key"
    );
    let drifting_threshold_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationThresholdSigner),
        RuntimeProviderBrokerBackendsV1::new().with_reputation_threshold_signer(Arc::new(
            ServerTestReputationThresholdSigner::drifting_after_operation(),
        )),
    )
    .expect("prepare drifting reputation threshold signer");
    let drifting_threshold = reputation_request(
        &drifting_threshold_state,
        1,
        OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1,
        threshold_payload.clone(),
    );
    validate_operation_request(&drifting_threshold).expect("validate drifting threshold request");
    assert_eq!(
        dispatch_server_operation(&drifting_threshold_state, &drifting_threshold),
        Err(BrokerError::Ambiguous),
        "post-dispatch qualification drift is an ambiguous mutation"
    );
    let governance_backend = Arc::new(ServerTestReputationGovernanceDag::exact());
    let governance_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationGovernanceDag),
        RuntimeProviderBrokerBackendsV1::new()
            .with_reputation_governance_dag(governance_backend.clone()),
    )
    .expect("prepare exact reputation Governance DAG");
    let governance_request = governance_request();
    let governance_payload = encode_canonical(
        &reputation_governance_request_to_wire(&governance_request)
            .expect("project reputation governance request"),
        MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
    )
    .expect("encode reputation governance request");
    for request_id in [1, 2] {
        let operation = reputation_request(
            &governance_state,
            request_id,
            OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1,
            governance_payload.clone(),
        );
        validate_operation_request(&operation).expect("validate reputation governance request");
        let result = dispatch_server_operation(&governance_state, &operation)
            .expect("reconcile reputation governance request");
        validate_operation_result(
            &operation,
            STATUS_OK_V1,
            &result,
            &governance_state.network_id,
        )
        .expect("validate pending governance result");
    }
    assert_eq!(
        governance_backend
            .reconciled_keys
            .lock()
            .expect("governance reconciliation keys")
            .as_slice(),
        &[
            governance_request.idempotency_key,
            governance_request.idempotency_key,
        ],
        "publication reconciliation preserves the exact operation key"
    );
    let drifting_governance_state = prepare_server_state(
        &reputation_catalog(IrohaRuntimeProviderSlotV1::ReputationGovernanceDag),
        RuntimeProviderBrokerBackendsV1::new().with_reputation_governance_dag(Arc::new(
            ServerTestReputationGovernanceDag::drifting_after_operation(),
        )),
    )
    .expect("prepare drifting reputation Governance DAG");
    let drifting_governance = reputation_request(
        &drifting_governance_state,
        1,
        OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1,
        governance_payload.clone(),
    );
    validate_operation_request(&drifting_governance).expect("validate drifting governance request");
    assert_eq!(
        dispatch_server_operation(&drifting_governance_state, &drifting_governance),
        Err(BrokerError::Ambiguous),
        "post-publication qualification drift is ambiguous"
    );
    let threshold_binding =
        reputation_binding(IrohaRuntimeProviderSlotV1::ReputationThresholdSigner);
    let cross_slot = make_operation_request(
        TEST_SESSION_ID,
        9,
        threshold_binding.clone(),
        observation(&threshold_binding).metadata_digest,
        OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1,
        governance_payload,
    )
    .expect("seal cross-slot reputation request");
    assert_eq!(
        validate_operation_request(&cross_slot),
        Err(BrokerError::BindingMismatch)
    );
    let mut trailing_threshold = threshold_payload;
    trailing_threshold.push(0);
    let trailing_threshold = make_operation_request(
        TEST_SESSION_ID,
        10,
        threshold_binding.clone(),
        observation(&threshold_binding).metadata_digest,
        OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1,
        trailing_threshold,
    )
    .expect("seal trailing threshold request");
    assert!(validate_operation_request(&trailing_threshold).is_err());
}
#[test]
fn reputation_qualification_results_are_exactly_bound() {
    for (slot, request_id) in [
        (
            IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter,
            1,
        ),
        (IrohaRuntimeProviderSlotV1::ReputationThresholdSigner, 2),
        (IrohaRuntimeProviderSlotV1::ReputationGovernanceDag, 3),
        (IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint, 4),
    ] {
        let binding = reputation_binding(slot);
        let operation = make_operation_request(
            TEST_SESSION_ID,
            request_id,
            binding.clone(),
            observation(&binding).metadata_digest,
            OPERATION_QUALIFY_V1,
            encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
                .expect("encode qualification request"),
        )
        .expect("seal reputation qualification operation");
        validate_operation_request(&operation)
            .expect("validate reputation qualification operation");
        let exact = QualificationResultWireV1 {
            revision: binding.revision.expect("qualification revision"),
            policy_digest: binding.policy_digest.expect("qualification policy digest"),
        };
        let exact = encode_canonical(&exact, MAX_OPERATION_FRAME_BYTES_V1)
            .expect("encode exact reputation qualification");
        assert_eq!(
            validate_operation_result(&operation, STATUS_OK_V1, &exact, &network_id(),),
            Ok(()),
            "{slot:?}"
        );
        let stale = QualificationResultWireV1 {
            revision: binding
                .revision
                .expect("qualification revision")
                .saturating_add(1),
            policy_digest: binding.policy_digest.expect("qualification policy digest"),
        };
        let stale = encode_canonical(&stale, MAX_OPERATION_FRAME_BYTES_V1)
            .expect("encode stale reputation qualification");
        assert_eq!(
            validate_operation_result(&operation, STATUS_OK_V1, &stale, &network_id(),),
            Err(BrokerError::Protocol),
            "{slot:?}"
        );
        let mut trailing = exact;
        trailing.push(0);
        assert_eq!(
            validate_operation_result(&operation, STATUS_OK_V1, &trailing, &network_id(),),
            Err(BrokerError::Protocol),
            "{slot:?}"
        );
    }
}
#[test]
fn reputation_runtime_result_shapes_and_ambiguity_are_exact() {
    let threshold_request = threshold_request();
    let threshold_binding =
        reputation_binding(IrohaRuntimeProviderSlotV1::ReputationThresholdSigner);
    let threshold_operation = make_operation_request(
        TEST_SESSION_ID,
        1,
        threshold_binding.clone(),
        observation(&threshold_binding).metadata_digest,
        OPERATION_REPUTATION_THRESHOLD_RECONCILE_V1,
        encode_canonical(
            &reputation_threshold_request_to_wire(&threshold_request)
                .expect("project threshold request"),
            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
        )
        .expect("encode threshold request"),
    )
    .expect("seal threshold operation");
    validate_operation_request(&threshold_operation).expect("validate threshold operation");
    let pending = ReputationReconcileResultWireV1 {
        outcome: 0,
        canonical_result: Vec::new(),
        failure_receipt: [0; 32],
    };
    let pending = encode_canonical(&pending, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
        .expect("encode pending reconciliation");
    validate_operation_result(&threshold_operation, STATUS_OK_V1, &pending, &network_id())
        .expect("accept canonical pending result");
    for invalid in [
        ReputationReconcileResultWireV1 {
            outcome: 0,
            canonical_result: Vec::new(),
            failure_receipt: [0x61; 32],
        },
        ReputationReconcileResultWireV1 {
            outcome: 1,
            canonical_result: Vec::new(),
            failure_receipt: [0; 32],
        },
        ReputationReconcileResultWireV1 {
            outcome: 2,
            canonical_result: Vec::new(),
            failure_receipt: [0; 32],
        },
    ] {
        let invalid = encode_canonical(&invalid, MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1)
            .expect("encode invalid reconciliation result");
        assert_eq!(
            validate_operation_result(&threshold_operation, STATUS_OK_V1, &invalid, &network_id(),),
            Err(BrokerError::Protocol)
        );
    }
    let unit = encode_canonical(&(), MAX_OPERATION_FRAME_BYTES_V1)
        .expect("encode payload-free broker status");
    assert_eq!(
        validate_operation_result(
            &threshold_operation,
            STATUS_AMBIGUOUS_V1,
            &unit,
            &network_id(),
        ),
        Ok(())
    );
    let governance_request = governance_request();
    let governance_binding =
        reputation_binding(IrohaRuntimeProviderSlotV1::ReputationGovernanceDag);
    let governance_operation = make_operation_request(
        TEST_SESSION_ID,
        2,
        governance_binding.clone(),
        observation(&governance_binding).metadata_digest,
        OPERATION_REPUTATION_GOVERNANCE_RECONCILE_V1,
        encode_canonical(
            &reputation_governance_request_to_wire(&governance_request)
                .expect("project governance request"),
            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
        )
        .expect("encode governance request"),
    )
    .expect("seal governance operation");
    validate_operation_request(&governance_operation).expect("validate governance operation");
    assert_eq!(
        validate_operation_result(
            &governance_operation,
            STATUS_AMBIGUOUS_V1,
            &unit,
            &network_id(),
        ),
        Ok(())
    );
    let journal_binding =
        reputation_binding(IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter);
    let authority_keypair = KeyPair::try_from_seed(vec![0x89; 32], Algorithm::Ed25519)
        .expect("derive result-shape authority");
    let mut journal_operation = make_operation_request(
        TEST_SESSION_ID,
        3,
        journal_binding.clone(),
        observation(&journal_binding).metadata_digest,
        OPERATION_REPUTATION_JOURNAL_SUPPORTS_AUTHORITY_V1,
        encode_canonical(
            &ReputationJournalSupportsAuthorityRequestWireV1 {
                authority: AccountId::new(authority_keypair.public_key().clone()),
            },
            MAX_REPUTATION_RUNTIME_FRAME_BYTES_V1,
        )
        .expect("encode supports-authority request"),
    )
    .expect("seal supports-authority operation");
    validate_operation_request(&journal_operation).expect("validate supports-authority operation");
    assert_eq!(
        validate_operation_result(
            &journal_operation,
            STATUS_AMBIGUOUS_V1,
            &unit,
            &network_id(),
        ),
        Err(BrokerError::Protocol),
        "read-only authority probing cannot be ambiguous"
    );
    journal_operation.operation = OPERATION_REPUTATION_JOURNAL_SUBMIT_V1;
    assert_eq!(
        validate_operation_result(
            &journal_operation,
            STATUS_AMBIGUOUS_V1,
            &unit,
            &network_id(),
        ),
        Ok(()),
        "journal submission is mutation-ambiguous"
    );
}
#[test]
fn evidence_viewer_and_moderation_archive_wire_shapes_are_exact() {
    assert_eq!(
        validate_exact_backend_set(&[], &RuntimeProviderBrokerBackendsV1::new()),
        Ok(())
    );
    for slot in [
        IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn,
        IrohaRuntimeProviderSlotV1::EvidenceViewerGrantAuthority,
        IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner,
        IrohaRuntimeProviderSlotV1::EvidenceViewerErasure,
        IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore,
        IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher,
    ] {
        let binding = evidence_viewer_binding(slot);
        assert_eq!(validate_wire_binding(&binding), Ok(()), "{slot:?}");
        let observed = observation(&binding);
        assert_eq!(
            validate_observation(&binding, &observed),
            Ok(()),
            "{slot:?}"
        );
        assert_eq!(
            validate_exact_backend_set(
                std::slice::from_ref(&binding),
                &RuntimeProviderBrokerBackendsV1::new(),
            ),
            Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch),
            "{slot:?}"
        );
        let mut confused = binding.clone();
        confused.slot = if slot == IrohaRuntimeProviderSlotV1::EvidenceViewerErasure {
            IrohaRuntimeProviderSlotV1::EvidenceViewerWebAuthn.wire_id()
        } else {
            IrohaRuntimeProviderSlotV1::EvidenceViewerErasure.wire_id()
        };
        assert_eq!(
            validate_wire_binding(&confused),
            Err(BrokerError::BindingMismatch),
            "{slot:?}"
        );
    }
    let receipt = evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner);
    let mut substituted_receipt = observation(&receipt);
    substituted_receipt.evidence_viewer_receipt_signer_public_key = Some([0xEE; 32]);
    metadata_digest(&mut substituted_receipt);
    assert_eq!(
        validate_observation(&receipt, &substituted_receipt),
        Err(BrokerError::BindingMismatch)
    );
    let archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive);
    let mut substituted_archive = observation(&archive);
    substituted_archive.evidence_viewer_archive_id = Some([0xEF; 32]);
    metadata_digest(&mut substituted_archive);
    assert_eq!(
        validate_observation(&archive, &substituted_archive),
        Err(BrokerError::BindingMismatch)
    );
}
#[test]
fn moderation_archive_pre_dispatch_validation_is_slot_exact_and_body_bounded() {
    let moderation_archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive);
    let unit = encode_canonical(
        &ModerationPanelNotificationArchiveQualifyRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            network_id: network_id(),
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode archive qualification");
    let qualify = validated_operation(
        moderation_archive.clone(),
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_QUALIFY_V1,
        unit,
    );
    assert_eq!(
        validate_operation_request_for_session(&qualify, "server-test-chain", &network_id()),
        Ok(())
    );
    let install_payload = encode_canonical(
        &ModerationPanelNotificationArchiveInstallRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            network_id: network_id(),
            operation_id: [0xA1; 32],
            receipt_message: [0xA2; 32],
            canonical_artifact: vec![0xA3],
        },
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )
    .expect("encode moderation archive install");
    let install = validated_operation(
        moderation_archive.clone(),
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
        install_payload.clone(),
    );
    assert_eq!(
        validate_operation_request_for_session(&install, "server-test-chain", &network_id()),
        Ok(())
    );
    let read_payload = encode_canonical(
        &ModerationPanelNotificationArchiveReadRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            network_id: network_id(),
            operation_id: [0xA1; 32],
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode moderation archive read");
    let read = validated_operation(
        moderation_archive.clone(),
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_READ_V1,
        read_payload,
    );
    assert_eq!(
        validate_operation_request_for_session(&read, "server-test-chain", &network_id()),
        Ok(())
    );
    let evidence_archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive);
    let cross_slot = make_operation_request(
        TEST_SESSION_ID,
        91,
        evidence_archive.clone(),
        observation(&evidence_archive).metadata_digest,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
        install_payload.clone(),
    )
    .expect("seal cross-slot archive operation");
    assert_eq!(
        validate_operation_request_for_session(&cross_slot, "server-test-chain", &network_id()),
        Err(BrokerError::BindingMismatch)
    );
    for (request_id, operation, payload) in [
        (
            93,
            OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_QUALIFY_V1,
            encode_canonical(
                &ModerationPanelNotificationArchiveQualifyRequestWireV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                    slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                    network_id: network_id(),
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .expect("encode cross-slot moderation qualification"),
        ),
        (
            94,
            OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
            install_payload.clone(),
        ),
        (
            95,
            OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_READ_V1,
            encode_canonical(
                &ModerationPanelNotificationArchiveReadRequestWireV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                    slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                    network_id: network_id(),
                    operation_id: [0xA1; 32],
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .expect("encode cross-slot moderation read"),
        ),
    ] {
        let request = make_operation_request(
            TEST_SESSION_ID,
            request_id,
            evidence_archive.clone(),
            observation(&evidence_archive).metadata_digest,
            operation,
            payload,
        )
        .expect("seal moderation operation on evidence slot");
        assert_eq!(
            validate_operation_request_for_session(&request, "server-test-chain", &network_id()),
            Err(BrokerError::BindingMismatch)
        );
    }
    for (request_id, operation, payload) in [
        (
            96,
            OPERATION_EVIDENCE_VIEWER_ARCHIVE_INSTALL_V1,
            encode_canonical(
                &EvidenceViewerArchiveInstallRequestWireV1 {
                    operation_id: [0xD1; 32],
                    receipt_message: [0xD2; 32],
                    canonical_artifact: vec![0xD3],
                },
                MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
            )
            .expect("encode evidence archive install"),
        ),
        (
            97,
            OPERATION_EVIDENCE_VIEWER_ARCHIVE_READ_V1,
            encode_canonical(
                &EvidenceViewerArchiveReadRequestWireV1 {
                    operation_id: [0xD1; 32],
                },
                MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
            )
            .expect("encode evidence archive read"),
        ),
    ] {
        let request = make_operation_request(
            TEST_SESSION_ID,
            request_id,
            moderation_archive.clone(),
            observation(&moderation_archive).metadata_digest,
            operation,
            payload,
        )
        .expect("seal evidence operation on moderation slot");
        assert_eq!(
            validate_operation_request_for_session(&request, "server-test-chain", &network_id()),
            Err(BrokerError::BindingMismatch)
        );
    }
    let mut one_byte_archive = moderation_archive;
    one_byte_archive
        .moderation_panel_notification_archive_binding
        .as_mut()
        .expect("moderation archive binding")
        .max_bytes = 1;
    assert_eq!(validate_wire_binding(&one_byte_archive), Ok(()));
    let oversized_payload = encode_canonical(
        &ModerationPanelNotificationArchiveInstallRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            network_id: network_id(),
            operation_id: [0xB1; 32],
            receipt_message: [0xB2; 32],
            canonical_artifact: vec![0xB3, 0xB4],
        },
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )
    .expect("encode over-bound moderation archive install");
    let oversized = make_operation_request(
        TEST_SESSION_ID,
        92,
        one_byte_archive.clone(),
        observation(&one_byte_archive).metadata_digest,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
        oversized_payload,
    )
    .expect("seal over-bound archive operation");
    assert_eq!(
        validate_operation_request_for_session(&oversized, "server-test-chain", &network_id()),
        Err(BrokerError::Rejected)
    );
}
include!("moderation_source_attestation_tests.rs");
#[test]
fn moderation_archive_fixture_is_preflighted_before_every_mutating_backend() {
    let fixture = test_moderation::moderation_panel_notification_archive_broker_fixture_v1()
        .expect("build genuine signed moderation archive fixture");
    let catalog =
        IrohaRuntimeProviderBindingsV1::qualified_moderation_panel_notification_archive_for_test(
            &fixture,
            SERVER_TEST_MODERATION_PUBLICATION_HANDLE,
            7,
            TEST_POLICY_DIGEST,
        );
    let checkpoint = Arc::new(ServerTestModerationCheckpointStore::from_fixture(&fixture));
    let archive = Arc::new(ServerTestModerationPanelNotificationArchive::from_fixture(
        &fixture,
    ));
    let publication = Arc::new(ServerTestModerationHandoffBoundary::exact(
        test_moderation::ModerationTerminalHandoffKindV1::Publication,
    ));
    let state = prepare_server_state(
        &catalog,
        RuntimeProviderBrokerBackendsV1::new()
            .with_moderation_publication_handoff(publication.clone())
            .with_moderation_checkpoint_store(checkpoint.clone())
            .with_moderation_panel_notification_archive(archive.clone()),
    )
    .expect("prepare exact moderation archive broker state");
    let operation_for_slot =
        |request_id: u64, slot: IrohaRuntimeProviderSlotV1, operation, payload| {
            let index = state
                .catalog
                .iter()
                .position(|binding| binding.slot == slot.wire_id())
                .expect("fixture provider binding");
            make_operation_request(
                TEST_SESSION_ID,
                request_id,
                state.catalog[index].clone(),
                state.observations[index].metadata_digest,
                operation,
                payload,
            )
            .expect("seal fixture operation request")
        };
    let mut substituted_receipt = fixture.validation.receipt_message;
    substituted_receipt[0] ^= 1;
    let substituted_install = operation_for_slot(
        1,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
        encode_canonical(
            &ModerationPanelNotificationArchiveInstallRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                network_id: fixture.network_id,
                operation_id: fixture.validation.operation_id,
                receipt_message: substituted_receipt,
                canonical_artifact: fixture.canonical_artifact.clone(),
            },
            MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
        )
        .expect("encode substituted archive install"),
    );
    validate_operation_request_for_session(
        &substituted_install,
        "server-test-chain",
        &fixture.network_id,
    )
    .expect("substituted receipt is structurally canonical");
    assert_eq!(
        dispatch_server_operation(&state, &substituted_install),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        archive.install_calls.load(Ordering::Acquire),
        0,
        "derived receipt substitution must be rejected before archive installation"
    );
    let install = operation_for_slot(
        2,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_INSTALL_V1,
        encode_canonical(
            &ModerationPanelNotificationArchiveInstallRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                network_id: fixture.network_id,
                operation_id: fixture.validation.operation_id,
                receipt_message: fixture.validation.receipt_message,
                canonical_artifact: fixture.canonical_artifact.clone(),
            },
            MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
        )
        .expect("encode exact archive install"),
    );
    validate_operation_request_for_session(&install, "server-test-chain", &fixture.network_id)
        .expect("validate exact archive install");
    let install_result =
        dispatch_server_operation(&state, &install).expect("install genuine archive fixture");
    validate_operation_result(&install, STATUS_OK_V1, &install_result, &state.network_id)
        .expect("validate exact archive install result");
    let install_result = decode_canonical::<ModerationPanelNotificationArchiveInstallResultWireV1>(
        &install_result,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("decode archive install result");
    assert_eq!(install_result.signature, fixture.archive_signature);
    assert_eq!(archive.install_calls.load(Ordering::Acquire), 1);
    let read = operation_for_slot(
        3,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_READ_V1,
        encode_canonical(
            &ModerationPanelNotificationArchiveReadRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
                network_id: fixture.network_id,
                operation_id: fixture.validation.operation_id,
            },
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
        )
        .expect("encode exact archive read"),
    );
    let readback = dispatch_server_operation(&state, &read).expect("read genuine archive fixture");
    let readback = decode_canonical::<Option<ModerationPanelNotificationArchiveReadbackWireV1>>(
        &readback,
        MAX_EVIDENCE_VIEWER_BULK_FRAME_BYTES_V1,
    )
    .expect("decode exact archive readback")
    .expect("installed fixture must be readable");
    assert_eq!(
        readback.canonical_artifact.as_slice(),
        fixture.canonical_artifact.as_slice()
    );
    assert_eq!(readback.signature, fixture.archive_signature);
    let mut substituted_statement = fixture.source_attestation.clone();
    substituted_statement.terminal_set_digest[0] ^= 1;
    let substituted_attestation = operation_for_slot(
        4,
        IrohaRuntimeProviderSlotV1::ModerationCheckpointStore,
        OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
        encode_canonical(
            &ModerationPanelNotificationSourceAttestRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
                network_id: fixture.network_id,
                statement: substituted_statement,
            },
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
        )
        .expect("encode substituted source attestation"),
    );
    validate_operation_request_for_session(
        &substituted_attestation,
        "server-test-chain",
        &fixture.network_id,
    )
    .expect("substituted source digest is structurally canonical");
    assert_eq!(
        dispatch_server_operation(&state, &substituted_attestation),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        checkpoint.attest_calls.load(Ordering::Acquire),
        0,
        "source substitution must be rejected before checkpoint attestation signing"
    );
    let attest = operation_for_slot(
        5,
        IrohaRuntimeProviderSlotV1::ModerationCheckpointStore,
        OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
        encode_canonical(
            &ModerationPanelNotificationSourceAttestRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
                network_id: fixture.network_id,
                statement: fixture.source_attestation.clone(),
            },
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
        )
        .expect("encode exact source attestation"),
    );
    let attest_result =
        dispatch_server_operation(&state, &attest).expect("attest genuine terminal set");
    let attest_result = decode_canonical::<ModerationPanelNotificationSourceAttestResultWireV1>(
        &attest_result,
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("decode exact source attestation result");
    assert_eq!(
        attest_result.statement_digest,
        fixture.validation.source_attestation_digest
    );
    fixture
        .source_attestation
        .verify(attest_result.signature)
        .expect("verify independently signed source attestation");
    assert_eq!(checkpoint.attest_calls.load(Ordering::Acquire), 1);
    let head = decode_canonical::<test_moderation::ModerationPanelNotificationArchiveHeadV1>(
        &fixture.canonical_signed_head,
        MAX_MODERATION_HANDOFF_CANONICAL_BYTES_V1,
    )
    .expect("decode genuine signed archive head");
    let archive_head_read_payload = encode_canonical(&(), MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
        .expect("encode archive-head read request");
    let cross_slot_read = operation_for_slot(
        60,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1,
        archive_head_read_payload.clone(),
    );
    assert_eq!(
        validate_operation_request_for_session(
            &cross_slot_read,
            "server-test-chain",
            &fixture.network_id,
        ),
        Err(BrokerError::BindingMismatch)
    );
    assert_eq!(
        dispatch_server_operation(&state, &cross_slot_read),
        Err(BrokerError::BindingMismatch)
    );
    let mut trailing_read_payload = archive_head_read_payload.clone();
    trailing_read_payload.push(0);
    let trailing_read = operation_for_slot(
        61,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1,
        trailing_read_payload,
    );
    assert_eq!(
        validate_operation_request_for_session(
            &trailing_read,
            "server-test-chain",
            &fixture.network_id,
        ),
        Err(BrokerError::Protocol)
    );
    assert_eq!(
        dispatch_server_operation(&state, &trailing_read),
        Err(BrokerError::Protocol)
    );
    let empty_head_read = operation_for_slot(
        62,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1,
        archive_head_read_payload.clone(),
    );
    validate_operation_request_for_session(
        &empty_head_read,
        "server-test-chain",
        &fixture.network_id,
    )
    .expect("validate empty archive-head readback request");
    let empty_head_read_result = dispatch_server_operation(&state, &empty_head_read)
        .expect("read empty public archive head");
    validate_operation_result(
        &empty_head_read,
        STATUS_OK_V1,
        &empty_head_read_result,
        &state.network_id,
    )
    .expect("validate empty archive-head readback result");
    let empty_head_read_result =
        decode_canonical::<ModerationPanelNotificationArchiveHeadReadResultWireV1>(
            &empty_head_read_result,
            MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
        )
        .expect("decode empty archive-head readback");
    assert_eq!(
        empty_head_read_result.version,
        MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1
    );
    assert_eq!(
        empty_head_read_result.slot,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id()
    );
    assert!(empty_head_read_result.canonical_head.is_none());
    let publish_wire = ModerationPanelNotificationArchiveHeadPublishRequestWireV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
        slot: IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id(),
        network_id: fixture.network_id,
        head: head.clone(),
        canonical_head: fixture.canonical_signed_head.clone(),
    };
    let publish_payload = encode_canonical(&publish_wire, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
        .expect("encode exact archive-head publication");
    let cross_slot_publish = operation_for_slot(
        6,
        IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_PUBLISH_V1,
        publish_payload.clone(),
    );
    assert_eq!(
        validate_operation_request_for_session(
            &cross_slot_publish,
            "server-test-chain",
            &fixture.network_id,
        ),
        Err(BrokerError::BindingMismatch)
    );
    assert_eq!(
        dispatch_server_operation(&state, &cross_slot_publish),
        Err(BrokerError::BindingMismatch)
    );
    let mut substituted_head = head;
    substituted_head.source_checkpoint_revision[0] ^= 1;
    let substituted_head_bytes =
        norito::to_bytes(&substituted_head).expect("encode substituted archive head");
    let substituted_publish = operation_for_slot(
        7,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_PUBLISH_V1,
        encode_canonical(
            &ModerationPanelNotificationArchiveHeadPublishRequestWireV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
                slot: IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff.wire_id(),
                network_id: fixture.network_id,
                head: substituted_head,
                canonical_head: substituted_head_bytes,
            },
            MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
        )
        .expect("encode substituted archive-head publication"),
    );
    assert_eq!(
        validate_operation_request_for_session(
            &substituted_publish,
            "server-test-chain",
            &fixture.network_id,
        ),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        dispatch_server_operation(&state, &substituted_publish),
        Err(BrokerError::Rejected)
    );
    let mut substituted_source_catalog = state.catalog.clone();
    substituted_source_catalog
        .iter_mut()
        .find(|binding| {
            binding.slot == IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id()
        })
        .expect("checkpoint fixture binding")
        .moderation_checkpoint_attestation_public_key = Some(TEST_SIGNER_KEY);
    assert_eq!(
        validate_moderation_panel_notification_archive_head_at_broker_boundary(
            &fixture.canonical_signed_head,
            &fixture.network_id,
            &substituted_source_catalog,
        ),
        Err(BrokerError::Rejected),
        "the independently administered checkpoint key is part of the head boundary"
    );
    let mut substituted_signer_catalog = state.catalog.clone();
    substituted_signer_catalog
        .iter_mut()
        .find(|binding| {
            binding.slot == IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id()
        })
        .expect("archive fixture binding")
        .moderation_panel_notification_archive_binding
        .as_mut()
        .expect("exact archive fixture binding")
        .public_key = TEST_SIGNER_KEY;
    assert_eq!(
        validate_moderation_panel_notification_archive_head_at_broker_boundary(
            &fixture.canonical_signed_head,
            &fixture.network_id,
            &substituted_signer_catalog,
        ),
        Err(BrokerError::Rejected),
        "the sealed current signer epoch is part of the head boundary"
    );
    assert_eq!(
        publication.delivery_calls.load(Ordering::Acquire),
        0,
        "cross-slot and signed-head substitution must precede publication"
    );
    let publish = operation_for_slot(
        8,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_PUBLISH_V1,
        publish_payload,
    );
    validate_operation_request_for_session(&publish, "server-test-chain", &fixture.network_id)
        .expect("validate genuine archive-head publication");
    let publish_result =
        dispatch_server_operation(&state, &publish).expect("publish genuine signed archive head");
    validate_operation_result(&publish, STATUS_OK_V1, &publish_result, &state.network_id)
        .expect("validate dedicated archive-head result");
    let publish_result = decode_canonical::<
        ModerationPanelNotificationArchiveHeadPublishResultWireV1,
    >(&publish_result, MAX_MODERATION_HANDOFF_FRAME_BYTES_V1)
    .expect("decode archive-head publication result");
    assert_eq!(publish_result.operation_id, fixture.validation.operation_id);
    assert_eq!(publish_result.head_digest, fixture.validation.head_digest);
    assert_eq!(
        publish_result.chain_commitment,
        fixture.validation.chain_commitment
    );
    assert_eq!(publish_result.outcome, 1);
    assert_eq!(publication.delivery_calls.load(Ordering::Acquire), 1);
    let published_head_read = operation_for_slot(
        63,
        IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_HEAD_READ_V1,
        archive_head_read_payload,
    );
    validate_operation_request_for_session(
        &published_head_read,
        "server-test-chain",
        &fixture.network_id,
    )
    .expect("validate published archive-head readback request");
    let published_head_read_result = dispatch_server_operation(&state, &published_head_read)
        .expect("read exact public archive head");
    validate_operation_result(
        &published_head_read,
        STATUS_OK_V1,
        &published_head_read_result,
        &state.network_id,
    )
    .expect("validate published archive-head readback result");
    let published_head_read_result =
        decode_canonical::<ModerationPanelNotificationArchiveHeadReadResultWireV1>(
            &published_head_read_result,
            MAX_MODERATION_HANDOFF_FRAME_BYTES_V1,
        )
        .expect("decode published archive-head readback");
    assert_eq!(
        published_head_read_result.canonical_head.as_deref(),
        Some(fixture.canonical_signed_head.as_slice())
    );
    assert_eq!(
        publication.delivery_calls.load(Ordering::Acquire),
        1,
        "public head reads must never replay publication"
    );
}
#[test]
fn evidence_transparency_ambiguity_reconnects_for_readback_without_replay() {
    let catalog = evidence_transparency_publisher_test_catalog();
    let backend = Arc::new(ServerTestEvidenceTransparencyPublisher::default());
    let (_directory, policy, shutdown, server) = start_signer(
        catalog.clone(),
        RuntimeProviderBrokerBackendsV1::new()
            .with_evidence_viewer_transparency_publisher(backend.clone()),
    );
    let dependencies = resolve(&catalog, &policy).expect("resolve transparency-publisher proxy");
    let publisher = dependencies
        .sorafs_evidence_viewer_transparency_publisher
        .as_ref()
        .expect("resolved transparency publisher");
    assert_eq!(
        publisher.compare_and_publish(&evidence_transparency_test_body()),
        Err(
            test_evidence_transparency::
                EvidenceViewerTransparencyPublisherExternalErrorV1::Ambiguous,
        )
    );
    assert_eq!(
        backend.compare_calls.load(Ordering::Acquire),
        1,
        "an ambiguous mutation must never be replayed"
    );
    assert_eq!(
        publisher
            .qualification()
            .expect("requalify over the fresh broker session"),
        node::evidence_viewer::EvidenceViewerRuntimeProviderQualificationV1::new(
            7,
            TEST_POLICY_DIGEST
        ),
    );
    assert_eq!(
        publisher.load_head(),
        Ok(None),
        "fresh authoritative readback must remain available"
    );
    assert_eq!(
        backend.compare_calls.load(Ordering::Acquire),
        1,
        "qualification and readback must not replay the mutation"
    );
    drop(dependencies);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join transparency-publisher broker")
        .expect("transparency-publisher broker exits cleanly");
}
struct TestGlobalBeaconBrokerBackendV1 {
    handle: &'static str,
    qualification: ConsensusSignerProviderQualificationV1,
}

impl GlobalBeaconPartialSignerBrokerBackendV1 for TestGlobalBeaconBrokerBackendV1 {
    fn handle(&self) -> &str {
        self.handle
    }

    fn qualification(
        &self,
    ) -> Result<ConsensusSignerProviderQualificationV1, GlobalBeaconPartialSignerBrokerBackendErrorV1>
    {
        Ok(self.qualification)
    }

    fn sign_partial(
        &self,
        _session: &iroha_core::beacon::ValidatedGlobalThresholdBeaconSessionV1,
        _payload: &[u8],
    ) -> Result<
        iroha_data_model::consensus::GlobalThresholdBeaconPartialSignatureV1,
        GlobalBeaconPartialSignerBrokerBackendErrorV1,
    > {
        Err(GlobalBeaconPartialSignerBrokerBackendErrorV1)
    }
}

struct TestParliamentTleBrokerBackendV1 {
    handle: &'static str,
    qualification: ConsensusSignerProviderQualificationV1,
}

impl ParliamentTlePartialReleaseSignerBrokerBackendV1 for TestParliamentTleBrokerBackendV1 {
    fn handle(&self) -> &str {
        self.handle
    }

    fn qualification(
        &self,
    ) -> Result<
        ConsensusSignerProviderQualificationV1,
        ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    > {
        Ok(self.qualification)
    }

    fn sign_projected_partial_release(
        &self,
        _projection: &iroha_core::tle_release::ValidatedTleReleaseProjectionV1,
    ) -> Result<
        iroha_core::tle_release::TlePartialReleaseShareV1,
        ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    > {
        Err(ParliamentTlePartialReleaseSignerBrokerBackendErrorV1)
    }
}

#[test]
fn consensus_signer_broker_startup_is_exact_and_fail_closed() {
    let handle = "hsm://iroha/consensus-signers/primary";
    let revision = 7;
    let digest = [0xA7; 32];
    let qualification = ConsensusSignerProviderQualificationV1::new(revision, digest, false);

    let beacon_catalog = IrohaRuntimeProviderBindingsV1::qualified_for_test(
        "broker-consensus-test",
        IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
        handle,
        revision,
        digest,
    );
    assert!(matches!(
        prepare_server_state(&beacon_catalog, RuntimeProviderBrokerBackendsV1::new()),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    ));
    prepare_server_state(
        &beacon_catalog,
        RuntimeProviderBrokerBackendsV1::new().with_global_beacon_partial_signer(Arc::new(
            TestGlobalBeaconBrokerBackendV1 {
                handle,
                qualification,
            },
        )),
    )
    .expect("exact beacon signer backend qualifies");
    for backend in [
        TestGlobalBeaconBrokerBackendV1 {
            handle: "hsm://iroha/consensus-signers/substituted",
            qualification,
        },
        TestGlobalBeaconBrokerBackendV1 {
            handle,
            qualification: ConsensusSignerProviderQualificationV1::new(revision + 1, digest, false),
        },
        TestGlobalBeaconBrokerBackendV1 {
            handle,
            qualification: ConsensusSignerProviderQualificationV1::new(revision, [0xA8; 32], false),
        },
        TestGlobalBeaconBrokerBackendV1 {
            handle,
            qualification: ConsensusSignerProviderQualificationV1::new(revision, digest, true),
        },
    ] {
        assert!(matches!(
            prepare_server_state(
                &beacon_catalog,
                RuntimeProviderBrokerBackendsV1::new()
                    .with_global_beacon_partial_signer(Arc::new(backend)),
            ),
            Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
        ));
    }

    let tle_catalog = IrohaRuntimeProviderBindingsV1::qualified_for_test(
        "broker-consensus-test",
        IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
        handle,
        revision,
        digest,
    );
    assert!(matches!(
        prepare_server_state(
            &tle_catalog,
            RuntimeProviderBrokerBackendsV1::new().with_global_beacon_partial_signer(Arc::new(
                TestGlobalBeaconBrokerBackendV1 {
                    handle,
                    qualification,
                },
            )),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    ));
    prepare_server_state(
        &tle_catalog,
        RuntimeProviderBrokerBackendsV1::new().with_parliament_tle_partial_release_signer(
            Arc::new(TestParliamentTleBrokerBackendV1 {
                handle,
                qualification,
            }),
        ),
    )
    .expect("exact Parliament TLE signer backend qualifies");
    for qualification in [
        ConsensusSignerProviderQualificationV1::new(revision + 1, digest, false),
        ConsensusSignerProviderQualificationV1::new(revision, [0xA8; 32], false),
        ConsensusSignerProviderQualificationV1::new(revision, digest, true),
    ] {
        assert!(matches!(
            prepare_server_state(
                &tle_catalog,
                RuntimeProviderBrokerBackendsV1::new().with_parliament_tle_partial_release_signer(
                    Arc::new(TestParliamentTleBrokerBackendV1 {
                        handle,
                        qualification,
                    },)
                ),
            ),
            Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
        ));
    }
}

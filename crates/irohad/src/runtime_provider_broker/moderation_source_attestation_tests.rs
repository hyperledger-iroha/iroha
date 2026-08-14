// Moderation source-attestation request validation regressions.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct RetiredModerationArchiveQualifyRequestWireV1 {
    version: u16,
    slot: u16,
    chain_id: String,
}
#[test]
fn moderation_source_attestation_pre_dispatch_is_exact_network_and_slot_bound() {
    let checkpoint = evidence_viewer_binding(IrohaRuntimeProviderSlotV1::ModerationCheckpointStore);
    let statement =
        sorafs_node::moderation_orchestrator::ModerationPanelNotificationSourceAttestationV1 {
            version: sorafs_node::moderation_orchestrator::
                MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
            attestor_slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
            network_id: server_test_network_id(),
            checkpoint_namespace_digest: [0x31; 32],
            checkpoint_generation: 7,
            checkpoint_revision: [0x32; 32],
            checkpoint_digest: [0x33; 32],
            source_manifest_digest: [0x36; 32],
            terminal_set_digest: [0x34; 32],
            terminal_record_count: 1,
            first_notification_id: [0x35; 32],
            last_notification_id: [0x35; 32],
            attestor_handle: checkpoint.handle.clone(),
            attestor_revision: checkpoint.revision.expect("checkpoint revision"),
            attestor_policy_digest: checkpoint.policy_digest.expect("checkpoint policy"),
            attestor_public_key: checkpoint
                .moderation_checkpoint_attestation_public_key
                .expect("checkpoint attestation key"),
        };
    let payload = encode_canonical(
        &ModerationPanelNotificationSourceAttestRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
            network_id: server_test_network_id(),
            statement: statement.clone(),
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode typed source attestation");
    let request = validated_test_operation(
        checkpoint.clone(),
        OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
        payload.clone(),
    );
    assert_eq!(
        validate_operation_request_for_session(
            &request,
            "server-test-chain",
            &server_test_network_id()
        ),
        Ok(())
    );
    let archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive);
    let wrong_slot = make_operation_request(
        TEST_SESSION_ID,
        98,
        archive.clone(),
        observation(&archive).metadata_digest,
        OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
        payload,
    )
    .expect("seal source attestation on archive slot");
    assert_eq!(
        validate_operation_request_for_session(
            &wrong_slot,
            "server-test-chain",
            &server_test_network_id()
        ),
        Err(BrokerError::BindingMismatch)
    );
    let mut substituted = statement;
    substituted.terminal_set_digest = [0; 32];
    let substituted_payload = encode_canonical(
        &ModerationPanelNotificationSourceAttestRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationCheckpointStore.wire_id(),
            network_id: server_test_network_id(),
            statement: substituted,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode substituted source attestation");
    let substituted = make_operation_request(
        TEST_SESSION_ID,
        99,
        checkpoint.clone(),
        observation(&checkpoint).metadata_digest,
        OPERATION_MODERATION_PANEL_NOTIFICATION_SOURCE_ATTEST_V1,
        substituted_payload,
    )
    .expect("seal substituted source attestation");
    assert_eq!(
        validate_operation_request_for_session(
            &substituted,
            "server-test-chain",
            &server_test_network_id()
        ),
        Err(BrokerError::Rejected)
    );
}
#[test]
fn moderation_archive_rejects_same_label_foreign_genesis_and_retired_chain_wire() {
    let archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive);
    let foreign_network = test_network_id(0x16);
    let payload = encode_canonical(
        &ModerationPanelNotificationArchiveQualifyRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            network_id: foreign_network,
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode foreign-network qualification");
    let request = validated_test_operation(
        archive,
        OPERATION_MODERATION_PANEL_NOTIFICATION_ARCHIVE_QUALIFY_V1,
        payload,
    );
    assert_eq!(
        validate_operation_request_for_session(
            &request,
            "server-test-chain",
            &server_test_network_id(),
        ),
        Err(BrokerError::BindingMismatch),
        "an identical display label must not admit a different genesis"
    );
    let legacy = encode_canonical(
        &RetiredModerationArchiveQualifyRequestWireV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_WIRE_VERSION_V1,
            slot: IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id(),
            chain_id: "server-test-chain".to_owned(),
        },
        MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
    )
    .expect("encode retired label-based qualification");
    assert!(
        decode_canonical::<ModerationPanelNotificationArchiveQualifyRequestWireV1>(
            &legacy,
            MAX_EVIDENCE_VIEWER_CONTROL_BYTES_V1,
        )
        .is_err(),
        "the retired chain_id wire shape must have no compatibility decoder"
    );
}
#[test]
fn moderation_delivery_rejects_same_label_foreign_genesis_before_dispatch() {
    use sorafs_node::moderation_orchestrator::ModerationTerminalHandoffKindV1 as Kind;
    let foreign_network = test_network_id(0x16);
    let mut handoff = moderation_handoff_test_request(Kind::Settlement);
    handoff.handoff.network_id = foreign_network;
    handoff.handoff.handoff_id = handoff.handoff.canonical_id();
    handoff.canonical_handoff = norito::to_bytes(&handoff.handoff).expect("encode foreign handoff");
    let handoff_wire = moderation_handoff_request_to_wire(
        &handoff,
        IrohaRuntimeProviderSlotV1::ModerationSettlementHandoff.wire_id(),
    )
    .expect("project intrinsically valid foreign handoff");
    assert!(
        matches!(
            validate_moderation_handoff_request(
                &handoff_wire,
                IrohaRuntimeProviderSlotV1::ModerationSettlementHandoff.wire_id(),
                Some(&server_test_network_id()),
            ),
            Err(BrokerError::Rejected)
        ),
        "an identical display label must not admit a foreign-network handoff"
    );
    let mut notification = moderation_panel_test_request();
    notification.notification.network_id = foreign_network;
    notification.notification.notification_id = notification.notification.canonical_id();
    notification.canonical_notification =
        norito::to_bytes(&notification.notification).expect("encode foreign notification");
    let notification_wire = moderation_panel_notification_request_to_wire(&notification)
        .expect("project intrinsically valid foreign notification");
    assert!(
        matches!(
            validate_moderation_panel_notification_request(
                &notification_wire,
                Some(&server_test_network_id()),
            ),
            Err(BrokerError::Rejected)
        ),
        "an identical display label must not admit a foreign-network notification"
    );
}

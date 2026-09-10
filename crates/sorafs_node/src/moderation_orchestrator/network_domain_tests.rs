// Exact-network wire and durable-domain regressions.
#[test]
fn source_attestation_and_archive_head_roundtrip_with_exact_network_id() {
    let fixture = moderation_panel_notification_archive_broker_fixture_v1()
        .expect("build canonical moderation archive fixture");
    // Roundtrip the source statement fields as payload; the signed archive head owns the frame.
    let statement_bytes = norito::codec::encode_adaptive(&fixture.source_attestation);
    let decoded_statement: ModerationPanelNotificationSourceAttestationV1 =
        norito::codec::decode_adaptive(&statement_bytes)
            .expect("decode source attestation payload");
    assert_eq!(decoded_statement, fixture.source_attestation);
    assert_eq!(decoded_statement.network_id, fixture.network_id);
    assert_eq!(
        norito::codec::encode_adaptive(&decoded_statement),
        statement_bytes
    );
    let decoded_head: ModerationPanelNotificationArchiveHeadV1 =
        norito::decode_from_bytes(&fixture.canonical_signed_head)
            .expect("decode canonical signed archive head");
    assert_eq!(decoded_head.network_id, fixture.network_id);
    assert_eq!(
        norito::to_bytes(&decoded_head).expect("re-encode canonical signed archive head"),
        fixture.canonical_signed_head
    );
}
#[test]
fn handoff_and_notification_roundtrip_retain_exact_network_identity() {
    let network_id = test_network_id();
    let foreign_network = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"moderation-same-label-foreign-genesis",
        )),
    );
    let cursor = ModerationFinalizedEventCursorV1 {
        sequence: 1,
        block_height: 7,
        block_hash: [0x71; 32],
        event_index: 0,
    };
    let recipient = account(7);
    let mut notification = ModerationPanelNotificationV1 {
        notification_id: [0; 32],
        network_id,
        source_operation_id: [0x72; 32],
        scope_digest: [0x73; 32],
        kind: ModerationPanelNotificationKindV1::PrimaryAssignment,
        recipient,
        finalized_event_cursor: cursor,
        source_occurred_at_unix_ms: 700,
    };
    notification.notification_id = notification.canonical_id();
    crate::frame_test_support::assert_current_frame(
        &notification,
        "sorafs_node::moderation_orchestrator::ModerationPanelNotificationV1",
    );
    let bytes = norito::codec::encode_adaptive(&notification);
    let decoded: ModerationPanelNotificationV1 =
        norito::codec::decode_adaptive(&bytes).expect("decode exact-network notification payload");
    assert_eq!(decoded, notification);
    assert!(decoded.is_bound_to_network(&network_id));
    assert!(!decoded.is_bound_to_network(&foreign_network));
    let mut handoff = ModerationTerminalHandoffV1 {
        handoff_id: [0; 32],
        network_id,
        kind: ModerationTerminalHandoffKindV1::Settlement,
        case_id: "case-1".to_owned(),
        round_id: "round-1".to_owned(),
        outcome_digest: [0x74; 32],
        outcome_finalized_at_unix_ms: 700,
        finalized_cursor: cursor,
        source_event_witness: ModerationFinalizedEventV1 {
            sequence: cursor.sequence,
            block_height: cursor.block_height,
            block_hash: cursor.block_hash,
            event_index: cursor.event_index,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::CaseFinalized,
                Some("case-1".to_owned()),
                Some("round-1".to_owned()),
                account(8),
                700,
            ),
        },
    };
    handoff.handoff_id = handoff.canonical_id();
    crate::frame_test_support::assert_current_frame(
        &handoff,
        "sorafs_node::moderation_orchestrator::ModerationTerminalHandoffV1",
    );
    let bytes = norito::codec::encode_adaptive(&handoff);
    let decoded: ModerationTerminalHandoffV1 =
        norito::codec::decode_adaptive(&bytes).expect("decode exact-network handoff payload");
    assert_eq!(decoded, handoff);
    assert!(decoded.is_bound_to_network(&network_id));
    assert!(!decoded.is_bound_to_network(&foreign_network));
}

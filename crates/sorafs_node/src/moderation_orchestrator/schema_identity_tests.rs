// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<StoredPanelNotificationV1>(
        "sorafs_node::moderation_orchestrator::StoredPanelNotificationV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationTerminalHandoffV1>(
        "sorafs_node::moderation_orchestrator::ModerationTerminalHandoffV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationPanelNotificationV1>(
        "sorafs_node::moderation_orchestrator::ModerationPanelNotificationV1",
    );
    crate::schema_identity_test_support::assert_identity::<
        ModerationPanelNotificationSourceAttestationV1,
    >("sorafs_node::moderation_orchestrator::ModerationPanelNotificationSourceAttestationV1");
    crate::schema_identity_test_support::assert_identity::<ModerationNativeActionV1>(
        "sorafs_node::moderation_orchestrator::ModerationNativeActionV1",
    );
    crate::schema_identity_test_support::assert_identity::<
        ModerationPanelNotificationArchiveArtifactV1,
    >("sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveArtifactV1");
    crate::schema_identity_test_support::assert_identity::<ModerationPanelNotificationArchiveHeadV1>(
        "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveHeadV1",
    );
    crate::schema_identity_test_support::assert_identity::<
        ModerationPanelNotificationArchivePayloadV1,
    >("sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchivePayloadV1");
    crate::schema_identity_test_support::assert_identity::<
        ModerationPanelNotificationArchiveRecordV1,
    >("sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveRecordV1");
    crate::schema_identity_test_support::assert_identity::<
        ModerationPanelNotificationArchiveSourceManifestV1,
    >(
        "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveSourceManifestV1"
    );
    crate::schema_identity_test_support::assert_identity::<StoredDeadLetterRedriveV1>(
        "sorafs_node::moderation_orchestrator::StoredDeadLetterRedriveV1",
    );
    crate::schema_identity_test_support::assert_identity::<StoredOperationV1>(
        "sorafs_node::moderation_orchestrator::StoredOperationV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationOrchestratorCheckpointV1>(
        "sorafs_node::moderation_orchestrator::ModerationOrchestratorCheckpointV1",
    );
}

#[test]
fn moderation_real_archive_head_advertises_explicit_schema_identity() {
    let fixture = moderation_panel_notification_archive_broker_fixture_v1()
        .expect("real signed archive fixture");
    let (head, validation) = validate_moderation_panel_notification_archive_head_for_broker_v1(
        &fixture.canonical_signed_head,
        &fixture.expectation(),
    )
    .expect("signature and authority check");
    assert_eq!(validation, fixture.validation);
    assert_eq!(
        crate::schema_identity_test_support::assert_canonical_frame(
            &head,
            "sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveHeadV1"
        ),
        fixture.canonical_signed_head
    );
    let action = policy_action(policy(1));
    crate::schema_identity_test_support::assert_canonical_frame(
        &action,
        "sorafs_node::moderation_orchestrator::ModerationNativeActionV1",
    );
}

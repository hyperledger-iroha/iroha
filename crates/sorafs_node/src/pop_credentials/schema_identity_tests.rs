// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<PopApprovalV1>(
        "sorafs_node::pop_credentials::PopApprovalV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopEncryptedEnrollmentV1>(
        "sorafs_node::pop_credentials::PopEncryptedEnrollmentV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopEncryptedWalletDeliveryV1>(
        "sorafs_node::pop_credentials::PopEncryptedWalletDeliveryV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopEnrollmentAadV1>(
        "sorafs_node::pop_credentials::PopEnrollmentAadV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopIssuerCheckpointV1>(
        "sorafs_node::pop_credentials::PopIssuerCheckpointV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopPrivateEnrollmentV1>(
        "sorafs_node::pop_credentials::PopPrivateEnrollmentV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopPrivateWalletDeliveryV1>(
        "sorafs_node::pop_credentials::PopPrivateWalletDeliveryV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopPrivateWitnessEnvelopeV1>(
        "sorafs_node::pop_credentials::PopPrivateWitnessEnvelopeV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopRegistryOperationV1>(
        "sorafs_node::pop_credentials::PopRegistryOperationV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopWalletDeliveryAadV1>(
        "sorafs_node::pop_credentials::PopWalletDeliveryAadV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopWalletVaultEnvelopeV1>(
        "sorafs_node::pop_credentials::PopWalletVaultEnvelopeV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopWalletVaultMetadataV1>(
        "sorafs_node::pop_credentials::PopWalletVaultMetadataV1",
    );
    crate::schema_identity_test_support::assert_identity::<PopWalletVaultPlaintextV1>(
        "sorafs_node::pop_credentials::PopWalletVaultPlaintextV1",
    );
}

#[test]
fn pop_real_enrollment_and_persisted_checkpoint_advertise_explicit_schema_identity() {
    let (_temp, mut service, policy, _wallet, approvers, enrollment) = service_fixture();
    let frame = crate::schema_identity_test_support::assert_canonical_frame(
        &enrollment,
        "sorafs_node::pop_credentials::PopEncryptedEnrollmentV1",
    );
    service.submit_enrollment(&frame, 20).unwrap();
    let signed = approval(
        "approver-0",
        &approvers[0],
        &enrollment,
        &policy,
        PopApprovalDecisionV1::Approve,
    );
    crate::schema_identity_test_support::assert_canonical_frame(
        &signed,
        "sorafs_node::pop_credentials::PopApprovalV1",
    );
    service.record_approval(signed, 20).unwrap();
    let checkpoint = crate::schema_identity_test_support::assert_canonical_frame(
        &service.state,
        "sorafs_node::pop_credentials::PopIssuerCheckpointV1",
    );
    assert_eq!(checkpoint, fs::read(&service.checkpoint_path).unwrap());
}

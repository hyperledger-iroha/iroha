// Actual compiler observations for existing owners in sorafs_manifest::repair::tests.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_fixture_owner_identities() {
    crate::captured_owner_identity_support::check_serialize_only::<
        self::RawQuantityEscalationPolicyV1,
    >(
        "sorafs_manifest::repair::tests::RawQuantityEscalationPolicyV1",
        "sorafs_manifest::repair::tests::RawQuantityEscalationPolicyV1",
        "70445bb65db5d569374bc1cd2f99351d",
    );
    crate::captured_owner_identity_support::check_serialize_only::<self::RawQuantitySlashProposalV1>(
        "sorafs_manifest::repair::tests::RawQuantitySlashProposalV1",
        "sorafs_manifest::repair::tests::RawQuantitySlashProposalV1",
        "7e7db50cbb661a6fb6686de5e35665d4",
    );
}

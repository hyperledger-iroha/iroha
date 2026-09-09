// Actual compiler observations for existing owners in sorafs_manifest::por::tests.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_fixture_owner_identities() {
    crate::captured_owner_identity_support::check_serialize_only::<self::LegacyPorChallengeStatusV1>(
        "sorafs_manifest::por::tests::LegacyPorChallengeStatusV1",
        "sorafs_manifest::por::tests::LegacyPorChallengeStatusV1",
        "d09240eec33d2c348e0c94c4dc123623",
    );
    crate::captured_owner_identity_support::check_serialize_only::<
        self::MissingRepairTaskFieldStatusV1,
    >(
        "sorafs_manifest::por::tests::MissingRepairTaskFieldStatusV1",
        "sorafs_manifest::por::tests::MissingRepairTaskFieldStatusV1",
        "2dbfe19ebab806449a26f4afddd86ed6",
    );
}

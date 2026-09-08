// Actual compiler observations for existing owners in sorafs_manifest::reputation::signed.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::ReputationScoringEvidenceV1>(
        "sorafs_manifest::reputation::signed::ReputationScoringEvidenceV1",
        "sorafs_manifest::reputation::signed::ReputationScoringEvidenceV1",
        "5c5674ac40a0b8b87e77fe1e37121b45",
        "5c5674ac40a0b8b87e77fe1e37121b45",
    );
    crate::captured_owner_identity_support::check_both::<self::ReputationTrustedSignerV1>(
        "sorafs_manifest::reputation::signed::ReputationTrustedSignerV1",
        "sorafs_manifest::reputation::signed::ReputationTrustedSignerV1",
        "8ea8bc74a99e4c44b29827b6360f4b93",
        "8ea8bc74a99e4c44b29827b6360f4b93",
    );
    crate::captured_owner_identity_support::check_both::<self::ReputationSnapshotTrustPolicyV1>(
        "sorafs_manifest::reputation::signed::ReputationSnapshotTrustPolicyV1",
        "sorafs_manifest::reputation::signed::ReputationSnapshotTrustPolicyV1",
        "69f7acfc0376fee61672c94eca3228c9",
        "69f7acfc0376fee61672c94eca3228c9",
    );
    crate::captured_owner_identity_support::check_both::<self::ReputationSnapshotSignatureV1>(
        "sorafs_manifest::reputation::signed::ReputationSnapshotSignatureV1",
        "sorafs_manifest::reputation::signed::ReputationSnapshotSignatureV1",
        "a726ab644cf134030b23e505ba3cfd54",
        "a726ab644cf134030b23e505ba3cfd54",
    );
    crate::captured_owner_identity_support::check_both::<self::SignedReputationSnapshotV1>(
        "sorafs_manifest::reputation::signed::SignedReputationSnapshotV1",
        "sorafs_manifest::reputation::signed::SignedReputationSnapshotV1",
        "8b493741bb293aa8edeecc81e85a4aef",
        "8b493741bb293aa8edeecc81e85a4aef",
    );
}

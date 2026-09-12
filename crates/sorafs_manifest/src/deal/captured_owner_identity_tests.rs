// Actual compiler observations for existing owners in sorafs_manifest::deal.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::MicropaymentPolicyV1>(
        "sorafs_manifest::deal::MicropaymentPolicyV1",
        "sorafs_manifest::deal::MicropaymentPolicyV1",
        "8a039c2819e5dc3f39d9a225de195f6c",
        "8a039c2819e5dc3f39d9a225de195f6c",
    );
    crate::captured_owner_identity_support::check_both::<self::DealMetadataEntry>(
        "sorafs_manifest::deal::DealMetadataEntry",
        "sorafs_manifest::deal::DealMetadataEntry",
        "01fa51e72fef886bcb20936053c3d069",
        "01fa51e72fef886bcb20936053c3d069",
    );
    crate::captured_owner_identity_support::check_both::<self::DealTermsV1>(
        "sorafs_manifest::deal::DealTermsV1",
        "sorafs_manifest::deal::DealTermsV1",
        "b0c754875e9865a789cf6745a81ad85a",
        "b0c754875e9865a789cf6745a81ad85a",
    );
    crate::captured_owner_identity_support::check_both::<self::DealMicropaymentV1>(
        "sorafs_manifest::deal::DealMicropaymentV1",
        "sorafs_manifest::deal::DealMicropaymentV1",
        "e165bb751fa41b281c08ef61554be9ff",
        "e165bb751fa41b281c08ef61554be9ff",
    );
    crate::captured_owner_identity_support::check_both::<self::DealLedgerSnapshotV1>(
        "sorafs_manifest::deal::DealLedgerSnapshotV1",
        "sorafs_manifest::deal::DealLedgerSnapshotV1",
        "7837e746312804ed584eecc5c002ca03",
        "7837e746312804ed584eecc5c002ca03",
    );
    crate::captured_owner_identity_support::check_both::<self::DealSettlementV1>(
        "sorafs_manifest::deal::DealSettlementV1",
        "sorafs_manifest::deal::DealSettlementV1",
        "ec2e0128a81d3915bbc5134e2c4c1542",
        "ec2e0128a81d3915bbc5134e2c4c1542",
    );
    crate::captured_owner_identity_support::check_both::<self::DealSettlementStatusV1>(
        "sorafs_manifest::deal::DealSettlementStatusV1",
        "sorafs_manifest::deal::DealSettlementStatusV1",
        "f8b269b66a8462ee791692852ef0702d",
        "f8b269b66a8462ee791692852ef0702d",
    );
}

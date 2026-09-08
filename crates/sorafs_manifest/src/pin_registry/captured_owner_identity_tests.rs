// Actual compiler observations for existing owners in sorafs_manifest::pin_registry.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::AliasBindingV1>(
        "sorafs_manifest::pin_registry::AliasBindingV1",
        "sorafs_manifest::pin_registry::AliasBindingV1",
        "a011ee3f9ec24299223f7ab5932e8cbe",
        "a011ee3f9ec24299223f7ab5932e8cbe",
    );
    crate::captured_owner_identity_support::check_both::<self::AliasProofBundleV1>(
        "sorafs_manifest::pin_registry::AliasProofBundleV1",
        "sorafs_manifest::pin_registry::AliasProofBundleV1",
        "77f76cdb56f89ddf5fe8f4e73a2b1ca5",
        "77f76cdb56f89ddf5fe8f4e73a2b1ca5",
    );
    crate::captured_owner_identity_support::check_both::<self::ReplicationReceiptV1>(
        "sorafs_manifest::pin_registry::ReplicationReceiptV1",
        "sorafs_manifest::pin_registry::ReplicationReceiptV1",
        "31dcb72350f41d454cbb87c5035d8e6e",
        "31dcb72350f41d454cbb87c5035d8e6e",
    );
    crate::captured_owner_identity_support::check_both::<self::ReplicationReceiptStatus>(
        "sorafs_manifest::pin_registry::ReplicationReceiptStatus",
        "sorafs_manifest::pin_registry::ReplicationReceiptStatus",
        "b9ee22f265f7a8dc81bb477e1aa2e3f7",
        "b9ee22f265f7a8dc81bb477e1aa2e3f7",
    );
    crate::captured_owner_identity_support::check_both::<self::ManifestPolicyV1>(
        "sorafs_manifest::pin_registry::ManifestPolicyV1",
        "sorafs_manifest::pin_registry::ManifestPolicyV1",
        "d7ac0aff05b24595e1ca07849d6c0303",
        "d7ac0aff05b24595e1ca07849d6c0303",
    );
}

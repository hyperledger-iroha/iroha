// Actual compiler observations for existing owners in sorafs_manifest::retention.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::RetentionSourceKindV1>(
        "sorafs_manifest::retention::RetentionSourceKindV1",
        "sorafs_manifest::retention::RetentionSourceKindV1",
        "7bb903059721c8a5cd918b9c32fce379",
        "7bb903059721c8a5cd918b9c32fce379",
    );
    crate::captured_owner_identity_support::check_both::<self::RetentionSourceV1>(
        "sorafs_manifest::retention::RetentionSourceV1",
        "sorafs_manifest::retention::RetentionSourceV1",
        "a87bd5c2f7ba829db49e28cb58936dd4",
        "a87bd5c2f7ba829db49e28cb58936dd4",
    );
}

// Actual compiler observations for existing owners in sorafs_manifest::reference.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::CancelAssetLockWireV1>(
        "sorafs_manifest::reference::CancelAssetLockWireV1",
        "iroha_data_model::isi::escrow::CancelAssetLock",
        "b5c8a665a7de80e2eef75ccb287078fa",
        "b5c8a665a7de80e2eef75ccb287078fa",
    );
    crate::captured_owner_identity_support::check_both::<self::ValidationContextFieldV1>(
        "sorafs_manifest::reference::ValidationContextFieldV1",
        "sorafs_manifest::reference::ValidationContextFieldV1",
        "27b0892c74fa2de816ea36d068bd5bd5",
        "27b0892c74fa2de816ea36d068bd5bd5",
    );
    crate::captured_owner_identity_support::check_both::<self::ValidationInputV1>(
        "sorafs_manifest::reference::ValidationInputV1",
        "sorafs_manifest::reference::ValidationInputV1",
        "7f78d918e25db59bbbda9e41d48530a5",
        "7f78d918e25db59bbbda9e41d48530a5",
    );
    crate::captured_owner_identity_support::check_both::<self::ValidationOutcomeV1>(
        "sorafs_manifest::reference::ValidationOutcomeV1",
        "sorafs_manifest::reference::ValidationOutcomeV1",
        "73332994fb06d8da111be320f1a57a2b",
        "73332994fb06d8da111be320f1a57a2b",
    );
}

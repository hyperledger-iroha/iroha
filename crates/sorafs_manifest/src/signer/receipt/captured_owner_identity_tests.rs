// Actual compiler observations for existing owners in sorafs_manifest::signer::receipt.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseManifestRequestV1>(
        "sorafs_manifest::signer::receipt::SignerReleaseManifestRequestV1",
        "sorafs_manifest::signer::receipt::SignerReleaseManifestRequestV1",
        "6a5743cd455148ba06c95c95f11843bb",
        "6a5743cd455148ba06c95c95f11843bb",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerOperationProvenanceV1>(
        "sorafs_manifest::signer::receipt::SignerOperationProvenanceV1",
        "sorafs_manifest::signer::receipt::SignerOperationProvenanceV1",
        "7868a17d101cba61c3bbc60d580949ae",
        "7868a17d101cba61c3bbc60d580949ae",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerOperationFinalizedAnchorV1>(
        "sorafs_manifest::signer::receipt::SignerOperationFinalizedAnchorV1",
        "sorafs_manifest::signer::receipt::SignerOperationFinalizedAnchorV1",
        "d7907d99a362df9f95931f6417033f57",
        "d7907d99a362df9f95931f6417033f57",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerCompletedOperationV1>(
        "sorafs_manifest::signer::receipt::SignerCompletedOperationV1",
        "sorafs_manifest::signer::receipt::SignerCompletedOperationV1",
        "fc7a4d05eaf5d7c123bebae19a6972c2",
        "fc7a4d05eaf5d7c123bebae19a6972c2",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseManifestReceiptV1>(
        "sorafs_manifest::signer::receipt::SignerReleaseManifestReceiptV1",
        "sorafs_manifest::signer::receipt::SignerReleaseManifestReceiptV1",
        "eb91f33c9a642b6f2aa7bbaf5a8c3fce",
        "eb91f33c9a642b6f2aa7bbaf5a8c3fce",
    );
}

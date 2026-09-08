// Actual compiler observations for existing owners in sorafs_manifest::hybrid_envelope.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::HybridKemBundleV1>(
        "sorafs_manifest::hybrid_envelope::HybridKemBundleV1",
        "sorafs_manifest::hybrid_envelope::HybridKemBundleV1",
        "2ee0605aeee0b7d2520f55242b06e399",
        "2ee0605aeee0b7d2520f55242b06e399",
    );
    crate::captured_owner_identity_support::check_both::<self::HybridPayloadEnvelopeV1>(
        "sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1",
        "sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1",
        "51212a683e2c2c44c3bf9a757d736fca",
        "51212a683e2c2c44c3bf9a757d736fca",
    );
}

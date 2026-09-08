// Actual compiler observations for existing owners in sorafs_manifest::signer::release_evidence.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseEvidencePolicyV1>(
        "sorafs_manifest::signer::release_evidence::SignerReleaseEvidencePolicyV1",
        "sorafs_manifest::signer::release_evidence::SignerReleaseEvidencePolicyV1",
        "c34971e62c71506532dd209f556512ae",
        "c34971e62c71506532dd209f556512ae",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseEvidenceTrustV1>(
        "sorafs_manifest::signer::release_evidence::SignerReleaseEvidenceTrustV1",
        "sorafs_manifest::signer::release_evidence::SignerReleaseEvidenceTrustV1",
        "c8c36188560b032d1aeabddb5004aa7a",
        "c8c36188560b032d1aeabddb5004aa7a",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseStateObservationBodyV1>(
        "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationBodyV1",
        "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationBodyV1",
        "b6d7ab2aa127cf9b49c7e3a1b2a737b8",
        "b6d7ab2aa127cf9b49c7e3a1b2a737b8",
    );
    crate::captured_owner_identity_support::check_both::<self::SignerReleaseStateObservationV1>(
        "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationV1",
        "sorafs_manifest::signer::release_evidence::SignerReleaseStateObservationV1",
        "b168f5582d88de223290b083e9a44884",
        "b168f5582d88de223290b083e9a44884",
    );
}

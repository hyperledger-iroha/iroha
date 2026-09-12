// Actual compiler observations for existing owners in sorafs_manifest::proof_stream.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::ProofStreamRequestV1>(
        "sorafs_manifest::proof_stream::ProofStreamRequestV1",
        "sorafs_manifest::proof_stream::ProofStreamRequestV1",
        "446c6f0dce90a8a51f0fc2852298c812",
        "446c6f0dce90a8a51f0fc2852298c812",
    );
    crate::captured_owner_identity_support::check_both::<self::ProofStreamKind>(
        "sorafs_manifest::proof_stream::ProofStreamKind",
        "sorafs_manifest::proof_stream::ProofStreamKind",
        "31e818559613df8ce1930ff83965e2b0",
        "31e818559613df8ce1930ff83965e2b0",
    );
    crate::captured_owner_identity_support::check_both::<self::ProofStreamTier>(
        "sorafs_manifest::proof_stream::ProofStreamTier",
        "sorafs_manifest::proof_stream::ProofStreamTier",
        "6dfbbcb04c0a89ebcf7199c67c43ac20",
        "6dfbbcb04c0a89ebcf7199c67c43ac20",
    );
}

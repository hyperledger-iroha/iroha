// Actual compiler observations for existing owners in sorafs_manifest::potr.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::PotrReceiptV1>(
        "sorafs_manifest::potr::PotrReceiptV1",
        "sorafs_manifest::potr::PotrReceiptV1",
        "8992f38091ce793b0719d9c89f5cbfd2",
        "8992f38091ce793b0719d9c89f5cbfd2",
    );
    crate::captured_owner_identity_support::check_both::<self::PotrSignatureV1>(
        "sorafs_manifest::potr::PotrSignatureV1",
        "sorafs_manifest::potr::PotrSignatureV1",
        "541ef267d11582145e17a78344bf4d3f",
        "541ef267d11582145e17a78344bf4d3f",
    );
    crate::captured_owner_identity_support::check_both::<self::PotrSignatureAlgorithm>(
        "sorafs_manifest::potr::PotrSignatureAlgorithm",
        "sorafs_manifest::potr::PotrSignatureAlgorithm",
        "02aa816f0c9ab018b792593ae0495576",
        "02aa816f0c9ab018b792593ae0495576",
    );
    crate::captured_owner_identity_support::check_both::<self::PotrStatus>(
        "sorafs_manifest::potr::PotrStatus",
        "sorafs_manifest::potr::PotrStatus",
        "44edbecd1a4a07b9ff8755385cfd2c7a",
        "44edbecd1a4a07b9ff8755385cfd2c7a",
    );
}

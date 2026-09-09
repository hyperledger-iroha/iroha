// Actual compiler observations for existing owners in sorafs_manifest::pricing::signed.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::PricingTrustedSignerV1>(
        "sorafs_manifest::pricing::signed::PricingTrustedSignerV1",
        "sorafs_manifest::pricing::signed::PricingTrustedSignerV1",
        "9a019158d55d03b49836ccb600b65fab",
        "9a019158d55d03b49836ccb600b65fab",
    );
    crate::captured_owner_identity_support::check_both::<self::PricingTrustPolicyV1>(
        "sorafs_manifest::pricing::signed::PricingTrustPolicyV1",
        "sorafs_manifest::pricing::signed::PricingTrustPolicyV1",
        "51c8b4fd7593ca2b2c2a1cc0c4d583be",
        "51c8b4fd7593ca2b2c2a1cc0c4d583be",
    );
    crate::captured_owner_identity_support::check_both::<self::PricingManifestSignatureV1>(
        "sorafs_manifest::pricing::signed::PricingManifestSignatureV1",
        "sorafs_manifest::pricing::signed::PricingManifestSignatureV1",
        "041f4361d3f6720fda0068e0da6a017b",
        "041f4361d3f6720fda0068e0da6a017b",
    );
    crate::captured_owner_identity_support::check_both::<self::GovernedPricingManifestV1>(
        "sorafs_manifest::pricing::signed::GovernedPricingManifestV1",
        "sorafs_manifest::pricing::signed::GovernedPricingManifestV1",
        "bfcd70e8f588fce6b20bc6194fb5359c",
        "bfcd70e8f588fce6b20bc6194fb5359c",
    );
    crate::captured_owner_identity_support::check_both::<self::GovernedPricingAdmissionV1>(
        "sorafs_manifest::pricing::signed::GovernedPricingAdmissionV1",
        "sorafs_manifest::pricing::signed::GovernedPricingAdmissionV1",
        "a73ee4363b4e37586a2898c76339b3be",
        "a73ee4363b4e37586a2898c76339b3be",
    );
    crate::captured_owner_identity_support::check_both::<self::GovernedPricingSeriesV1>(
        "sorafs_manifest::pricing::signed::GovernedPricingSeriesV1",
        "sorafs_manifest::pricing::signed::GovernedPricingSeriesV1",
        "4fff581d70221ae65feea3ba18d84737",
        "4fff581d70221ae65feea3ba18d84737",
    );
}

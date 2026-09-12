// Actual compiler observations for existing owners in sorafs_manifest::pricing.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::PricingManifestV1>(
        "sorafs_manifest::pricing::PricingManifestV1",
        "sorafs_manifest::pricing::PricingManifestV1",
        "4973cc510df1dac1bb85ca48e31624e4",
        "4973cc510df1dac1bb85ca48e31624e4",
    );
    crate::captured_owner_identity_support::check_both::<self::PricingTierV1>(
        "sorafs_manifest::pricing::PricingTierV1",
        "sorafs_manifest::pricing::PricingTierV1",
        "55bf9cd369c700d653b4cc48b7372499",
        "55bf9cd369c700d653b4cc48b7372499",
    );
    crate::captured_owner_identity_support::check_both::<self::CreditPolicyV1>(
        "sorafs_manifest::pricing::CreditPolicyV1",
        "sorafs_manifest::pricing::CreditPolicyV1",
        "ad1f65e27111fb26a76a965e43c5a506",
        "ad1f65e27111fb26a76a965e43c5a506",
    );
    crate::captured_owner_identity_support::check_both::<self::BondPolicyV1>(
        "sorafs_manifest::pricing::BondPolicyV1",
        "sorafs_manifest::pricing::BondPolicyV1",
        "0be639944d9cec669c13e88c1cde08c4",
        "0be639944d9cec669c13e88c1cde08c4",
    );
    crate::captured_owner_identity_support::check_both::<self::PricingMicropaymentPolicyV1>(
        "sorafs_manifest::pricing::PricingMicropaymentPolicyV1",
        "sorafs_manifest::pricing::PricingMicropaymentPolicyV1",
        "b1e5919a171039ae8b9522e4775a319d",
        "b1e5919a171039ae8b9522e4775a319d",
    );
}

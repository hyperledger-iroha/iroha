// Actual compiler observations for existing owners in sorafs_manifest::hedging.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::HedgingFeedStatusV1>(
        "sorafs_manifest::hedging::HedgingFeedStatusV1",
        "sorafs_manifest::hedging::HedgingFeedStatusV1",
        "3f33c13817635d6088fb30207d43c0bd",
        "3f33c13817635d6088fb30207d43c0bd",
    );
    crate::captured_owner_identity_support::check_both::<self::BillingLineDirectionV1>(
        "sorafs_manifest::hedging::BillingLineDirectionV1",
        "sorafs_manifest::hedging::BillingLineDirectionV1",
        "4fa23321d0423566a87433f8e37138b8",
        "4fa23321d0423566a87433f8e37138b8",
    );
    crate::captured_owner_identity_support::check_both::<self::BillingLineItemKindV1>(
        "sorafs_manifest::hedging::BillingLineItemKindV1",
        "sorafs_manifest::hedging::BillingLineItemKindV1",
        "e62222605dc579d13eee3d0c53f4b808",
        "e62222605dc579d13eee3d0c53f4b808",
    );
    crate::captured_owner_identity_support::check_both::<self::HedgingPriceFeedV1>(
        "sorafs_manifest::hedging::HedgingPriceFeedV1",
        "sorafs_manifest::hedging::HedgingPriceFeedV1",
        "f7608ce7453eec1ddbf48e9960ce8abd",
        "f7608ce7453eec1ddbf48e9960ce8abd",
    );
    crate::captured_owner_identity_support::check_both::<self::HedgingReferencePriceDecisionV1>(
        "sorafs_manifest::hedging::HedgingReferencePriceDecisionV1",
        "sorafs_manifest::hedging::HedgingReferencePriceDecisionV1",
        "1c86da3886ac282e47bc14a2aec4a4a9",
        "1c86da3886ac282e47bc14a2aec4a4a9",
    );
    crate::captured_owner_identity_support::check_both::<self::BillingLineItemV1>(
        "sorafs_manifest::hedging::BillingLineItemV1",
        "sorafs_manifest::hedging::BillingLineItemV1",
        "8ffbfdf5a7a75fb716dfd4132f5d4b7d",
        "8ffbfdf5a7a75fb716dfd4132f5d4b7d",
    );
    crate::captured_owner_identity_support::check_both::<self::BillingStatementV1>(
        "sorafs_manifest::hedging::BillingStatementV1",
        "sorafs_manifest::hedging::BillingStatementV1",
        "69991b87c17fc1da291224809ee583c5",
        "69991b87c17fc1da291224809ee583c5",
    );
}

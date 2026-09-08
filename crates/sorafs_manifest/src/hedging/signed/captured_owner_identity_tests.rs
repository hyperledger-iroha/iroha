// Actual compiler observations for existing owners in sorafs_manifest::hedging::signed.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::HedgingFeedBindingV1>(
        "sorafs_manifest::hedging::signed::HedgingFeedBindingV1",
        "sorafs_manifest::hedging::signed::HedgingFeedBindingV1",
        "e11a45665360a40f2d949b56a4da786a",
        "e11a45665360a40f2d949b56a4da786a",
    );
    crate::captured_owner_identity_support::check_both::<self::HedgingTrustedSignerV1>(
        "sorafs_manifest::hedging::signed::HedgingTrustedSignerV1",
        "sorafs_manifest::hedging::signed::HedgingTrustedSignerV1",
        "fb28537d0e8f7e1d4ef4f757b4fd73f5",
        "fb28537d0e8f7e1d4ef4f757b4fd73f5",
    );
    crate::captured_owner_identity_support::check_both::<self::HedgingFeedTrustPolicyV1>(
        "sorafs_manifest::hedging::signed::HedgingFeedTrustPolicyV1",
        "sorafs_manifest::hedging::signed::HedgingFeedTrustPolicyV1",
        "91b9809e6676b50cf7c6d57cf48bf8d7",
        "91b9809e6676b50cf7c6d57cf48bf8d7",
    );
    crate::captured_owner_identity_support::check_both::<self::SignedHedgingPriceFeedV1>(
        "sorafs_manifest::hedging::signed::SignedHedgingPriceFeedV1",
        "sorafs_manifest::hedging::signed::SignedHedgingPriceFeedV1",
        "a6ccdbfc399c59c3754ce873dafdce9b",
        "a6ccdbfc399c59c3754ce873dafdce9b",
    );
    crate::captured_owner_identity_support::check_both::<
        self::GovernedHedgingReferencePriceDecisionV1,
    >(
        "sorafs_manifest::hedging::signed::GovernedHedgingReferencePriceDecisionV1",
        "sorafs_manifest::hedging::signed::GovernedHedgingReferencePriceDecisionV1",
        "e838610cbcdc20125936b34d83d662c2",
        "e838610cbcdc20125936b34d83d662c2",
    );
    crate::captured_owner_identity_support::check_both::<self::GovernedBillingStatementV1>(
        "sorafs_manifest::hedging::signed::GovernedBillingStatementV1",
        "sorafs_manifest::hedging::signed::GovernedBillingStatementV1",
        "cdd7de8e5b9fd4bc174c45ed1d11088e",
        "cdd7de8e5b9fd4bc174c45ed1d11088e",
    );
    crate::captured_owner_identity_support::check_both::<self::SignedHedgingFeedAdmissionV1>(
        "sorafs_manifest::hedging::signed::SignedHedgingFeedAdmissionV1",
        "sorafs_manifest::hedging::signed::SignedHedgingFeedAdmissionV1",
        "296e0ecbdd93e5a565e4a17775a6ce90",
        "296e0ecbdd93e5a565e4a17775a6ce90",
    );
    crate::captured_owner_identity_support::check_both::<self::SignedHedgingFeedLedgerV1>(
        "sorafs_manifest::hedging::signed::SignedHedgingFeedLedgerV1",
        "sorafs_manifest::hedging::signed::SignedHedgingFeedLedgerV1",
        "11f1f62929508ac4365de1ea84d26be4",
        "11f1f62929508ac4365de1ea84d26be4",
    );
}

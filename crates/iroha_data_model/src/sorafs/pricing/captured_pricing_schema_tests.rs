//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::TierRate>(
        "iroha_data_model::sorafs::pricing::TierRate",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CollateralPolicy>(
        "iroha_data_model::sorafs::pricing::CollateralPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CreditPolicy>(
        "iroha_data_model::sorafs::pricing::CreditPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CommitmentDiscountTier>(
        "iroha_data_model::sorafs::pricing::CommitmentDiscountTier",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DiscountSchedule>(
        "iroha_data_model::sorafs::pricing::DiscountSchedule",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PricingScheduleRecord>(
        "iroha_data_model::sorafs::pricing::PricingScheduleRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProviderCreditRecord>(
        "iroha_data_model::sorafs::pricing::ProviderCreditRecord",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

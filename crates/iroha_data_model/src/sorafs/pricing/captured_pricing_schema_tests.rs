//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::TierRate>(
        "iroha_data_model::sorafs::pricing::TierRate",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CollateralPolicy>(
        "iroha_data_model::sorafs::pricing::CollateralPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CreditPolicy>(
        "iroha_data_model::sorafs::pricing::CreditPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CommitmentDiscountTier>(
        "iroha_data_model::sorafs::pricing::CommitmentDiscountTier",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DiscountSchedule>(
        "iroha_data_model::sorafs::pricing::DiscountSchedule",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PricingScheduleRecord>(
        "iroha_data_model::sorafs::pricing::PricingScheduleRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ProviderCreditRecord>(
        "iroha_data_model::sorafs::pricing::ProviderCreditRecord",
    );
}

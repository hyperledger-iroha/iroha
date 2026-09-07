//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ViralRewardBudget>(
        "iroha_data_model::social::ViralRewardBudget",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralDailyCounter>(
        "iroha_data_model::social::ViralDailyCounter",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralCampaignBudget>(
        "iroha_data_model::social::ViralCampaignBudget",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralEscrowRecord>(
        "iroha_data_model::social::ViralEscrowRecord",
    );
}

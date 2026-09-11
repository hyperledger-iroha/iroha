//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ViralRewardBudget>(
        "iroha_data_model::social::ViralRewardBudget",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralDailyCounter>(
        "iroha_data_model::social::ViralDailyCounter",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralCampaignBudget>(
        "iroha_data_model::social::ViralCampaignBudget",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralEscrowRecord>(
        "iroha_data_model::social::ViralEscrowRecord",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

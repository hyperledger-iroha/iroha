//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SocialEvent>(
        "iroha_data_model::events::data::social::SocialEvent",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralRewardApplied>(
        "iroha_data_model::events::data::social::ViralRewardApplied",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralEscrowCreated>(
        "iroha_data_model::events::data::social::ViralEscrowCreated",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralEscrowReleased>(
        "iroha_data_model::events::data::social::ViralEscrowReleased",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ViralEscrowCancelled>(
        "iroha_data_model::events::data::social::ViralEscrowCancelled",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

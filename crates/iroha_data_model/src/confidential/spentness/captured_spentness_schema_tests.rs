//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialSpentnessPathV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessPathV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialSpentnessStateKindV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessStateKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialSpentnessStateV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessStateV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialSpentnessCheckpointV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessCheckpointV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialSpentnessProofV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessProofV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

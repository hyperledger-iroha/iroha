//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialSpentnessPathV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessPathV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialSpentnessStateKindV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessStateKindV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialSpentnessStateV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessStateV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialSpentnessCheckpointV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessCheckpointV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialSpentnessProofV1>(
        "iroha_data_model::confidential::spentness::ConfidentialSpentnessProofV1",
    );
}

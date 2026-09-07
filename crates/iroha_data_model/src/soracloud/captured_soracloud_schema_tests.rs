//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::SoracloudTxInstruction>(
        "iroha_data_model::soracloud::SoracloudTxInstruction",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SoracloudMutationDraftResponse>(
        "iroha_data_model::soracloud::SoracloudMutationDraftResponse",
    );
}

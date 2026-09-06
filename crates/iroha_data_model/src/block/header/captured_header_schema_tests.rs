//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::BlockHeaderConsensusProjectionV1>(
        "iroha_data_model::block::header::BlockHeaderConsensusProjectionV1",
    );
}

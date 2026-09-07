//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::FinalizedNextEpochSnapshot>(
        "iroha_data_model::block::consensus_v2::finality::FinalizedNextEpochSnapshot",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::V2FinalityArtifact>(
        "iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact",
    );
}

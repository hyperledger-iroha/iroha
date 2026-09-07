//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ChunkRole>(
        "iroha_data_model::da::manifest::ChunkRole",
    );
}

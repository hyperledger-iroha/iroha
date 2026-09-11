//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] =
    &[crate::captured_schema_tests::Case::bidirectional::<
        super::ChunkRole,
    >("iroha_data_model::da::manifest::ChunkRole")];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

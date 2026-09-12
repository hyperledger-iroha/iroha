//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::FinalizedNextEpochSnapshot>(
        "iroha_data_model::block::consensus_v2::finality::FinalizedNextEpochSnapshot",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::V2FinalityArtifact>(
        "iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

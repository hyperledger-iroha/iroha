//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::LanePrivacyProof>(
        "iroha_data_model::nexus::privacy::LanePrivacyProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LanePrivacyMerkleWitness>(
        "iroha_data_model::nexus::privacy::LanePrivacyMerkleWitness",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LanePrivacyWitness>(
        "iroha_data_model::nexus::privacy::LanePrivacyWitness",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

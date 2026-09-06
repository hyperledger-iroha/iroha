//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::LanePrivacyProof>(
        "iroha_data_model::nexus::privacy::LanePrivacyProof",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::LanePrivacyMerkleWitness>(
        "iroha_data_model::nexus::privacy::LanePrivacyMerkleWitness",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::LanePrivacyWitness>(
        "iroha_data_model::nexus::privacy::LanePrivacyWitness",
    );
}

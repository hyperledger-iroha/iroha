//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::ConsensusParametersFingerprintInput>(
        "iroha_data_model::block::consensus_v2::fingerprint::ConsensusParametersFingerprintInput",
    );
    crate::captured_schema_tests::assert_serialize::<super::NposGenesisFingerprintInput>(
        "iroha_data_model::block::consensus_v2::fingerprint::NposGenesisFingerprintInput",
    );
}

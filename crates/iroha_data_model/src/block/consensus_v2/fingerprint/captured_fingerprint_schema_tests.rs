//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::serialize::<super::ConsensusParametersFingerprintInput>(
        "iroha_data_model::block::consensus_v2::fingerprint::ConsensusParametersFingerprintInput",
    ),
    crate::captured_schema_tests::Case::serialize::<super::NposGenesisFingerprintInput>(
        "iroha_data_model::block::consensus_v2::fingerprint::NposGenesisFingerprintInput",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

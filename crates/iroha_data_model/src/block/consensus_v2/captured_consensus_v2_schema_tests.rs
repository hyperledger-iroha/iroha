//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ValidatorPower>(
        "iroha_data_model::block::consensus_v2::ValidatorPower",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DataAvailabilityLayout>(
        "iroha_data_model::block::consensus_v2::DataAvailabilityLayout",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PayloadEncoding>(
        "iroha_data_model::block::consensus_v2::PayloadEncoding",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiV2GenesisContextParameters>(
        "iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::HeightContextId>(
        "iroha_data_model::block::consensus_v2::HeightContextId",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);

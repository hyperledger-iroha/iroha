//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::KagemushaDeviceMintStageCommandV1>(
        "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageCommandV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::KagemushaDeviceMintStageResultV1>(
        "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageResultV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

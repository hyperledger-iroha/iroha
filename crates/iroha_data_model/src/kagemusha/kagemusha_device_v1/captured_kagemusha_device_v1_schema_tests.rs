//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::KagemushaDeviceMintStageCommandV1>(
        "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageCommandV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::KagemushaDeviceMintStageResultV1>(
        "iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageResultV1",
    );
}

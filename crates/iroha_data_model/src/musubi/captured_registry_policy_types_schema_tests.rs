//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::MusubiRegistryAdmissionModeV1>(
        "iroha_data_model::musubi::MusubiRegistryAdmissionModeV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::MusubiRegistryPolicyV1>(
        "iroha_data_model::musubi::MusubiRegistryPolicyV1",
    );
}

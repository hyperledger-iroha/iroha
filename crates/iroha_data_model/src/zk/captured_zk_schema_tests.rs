//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::BackendTag>(
        "iroha_data_model::zk::BackendTag",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::OpenVerifyEnvelope>(
        "iroha_data_model::zk::OpenVerifyEnvelope",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::StarkFriOpenProofV1>(
        "iroha_data_model::zk::StarkFriOpenProofV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ZkAcePrivacyPublicInputsV1>(
        "iroha_data_model::zk::ZkAcePrivacyPublicInputsV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ZkAcePackedBytesV1>(
        "iroha_data_model::zk::ZkAcePackedBytesV1",
    );
}

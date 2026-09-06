//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyProofBytesV1>(
        "iroha_data_model::privacy::PrivacyProofBytesV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IrohaZkAmsProofV1>(
        "iroha_data_model::privacy::IrohaZkAmsProofV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyProofV1>(
        "iroha_data_model::privacy::PrivacyProofV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyProofEnvelopeV1>(
        "iroha_data_model::privacy::PrivacyProofEnvelopeV1",
    );
}

//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialComputeMechanism>(
        "iroha_data_model::da::confidential_compute::ConfidentialComputeMechanism",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConfidentialComputePolicy>(
        "iroha_data_model::da::confidential_compute::ConfidentialComputePolicy",
    );
}

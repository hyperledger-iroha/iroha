//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::DaIngestAdmissionLaneV1>(
        "iroha_data_model::da::ingest::DaIngestAdmissionLaneV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaIngestAdmissionPolicyV1>(
        "iroha_data_model::da::ingest::DaIngestAdmissionPolicyV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaIngestSignatureV1>(
        "iroha_data_model::da::ingest::DaIngestSignatureV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinScopeSignatureV1>(
        "iroha_data_model::da::ingest::DaPinScopeSignatureV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaIngestAuthorizationV1>(
        "iroha_data_model::da::ingest::DaIngestAuthorizationV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinScopeV1>(
        "iroha_data_model::da::ingest::DaPinScopeV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinScopeAuthorizationV1>(
        "iroha_data_model::da::ingest::DaPinScopeAuthorizationV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaIngestRequestIntentV1>(
        "iroha_data_model::da::ingest::DaIngestRequestIntentV1",
    );
}

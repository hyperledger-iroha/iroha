//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::DaIngestAdmissionLaneV1>(
        "iroha_data_model::da::ingest::DaIngestAdmissionLaneV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaIngestAdmissionPolicyV1>(
        "iroha_data_model::da::ingest::DaIngestAdmissionPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaIngestSignatureV1>(
        "iroha_data_model::da::ingest::DaIngestSignatureV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinScopeSignatureV1>(
        "iroha_data_model::da::ingest::DaPinScopeSignatureV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaIngestAuthorizationV1>(
        "iroha_data_model::da::ingest::DaIngestAuthorizationV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinScopeV1>(
        "iroha_data_model::da::ingest::DaPinScopeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinScopeAuthorizationV1>(
        "iroha_data_model::da::ingest::DaPinScopeAuthorizationV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaIngestRequestIntentV1>(
        "iroha_data_model::da::ingest::DaIngestRequestIntentV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

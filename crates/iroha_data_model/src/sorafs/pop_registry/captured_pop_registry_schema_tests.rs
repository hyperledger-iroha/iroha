//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::PopIssuerPolicyV1>(
        "iroha_data_model::sorafs::pop_registry::PopIssuerPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopCredentialCommitmentV1>(
        "iroha_data_model::sorafs::pop_registry::PopCredentialCommitmentV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopCredentialCommitmentBatchV1>(
        "iroha_data_model::sorafs::pop_registry::PopCredentialCommitmentBatchV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopIssuerPolicyRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopIssuerPolicyRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopCredentialCommitmentRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopCredentialCommitmentRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopCommitmentRootRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopCommitmentRootRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRevocationPublicationRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopRevocationPublicationRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRegistryRevocationReasonV1>(
        "iroha_data_model::sorafs::pop_registry::PopRegistryRevocationReasonV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRevocationRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopRevocationRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRegistryAuditEventKindV1>(
        "iroha_data_model::sorafs::pop_registry::PopRegistryAuditEventKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRegistryAuditDigestRecordV1>(
        "iroha_data_model::sorafs::pop_registry::PopRegistryAuditDigestRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PopRegistryStatusV1>(
        "iroha_data_model::sorafs::pop_registry::PopRegistryStatusV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

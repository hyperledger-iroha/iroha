//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::BlobDigest>(
        "iroha_data_model::da::types::BlobDigest",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::StorageTicketId>(
        "iroha_data_model::da::types::StorageTicketId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BlobClass>(
        "iroha_data_model::da::types::BlobClass",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BlobCodec>(
        "iroha_data_model::da::types::BlobCodec",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::Compression>(
        "iroha_data_model::da::types::Compression",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GovernanceTag>(
        "iroha_data_model::da::types::GovernanceTag",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FecScheme>(
        "iroha_data_model::da::types::FecScheme",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ErasureProfile>(
        "iroha_data_model::da::types::ErasureProfile",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RetentionPolicy>(
        "iroha_data_model::da::types::RetentionPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExtraMetadata>(
        "iroha_data_model::da::types::ExtraMetadata",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MetadataEncryption>(
        "iroha_data_model::da::types::MetadataEncryption",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MetadataCipherEnvelope>(
        "iroha_data_model::da::types::MetadataCipherEnvelope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MetadataEntry>(
        "iroha_data_model::da::types::MetadataEntry",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MetadataVisibility>(
        "iroha_data_model::da::types::MetadataVisibility",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaRentPolicyV1>(
        "iroha_data_model::da::types::DaRentPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaRentQuote>(
        "iroha_data_model::da::types::DaRentQuote",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaRentLedgerProjection>(
        "iroha_data_model::da::types::DaRentLedgerProjection",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

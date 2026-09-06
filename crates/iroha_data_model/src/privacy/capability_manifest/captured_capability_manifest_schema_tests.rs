//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyOperationSchemaV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyOperationSchemaV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyExecutionModeV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyExecutionModeV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyFeatureMaskV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyFeatureMaskV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyCapabilityReadinessV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyCapabilityReadinessV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyCapabilityUnavailableReasonV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyCapabilityUnavailableReasonV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyExact12CapabilityRowV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyExact12CapabilityRowV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PrivacyExact12CapabilityManifestV1>(
        "iroha_data_model::privacy::capability_manifest::PrivacyExact12CapabilityManifestV1",
    );
}

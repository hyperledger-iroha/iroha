//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::GarLicenseSetV1>(
        "iroha_data_model::sorafs::gar::GarLicenseSetV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarCdnPolicyV1>(
        "iroha_data_model::sorafs::gar::GarCdnPolicyV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarModerationDirectiveV1>(
        "iroha_data_model::sorafs::gar::GarModerationDirectiveV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarModerationAction>(
        "iroha_data_model::sorafs::gar::GarModerationAction",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarMetricsPolicyV1>(
        "iroha_data_model::sorafs::gar::GarMetricsPolicyV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarPolicyPayloadV1>(
        "iroha_data_model::sorafs::gar::GarPolicyPayloadV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarEnforcementActionV1>(
        "iroha_data_model::sorafs::gar::GarEnforcementActionV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GarEnforcementReceiptV1>(
        "iroha_data_model::sorafs::gar::GarEnforcementReceiptV1",
    );
}

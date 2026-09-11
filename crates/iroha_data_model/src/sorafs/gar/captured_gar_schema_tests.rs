//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::GarLicenseSetV1>(
        "iroha_data_model::sorafs::gar::GarLicenseSetV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarCdnPolicyV1>(
        "iroha_data_model::sorafs::gar::GarCdnPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarModerationDirectiveV1>(
        "iroha_data_model::sorafs::gar::GarModerationDirectiveV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarModerationAction>(
        "iroha_data_model::sorafs::gar::GarModerationAction",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarMetricsPolicyV1>(
        "iroha_data_model::sorafs::gar::GarMetricsPolicyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarPolicyPayloadV1>(
        "iroha_data_model::sorafs::gar::GarPolicyPayloadV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarEnforcementActionV1>(
        "iroha_data_model::sorafs::gar::GarEnforcementActionV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GarEnforcementReceiptV1>(
        "iroha_data_model::sorafs::gar::GarEnforcementReceiptV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

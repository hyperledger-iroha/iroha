//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetGarAbuseCountV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetGarAbuseCountV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetGarAbuseShareV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetGarAbuseShareV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyModeV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyModeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyPrioShareV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyPrioShareV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetLatencyPercentileV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetLatencyPercentileV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyBucketMetricsV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyBucketMetricsV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacySuppressionReasonV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacySuppressionReasonV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventKindV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventHandshakeSuccessV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventHandshakeSuccessV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventHandshakeFailureV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventHandshakeFailureV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventThrottleV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventThrottleV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventActiveSampleV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventActiveSampleV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventVerifiedBytesV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventVerifiedBytesV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyEventGarAbuseCategoryV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyEventGarAbuseCategoryV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyHandshakeFailureV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyHandshakeFailureV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPowFailureReasonV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPowFailureReasonV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPowFailureCountV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPowFailureCountV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoranetPrivacyThrottleScopeV1>(
        "iroha_data_model::soranet::privacy_metrics::SoranetPrivacyThrottleScopeV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

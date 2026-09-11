//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonBlockIdExtV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonBlockIdExtV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonFinalizedMasterchainBlockV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonFinalizedMasterchainBlockV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonAccountStateReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonAccountStateReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonReplayForestReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonReplayForestReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonBridgePendingReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonBridgePendingReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonDeploymentReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonDeploymentReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonRouteStorageReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonRouteStorageReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonMasterStorageReadbackV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonMasterStorageReadbackV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonBreakerObservationRecordV1>(
        "iroha_data_model::bridge::sccp_ton_breaker::SccpTonBreakerObservationRecordV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

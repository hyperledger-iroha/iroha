//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SccpNetworkV1>(
        "iroha_data_model::bridge::sccp::SccpNetworkV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpLaneIdV1>(
        "iroha_data_model::bridge::sccp::SccpLaneIdV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundMessageContextV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundMessageContextV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundMessageKeyV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundMessageKeyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundMessageIndexKeyV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundMessageIndexKeyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundMessageDescriptorV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundMessageDescriptorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundPendingMessageRecordV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundPendingMessageRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpOutboundPendingUsageV1>(
        "iroha_data_model::bridge::sccp::SccpOutboundPendingUsageV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpInboundAnchorHighWaterKeyV1>(
        "iroha_data_model::bridge::sccp::SccpInboundAnchorHighWaterKeyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpEvmSourceEmitterV1>(
        "iroha_data_model::bridge::sccp::SccpEvmSourceEmitterV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTronSourceEmitterV1>(
        "iroha_data_model::bridge::sccp::SccpTronSourceEmitterV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonAddressV1>(
        "iroha_data_model::bridge::sccp::SccpTonAddressV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpTonSourceEmitterV1>(
        "iroha_data_model::bridge::sccp::SccpTonSourceEmitterV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpSourceEmitterV1>(
        "iroha_data_model::bridge::sccp::SccpSourceEmitterV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpSourceIdentityV1>(
        "iroha_data_model::bridge::sccp::SccpSourceIdentityV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

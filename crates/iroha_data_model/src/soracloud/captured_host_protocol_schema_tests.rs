//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudHostOperationV1>(
        "iroha_data_model::soracloud::SoracloudHostOperationV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudHostRequestEnvelopeV1>(
        "iroha_data_model::soracloud::SoracloudHostRequestEnvelopeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudHostRequestPayloadV1>(
        "iroha_data_model::soracloud::SoracloudHostRequestPayloadV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudHostResponseEnvelopeV1>(
        "iroha_data_model::soracloud::SoracloudHostResponseEnvelopeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudHostResponsePayloadV1>(
        "iroha_data_model::soracloud::SoracloudHostResponsePayloadV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadCommittedStateRequestV1>(
        "iroha_data_model::soracloud::SoracloudReadCommittedStateRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadCommittedStateResponseV1>(
        "iroha_data_model::soracloud::SoracloudReadCommittedStateResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudEmitStateMutationRequestV1>(
        "iroha_data_model::soracloud::SoracloudEmitStateMutationRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudEmitStateMutationResponseV1>(
        "iroha_data_model::soracloud::SoracloudEmitStateMutationResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudEmitMailboxMessageRequestV1>(
        "iroha_data_model::soracloud::SoracloudEmitMailboxMessageRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudEmitMailboxMessageResponseV1>(
        "iroha_data_model::soracloud::SoracloudEmitMailboxMessageResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudAppendJournalRequestV1>(
        "iroha_data_model::soracloud::SoracloudAppendJournalRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudAppendJournalResponseV1>(
        "iroha_data_model::soracloud::SoracloudAppendJournalResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudPublishCheckpointRequestV1>(
        "iroha_data_model::soracloud::SoracloudPublishCheckpointRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudPublishCheckpointResponseV1>(
        "iroha_data_model::soracloud::SoracloudPublishCheckpointResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadConfigRequestV1>(
        "iroha_data_model::soracloud::SoracloudReadConfigRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadConfigResponseV1>(
        "iroha_data_model::soracloud::SoracloudReadConfigResponseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadSecretEnvelopeRequestV1>(
        "iroha_data_model::soracloud::SoracloudReadSecretEnvelopeRequestV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SoracloudReadSecretEnvelopeResponseV1>(
        "iroha_data_model::soracloud::SoracloudReadSecretEnvelopeResponseV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ExternalExecutionRouteRole>(
        "iroha_data_model::block::execution_context::ExternalExecutionRouteRole",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ExternalExecutionRouteLeg>(
        "iroha_data_model::block::execution_context::ExternalExecutionRouteLeg",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ExternalExecutionContext>(
        "iroha_data_model::block::execution_context::ExternalExecutionContext",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CertifiedMergeLedgerReference>(
        "iroha_data_model::block::execution_context::CertifiedMergeLedgerReference",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AutonomousLanePayloadEnvelopeV1>(
        "iroha_data_model::block::execution_context::AutonomousLanePayloadEnvelopeV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::BlockExecutionContextBundle>(
        "iroha_data_model::block::execution_context::BlockExecutionContextBundle",
    );
}

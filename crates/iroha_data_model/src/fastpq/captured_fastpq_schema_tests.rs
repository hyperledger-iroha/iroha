//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::Case::bidirectional::<super::TransferTranscript>(
        "iroha_data_model::fastpq::TransferTranscript",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::TransferDeltaTranscript>(
        "iroha_data_model::fastpq::TransferDeltaTranscript",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::TransferSmtWitness>(
        "iroha_data_model::fastpq::TransferSmtWitness",
    )
    .check();
    // The final V1 transition batch deliberately replaces the captured pre-release frame.
    // `transition_batch_schema_rejects_the_pre_release_header` covers its identity.
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqStateTransition>(
        "iroha_data_model::fastpq::FastpqStateTransition",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqOperationKind>(
        "iroha_data_model::fastpq::FastpqOperationKind",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqPublicInputs>(
        "iroha_data_model::fastpq::FastpqPublicInputs",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::TransferTranscriptBundle>(
        "iroha_data_model::fastpq::TransferTranscriptBundle",
    )
    .check();
}

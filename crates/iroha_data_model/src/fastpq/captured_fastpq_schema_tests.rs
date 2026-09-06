//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::TransferTranscript>(
        "iroha_data_model::fastpq::TransferTranscript",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TransferDeltaTranscript>(
        "iroha_data_model::fastpq::TransferDeltaTranscript",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TransferSmtWitness>(
        "iroha_data_model::fastpq::TransferSmtWitness",
    );
    // The final V1 transition batch deliberately replaces the captured pre-release frame.
    // `transition_batch_schema_rejects_the_pre_release_header` covers its identity.
    crate::captured_schema_tests::assert_bidirectional::<super::FastpqStateTransition>(
        "iroha_data_model::fastpq::FastpqStateTransition",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::FastpqOperationKind>(
        "iroha_data_model::fastpq::FastpqOperationKind",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::FastpqPublicInputs>(
        "iroha_data_model::fastpq::FastpqPublicInputs",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TransferTranscriptBundle>(
        "iroha_data_model::fastpq::TransferTranscriptBundle",
    );
}

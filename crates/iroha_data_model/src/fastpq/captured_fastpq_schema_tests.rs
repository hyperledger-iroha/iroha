//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::TransferTranscript>(
        "iroha_data_model::fastpq::TransferTranscript",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TransferDeltaTranscript>(
        "iroha_data_model::fastpq::TransferDeltaTranscript",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TransferSmtWitness>(
        "iroha_data_model::fastpq::TransferSmtWitness",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqStateTransition>(
        "iroha_data_model::fastpq::FastpqStateTransition",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqOperationKind>(
        "iroha_data_model::fastpq::FastpqOperationKind",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FastpqPublicInputs>(
        "iroha_data_model::fastpq::FastpqPublicInputs",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TransferTranscriptBundle>(
        "iroha_data_model::fastpq::TransferTranscriptBundle",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }

    // The final V1 transition batch deliberately replaces the captured pre-release frame.
    // `transition_batch_schema_rejects_the_pre_release_header` covers its identity.
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);

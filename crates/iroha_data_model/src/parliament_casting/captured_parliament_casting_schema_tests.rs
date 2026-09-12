//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ParliamentTimedOvnCastingPhaseV1>(
        "iroha_data_model::parliament_casting::ParliamentTimedOvnCastingPhaseV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnRegistrationCorpusCommitmentV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnRegistrationCorpusCommitmentV1"),
    crate::captured_schema_tests::Case::bidirectional::<super::ParliamentTimedOvnReleaseBindingV1>(
        "iroha_data_model::parliament_casting::ParliamentTimedOvnReleaseBindingV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnCastingContextBindingV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnCastingSnapshotCommitmentV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingSnapshotCommitmentV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnCastingContextMembershipProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextMembershipProofV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnCastingWitnessProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingWitnessProofV1"),
    crate::captured_schema_tests::Case::bidirectional::<
        super::ParliamentTimedOvnFinalizedCastingProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnFinalizedCastingProofV1"),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ParliamentTimedOvnCastingPhaseV1>(
        "iroha_data_model::parliament_casting::ParliamentTimedOvnCastingPhaseV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnRegistrationCorpusCommitmentV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnRegistrationCorpusCommitmentV1");
    crate::captured_schema_tests::assert_bidirectional::<super::ParliamentTimedOvnReleaseBindingV1>(
        "iroha_data_model::parliament_casting::ParliamentTimedOvnReleaseBindingV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnCastingContextBindingV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextBindingV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnCastingSnapshotCommitmentV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingSnapshotCommitmentV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnCastingContextMembershipProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingContextMembershipProofV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnCastingWitnessProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnCastingWitnessProofV1");
    crate::captured_schema_tests::assert_bidirectional::<
        super::ParliamentTimedOvnFinalizedCastingProofV1,
    >("iroha_data_model::parliament_casting::ParliamentTimedOvnFinalizedCastingProofV1");
}

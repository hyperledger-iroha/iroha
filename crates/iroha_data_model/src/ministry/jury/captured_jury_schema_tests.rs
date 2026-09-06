//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryVoteChoice>(
        "iroha_data_model::ministry::jury::PolicyJuryVoteChoice",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryZkEnvelope>(
        "iroha_data_model::ministry::jury::PolicyJuryZkEnvelope",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryBallotMode>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotMode",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryBallotCommitV1>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotCommitV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryBallotRevealV1>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotRevealV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJurySortitionV1>(
        "iroha_data_model::ministry::jury::PolicyJurySortitionV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryAssignment>(
        "iroha_data_model::ministry::jury::PolicyJuryAssignment",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryFailoverPlan>(
        "iroha_data_model::ministry::jury::PolicyJuryFailoverPlan",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PolicyJuryWaitlistEntry>(
        "iroha_data_model::ministry::jury::PolicyJuryWaitlistEntry",
    );
}

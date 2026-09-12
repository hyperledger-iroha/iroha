//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryVoteChoice>(
        "iroha_data_model::ministry::jury::PolicyJuryVoteChoice",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryZkEnvelope>(
        "iroha_data_model::ministry::jury::PolicyJuryZkEnvelope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryBallotMode>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotMode",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryBallotCommitV1>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotCommitV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryBallotRevealV1>(
        "iroha_data_model::ministry::jury::PolicyJuryBallotRevealV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJurySortitionV1>(
        "iroha_data_model::ministry::jury::PolicyJurySortitionV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryAssignment>(
        "iroha_data_model::ministry::jury::PolicyJuryAssignment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryFailoverPlan>(
        "iroha_data_model::ministry::jury::PolicyJuryFailoverPlan",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PolicyJuryWaitlistEntry>(
        "iroha_data_model::ministry::jury::PolicyJuryWaitlistEntry",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

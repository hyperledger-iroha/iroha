//! Sealed candidate opening fixtures for the Parliament service tests.

use super::{
    AnonymousParliamentAttemptStateV1, AssignmentId, ParliamentBallotBoardRecordV1,
    ParliamentCredentialRecordV1, derived_due_index,
};
use crate::state::World;

impl World {
    /// Install the public candidate opening records into a test World for service-level HTTP checks.
    /// The caller selects the candidate addition and supplies the independently produced reducer rows.
    ///
    /// # Panics
    /// Panics if the supplied attempt and board disagree, their validation fails, or the
    /// committed fixture parameters cannot be installed in the test World.
    pub fn install_parliament_opening_for_testing(
        &self,
        attempt: AnonymousParliamentAttemptStateV1,
        board: ParliamentBallotBoardRecordV1,
        registrations: Vec<(AssignmentId, ParliamentCredentialRecordV1)>,
    ) {
        use iroha_data_model::{
            governance::anonymous_ballot::ParliamentCandidateTestingPolicyV1, parameter::Parameter,
        };
        let id = attempt.attempt().id;
        assert_eq!(id, board.context.body_context.governance_attempt_id);
        attempt
            .validate(&board.context.body_context.network_id)
            .unwrap();
        let checked = iroha_parliament_ballot::board::BallotBoardV1::restore(board.clone())
            .expect("valid sealed candidate board");
        let mut block = self.block();
        block
            .set_parameter_for_testing(Parameter::Custom(
                ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                    .to_custom_parameter()
                    .unwrap(),
            ))
            .unwrap();
        block.parliament_anonymous_attempts.insert(id, attempt);
        block
            .parliament_anonymous_boards
            .insert(*checked.scope(), board);
        for (assignment, record) in registrations {
            block
                .parliament_anonymous_registrations
                .insert(assignment, record);
        }
        for (id, scopes) in derived_due_index(&block) {
            block.parliament_anonymous_due.insert(id, scopes);
        }
        block.commit();
    }
}

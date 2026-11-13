//! The reducer and its authoritative ballot board advance together in the positioned pass.
//!
//! The caller supplies the original board and the shared pass's anchor. These adapters
//! bind the board to the attempt, derive opening time from Reflection, and apply closure
//! to the board and reducer atomically. They confer no runtime activation authority.

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousBodyResultV1, BallotClosureRecordV1, CastingClockEstablishedV1,
            CastingOpenedV1, ParliamentBallotBoardRecordV1,
        },
        types::{DeliberationPhaseV1, ParliamentBody},
    },
};
use iroha_parliament_ballot::board::BallotBoardV1;

use super::{
    AnonymousAttemptRejectionV1, AnonymousBallotDecisionV1, AnonymousParliamentAttemptStateV1,
    AnonymousParliamentErrorV1 as Error,
};

impl AnonymousParliamentAttemptStateV1 {
    /// Bind the typed board to the sealed body, purpose, proposal and frozen policy.
    fn check_board_identity(
        &self,
        board: &BallotBoardV1,
        network_id: &NetworkId,
    ) -> Result<ParliamentBody, Error> {
        let context = board.context().context();
        let body = context.body_context.body_role;
        let state = self
            .bodies
            .get(&body)
            .ok_or(Error::BodyNotSealed { body })?;
        let binding = state.roster_binding.ok_or(Error::RosterBinding)?;
        if context.body_context.network_id != *network_id
            || context.body_context.governance_attempt_id != self.attempt.id
            || context.body_context.election_attempt_id != state.sealed.election_attempt_id
            || board.context().body_instance_id() != &state.sealed.body_instance_id
            || board.roster_root() != &state.sealed.roster_root
            || board.purpose() != &self.purpose
            || board.proposal_content_id() != self.attempt.proposal_content_id
            || context.proposal_digest != self.proposal_digest
            || context.list_digest != binding.list_digest
            || context.seats != binding.seats
            || context.profile_id != binding.profile_id
            || board.sealed_timestamp_ms() != state.sealed_at.timestamp_ms
            || *board.context().policy()
                != self.policy.ballot_policy(self.attempt.risk_tier, body)?
        {
            return Err(Error::Restore {
                field: "board identity",
            });
        }
        Ok(body)
    }

    /// Fix a sealed board's opening time from this jury's actual Reflection entry.
    /// The pass calls this in the same item that enters Reflection, before G2c is listed.
    ///
    /// # Errors
    /// A mismatched board, inactive attempt, wrong stage, absent Reflection, or the board's
    /// scheduling error. A refusal changes neither the attempt nor the board.
    pub fn schedule_ballot_board(
        &self,
        board: &mut BallotBoardV1,
        network_id: &NetworkId,
    ) -> Result<u64, Error> {
        self.ensure_active(self.attempt.id)?;
        let body = self.check_board_identity(board, network_id)?;
        self.sealed_at_stage(body)?;
        let reflection = self.bodies[&body]
            .entered(DeliberationPhaseV1::Reflection)
            .ok_or(Error::BallotChronology)?;
        board
            .schedule_opening(reflection.timestamp_ms)
            .map_err(|error| Error::Board(Box::new(error)))
    }

    /// Open this jury's original board at the shared G2c position.
    /// The shared pass enforces its global concurrency bound before calling this method.
    ///
    /// # Errors
    /// A mismatched board, wrong stage or opening time, an anchor at/before Reflection, or
    /// the board's opening error. A refusal changes nothing.
    pub fn open_ballot_board(
        &self,
        board: &mut BallotBoardV1,
        network_id: &NetworkId,
        at: GovernanceAnchorV1,
    ) -> Result<CastingOpenedV1, Error> {
        self.ensure_active(self.attempt.id)?;
        let body = self.check_board_identity(board, network_id)?;
        self.sealed_at_stage(body)?;
        self.check_anchor(at)?;
        if board.opening_at_ms() != self.opening_at_ms(body) {
            return Err(Error::BallotChronology);
        }
        self.check_board_opening(body, at)?;
        board
            .open(at)
            .map_err(|error| Error::Board(Box::new(error)))
    }

    /// Establish the mandatory unpositioned clock at `h_open + 1`.
    ///
    /// # Errors
    /// A mismatched board, inconsistent opening, or the board's clock error. This does not
    /// consume a position or work units, and a refusal leaves the board unchanged.
    pub fn establish_ballot_board_clock(
        &self,
        board: &mut BallotBoardV1,
        network_id: &NetworkId,
        height: u64,
        timestamp_ms: u64,
    ) -> Result<CastingClockEstablishedV1, Error> {
        let body = self.check_board_identity(board, network_id)?;
        let opening = board.opening().copied().ok_or(Error::BallotChronology)?;
        if board.opening_at_ms() != self.opening_at_ms(body) {
            return Err(Error::BallotChronology);
        }
        self.check_board_opening(body, opening)?;
        board
            .establish_casting_clock(height, timestamp_ms)
            .map_err(|error| Error::Board(Box::new(error)))
    }

    /// Close the board and apply its actual corpus outcome to the reducer atomically.
    /// Confirmation registration is part of the same transition. `eligible` is evaluated
    /// only when a narrow Policy approval needs a fresh Confirmation snapshot.
    ///
    /// # Errors
    /// A mismatched board, a closure that is not due, or a reducer refusal. Both original
    /// values remain unchanged on every error, including local verifier failures.
    pub fn close_ballot_board<F>(
        &mut self,
        board: &mut BallotBoardV1,
        network_id: &NetworkId,
        at: GovernanceAnchorV1,
        eligible: F,
    ) -> Result<(AnonymousBallotDecisionV1, BallotClosureRecordV1), Error>
    where
        F: FnOnce() -> Vec<AccountId>,
    {
        let body = self.check_board_identity(board, network_id)?;
        if board.opening_at_ms() != self.opening_at_ms(body) {
            return Err(Error::BallotChronology);
        }
        let mut next_board = board.clone();
        let closure = *next_board
            .close(at)
            .map_err(|error| Error::Board(Box::new(error)))?;
        let mut next_attempt = self.clone();
        let decision = next_attempt.close_ballot(self.attempt.id, closure, network_id, eligible)?;
        *self = next_attempt;
        *board = next_board;
        Ok((decision, closure))
    }

    fn check_board_opening(
        &self,
        body: ParliamentBody,
        opening: GovernanceAnchorV1,
    ) -> Result<(), Error> {
        self.check_anchor(opening)?;
        let reflection = self
            .bodies
            .get(&body)
            .and_then(|state| state.entered(DeliberationPhaseV1::Reflection))
            .ok_or(Error::BallotChronology)?;
        // The opening takes a later pass position than the Reflection entry. In the same
        // block both carry that block's time, and a later block is never earlier.
        let position_follows =
            (opening.height, opening.position) > (reflection.height, reflection.position);
        let time_follows = if opening.height == reflection.height {
            opening.timestamp_ms == reflection.timestamp_ms
        } else {
            opening.timestamp_ms >= reflection.timestamp_ms
        };
        let due = self
            .opening_at_ms(body)
            .is_some_and(|due_ms| opening.timestamp_ms >= due_ms);
        if position_follows && time_follows && due {
            Ok(())
        } else {
            Err(Error::BallotChronology)
        }
    }

    /// Cross-check a persisted board's lifecycle against the reducer. Checking the board
    /// alone cannot establish that Reflection happened or its closure was applied.
    pub(super) fn validate_board_lifecycle(
        &self,
        board: &ParliamentBallotBoardRecordV1,
    ) -> Result<(), Error> {
        let body = board.context.body_context.body_role;
        if board.opening_at_ms != self.opening_at_ms(body) {
            return Err(Error::Restore {
                field: "board opening schedule",
            });
        }
        if let Some(opening) = board.opening {
            self.check_board_opening(body, opening)?;
        }
        let result_closure = self
            .results
            .get(&body)
            .and_then(|binding| match &binding.result {
                AnonymousBodyResultV1::Ballot(binding) => Some(&binding.closure),
                AnonymousBodyResultV1::PublicFinding(_) => None,
            });
        let rejection_closure = match &self.rejection {
            Some(
                AnonymousAttemptRejectionV1::Ballot(closure)
                | AnonymousAttemptRejectionV1::ConfirmationCapacityUnavailable(closure),
            ) if closure.body_role == body => Some(closure.as_ref()),
            _ => None,
        };
        if board.closure.as_ref() != result_closure.or(rejection_closure) {
            return Err(Error::Restore {
                field: "board closure outcome",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ballot_lifecycle_tests.rs"]
mod tests;

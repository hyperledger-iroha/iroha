//! The anonymous Parliament tables and their restore check (design §12.3, slice 2).
//!
//! World keeps three tables for the anonymous family, added by the
//! `iroha.governance.parliament.anonymous.v1` release addition:
//!
//! - `world.parliament_anonymous_attempts`: the State of each attempt;
//! - `world.parliament_anonymous_registrations`: the credential of each accepted binding
//!   assignment, keyed by the assignment, whose identifier binds its election and member;
//! - `world.parliament_anonymous_boards`: the board of each sealed binding jury, keyed by its
//!   ballot scope.
//!
//! Until a change set activates the addition the tables stay empty and out of snapshots. A
//! restored cut is accepted only if every attempt is one the reducer produces, every election
//! holds exactly the credentials of its accepted binding assignments and they rebuild its
//! ledger, every credential names a registration the cut's history could have executed, every
//! sealed binding jury has exactly the board its attempt and credentials give, and no row
//! belongs to no attempt.
//!
//! Realized lifecycle anchors and ballot corpus entries cannot lie above the restored cut.
//! Future sortition slots and lifecycle deadlines remain pending work, not realized facts.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        anonymous_ballot::{
            BallotScopeV1, ParliamentBallotBoardRecordV1, ParliamentCredentialRecordV1,
        },
        types::{AssignmentId, GovernanceAttemptId},
    },
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;

use super::{CitizenshipRecord, WorldReadOnly};
use crate::governance::parliament_anonymous::{
    AnonymousElectionCredentialsV1, AnonymousParliamentAttemptStateV1, AnonymousParliamentErrorV1,
};

/// The World members of the three tables, in registry order.
const MEMBERS: [&str; 3] = [
    "parliament_anonymous_attempts",
    "parliament_anonymous_registrations",
    "parliament_anonymous_boards",
];

/// The credentials World keeps for the accepted assignments `accepted` of one election, in
/// assignment order. An accepted assignment without a stored credential is left out, and the
/// reducer refuses the incomplete set.
pub(crate) fn election_credentials(
    registrations: &impl StorageReadOnly<AssignmentId, ParliamentCredentialRecordV1>,
    accepted: &BTreeSet<AssignmentId>,
) -> AnonymousElectionCredentialsV1 {
    accepted
        .iter()
        .filter_map(|assignment| {
            registrations
                .get(assignment)
                .map(|record| (*assignment, *record))
        })
        .collect()
}

/// The citizens whose bond is at least `minimum_bond`, in account order: the candidate snapshot
/// of an anonymous attempt's sortition registrations and of its Confirmation generation
/// (design §8.1). Enrollment and restore keep the registry within the V1 citizen cap, and the
/// reducer checks the snapshot's bounds.
pub(crate) fn eligible_citizens(
    citizens: &impl StorageReadOnly<AccountId, CitizenshipRecord>,
    minimum_bond: &Quantity,
) -> Vec<AccountId> {
    let mut eligible: Vec<AccountId> = citizens
        .iter()
        .filter(|(_, record)| record.amount >= *minimum_bond)
        .map(|(account, _)| account.clone())
        .collect();
    eligible.sort_unstable();
    eligible
}

/// The anonymous attempts that are not terminal. Each holds one casting position until it ends,
/// so their number is what the admission cap bounds (design §9).
pub(crate) fn nonterminal_attempts(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, AnonymousParliamentAttemptStateV1>,
) -> usize {
    attempts
        .iter()
        .filter(|(_, attempt)| !attempt.is_terminal())
        .count()
}

/// Why the anonymous Parliament tables of a restored cut are refused.
#[derive(Debug)]
pub(crate) enum AnonymousTablesRestoreError {
    /// The tables hold an entry the reducer does not produce.
    Invalid(String),
    /// The node could not rebuild a seal or board: a local failure, never a verdict.
    Local(AnonymousParliamentErrorV1),
}

/// Check the anonymous Parliament tables of `cut`, a cut at committed `height` on the network
/// `network_id`.
///
/// The checks that need no proof material run first, so a node that cannot rebuild a seal or
/// board still refuses a cut whose rows belong to no attempt.
///
/// # Errors
/// [`AnonymousTablesRestoreError::Invalid`] for the first entry the reducer does not produce,
/// and [`AnonymousTablesRestoreError::Local`] when the node cannot rebuild a seal or board.
pub(crate) fn validate_cut(
    cut: &impl WorldReadOnly,
    network_id: &NetworkId,
    height: u64,
) -> Result<(), AnonymousTablesRestoreError> {
    use AnonymousTablesRestoreError::Invalid;

    let attempts = cut.parliament_anonymous_attempts();
    let registrations = cut.parliament_anonymous_registrations();
    let stored_boards = cut.parliament_anonymous_boards();

    // A selection that does not install the family holds none of its rows.
    let uninstalled = crate::runtime::uninstalled_world_members(cut.runtime_selection())
        .map_err(Invalid)?
        .members;
    for (member, empty) in MEMBERS.into_iter().zip([
        attempts.is_empty(),
        registrations.is_empty(),
        stored_boards.is_empty(),
    ]) {
        if !empty && uninstalled.contains(&member) {
            return Err(Invalid(format!(
                "world.{member} holds rows its root selection does not install"
            )));
        }
    }

    let mut accepted = BTreeSet::new();
    for (id, attempt) in attempts.iter() {
        if attempt.attempt().id != *id {
            return Err(Invalid(format!(
                "anonymous Parliament attempt {id:?} is stored under another identifier"
            )));
        }
        attempt
            .validate_restored_height_v1(height)
            .map_err(|error| Invalid(format!("anonymous Parliament attempt {id:?}: {error}")))?;
        accepted.extend(
            attempt
                .generations()
                .iter()
                .flat_map(|generation| generation.elections())
                .flat_map(|election| election.accepted().iter().copied()),
        );
    }
    if let Some((assignment, _)) = registrations
        .iter()
        .find(|(assignment, _)| !accepted.contains(*assignment))
    {
        return Err(Invalid(format!(
            "a credential names the assignment {assignment:?}, which no anonymous Parliament attempt accepted"
        )));
    }
    type StoredBoard<'a> = (&'a BallotScopeV1, &'a ParliamentBallotBoardRecordV1);
    let mut boards: BTreeMap<GovernanceAttemptId, Vec<StoredBoard<'_>>> = BTreeMap::new();
    for (scope, board) in stored_boards.iter() {
        if board.opening.is_some_and(|anchor| anchor.height > height)
            || board
                .casting_clock
                .is_some_and(|clock| clock.height > height)
            || board.entries.iter().any(|entry| entry.height > height)
            || board.closure.as_ref().is_some_and(|closure| {
                closure.opening_anchor.height > height
                    || closure.casting_clock_height > height
                    || closure.closure_anchor.height > height
            })
        {
            return Err(Invalid(format!(
                "anonymous Parliament board {scope:?} records a block above restored height {height}"
            )));
        }
        if board.opening.is_some_and(|opening| opening.height < height)
            && board.casting_clock.is_none()
        {
            return Err(Invalid(format!(
                "anonymous Parliament board {scope:?} is missing its mandatory casting clock at restored height {height}"
            )));
        }
        let attempt = board.context.body_context.governance_attempt_id;
        if attempts.get(&attempt).is_none() {
            return Err(Invalid(format!(
                "a ballot board names the unknown anonymous Parliament attempt {attempt:?}"
            )));
        }
        boards.entry(attempt).or_default().push((scope, board));
    }

    for (id, attempt) in attempts.iter() {
        let refuse = |error: AnonymousParliamentErrorV1| {
            if error.is_local() {
                AnonymousTablesRestoreError::Local(error)
            } else {
                Invalid(format!("anonymous Parliament attempt {id:?}: {error}"))
            }
        };
        attempt.validate(network_id).map_err(refuse)?;
        attempt
            .validate_credentials(
                network_id,
                height,
                |_, accepted| election_credentials(registrations, accepted),
                boards.remove(id).unwrap_or_default(),
            )
            .map_err(refuse)?;
    }
    Ok(())
}

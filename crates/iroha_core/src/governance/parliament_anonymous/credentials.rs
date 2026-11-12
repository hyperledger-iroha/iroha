//! Credential registration (design §3.2) and the credential-aware seal (§3.3) of an
//! anonymous attempt.
//!
//! A binding seat is accepted only together with its verified credential. The adapter
//! derives the authority's assignment from the attempt, rebuilds the election's
//! registration ledger from the compact records World keeps for it, which must be exactly
//! one per accepted assignment, verifies the registration against the authoritative body
//! context, and records the acceptance and the credential in one step. The full proof stays
//! in the signed input the record's source names; World keeps only `(C, auth)` and that
//! locator.
//!
//! A binding jury seals exactly its seated credentials. Before `CloseInvitations` changes
//! anything, the seal step asks the ledger for the seal of the seated assignments and builds
//! the board from facts of committed State; the seal's `(R, S, profile_id)` then enters the
//! 64-byte roster root. The planner trial-applies due items with the same step, so a plan
//! agrees with what execution records.
//!
//! A refusal the node cannot decide ([`AnonymousParliamentErrorV1::is_local`]) is never a
//! verdict: a registration is not rejected for it, and a plan fails instead of recording it.

use std::collections::{BTreeMap, BTreeSet};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            BallotBodyContextV1, BallotScopeV1, Digest512, ParliamentBallotBoardRecordV1,
            ParliamentCredentialRecordV1, ParliamentCredentialSourceV1,
            RegisterParliamentCredentialV1, SealedCredentialListPublishedV1,
            anonymous_body_instance_id_v1,
        },
        types::{AssignmentId, BodyElectionAttemptId, BodyInstanceId, ParliamentBody},
    },
};
use iroha_parliament_ballot::board::{
    BallotBoardV1, BoardSealFactsV1, RegisteredCredentialV1, RegistrationLedgerErrorV1,
    RegistrationLedgerV1, SealedRosterV1,
};

use super::{
    AnonymousGenerationStatusV1, AnonymousInvitationCloseV1, AnonymousLifecycleItemV1,
    AnonymousParliamentAttemptStateV1, AnonymousParliamentErrorV1 as Error, is_binding,
};

/// The compact credential records of one binding election, as World keeps them.
pub type AnonymousElectionCredentialsV1 = Vec<(AssignmentId, ParliamentCredentialRecordV1)>;

/// A sealed binding jury's board and its seal output: the complete canonical list with the
/// body instance and the roster root it is bound into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousSealedBoardV1 {
    /// The board World keeps from seal to closure.
    pub board: ParliamentBallotBoardRecordV1,
    /// The seal's block-start output.
    pub published: SealedCredentialListPublishedV1,
}

/// What an applied lifecycle item did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnonymousItemEffectV1 {
    /// What a `CloseInvitations` item decided; `None` for every other item.
    pub close: Option<AnonymousInvitationCloseV1>,
    /// The board and seal output of the binding jury the item sealed.
    pub board: Option<Box<AnonymousSealedBoardV1>>,
}

/// What a planned item does when the pass applies it in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnonymousItemOutcomeV1 {
    /// The item applies.
    Applied(AnonymousItemEffectV1),
    /// The item lapses, uncharged: an earlier item of the same pass ended its occurrence or
    /// its attempt.
    Lapsed(Box<Error>),
    /// The item is refused although it is due: the attempt and its stored credentials
    /// disagree. State that restore accepted never produces it.
    Refused(Box<Error>),
}

/// One due item of a plan and what applying it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnonymousPlannedItemV1 {
    /// The item.
    pub item: AnonymousLifecycleItemV1,
    /// What it does when the pass applies it in this order.
    pub outcome: AnonymousItemOutcomeV1,
}

const fn restore(field: &'static str) -> Error {
    Error::Restore { field }
}

/// The registration ledger of `context`, rebuilt from the records World keeps for its
/// election: exactly one per assignment in `accepted`, each seat and commitment once. The
/// proofs were verified when the records were committed.
fn election_ledger(
    context: BallotBodyContextV1,
    accepted: &BTreeSet<AssignmentId>,
    records: &[(AssignmentId, ParliamentCredentialRecordV1)],
) -> Result<RegistrationLedgerV1, Error> {
    let assignments: BTreeSet<AssignmentId> =
        records.iter().map(|(assignment, _)| *assignment).collect();
    if assignments.len() != records.len() || assignments != *accepted {
        return Err(Error::CredentialStore);
    }
    RegistrationLedgerV1::restore(
        context,
        records.iter().map(|(assignment, record)| {
            (
                *assignment,
                RegisteredCredentialV1 {
                    commitment: record.commitment,
                    authorization: record.authorization,
                },
            )
        }),
    )
    .map_err(|error| match error {
        // A repeated seat or commitment is World's corruption, not a registrant's fault.
        RegistrationLedgerErrorV1::SeatRegistered
        | RegistrationLedgerErrorV1::DuplicateCommitment => Error::CredentialStore,
        other => Error::Credential(Box::new(other)),
    })
}

impl AnonymousParliamentAttemptStateV1 {
    /// The body of the active election `election_attempt_id`.
    fn active_election_body(
        &self,
        election_attempt_id: BodyElectionAttemptId,
    ) -> Result<ParliamentBody, Error> {
        self.policy
            .required_bodies
            .iter()
            .map(|required| required.body)
            .find(|body| {
                self.active_election(*body)
                    .is_some_and(|(generation, election)| {
                        self.generations[generation].elections()[election].id()
                            == election_attempt_id
                    })
            })
            .ok_or(Error::UnknownElection)
    }

    /// The accepted assignments of the election `election_attempt_id`, primaries and
    /// alternates.
    ///
    /// # Errors
    /// [`Error::UnknownElection`] for an election that is not this attempt's.
    pub fn accepted_assignments(
        &self,
        election_attempt_id: BodyElectionAttemptId,
    ) -> Result<&BTreeSet<AssignmentId>, Error> {
        self.generations
            .iter()
            .flat_map(|generation| &generation.elections)
            .find(|election| election.id() == election_attempt_id)
            .map(|election| &election.accepted)
            .ok_or(Error::UnknownElection)
    }

    /// The authoritative context of the binding body of the active election
    /// `election_attempt_id`.
    ///
    /// # Errors
    /// [`Error::UnknownElection`] for an election that is not a body's active one, and
    /// [`Error::DecisionMode`] for a public body.
    pub fn binding_body_context(
        &self,
        network_id: &NetworkId,
        election_attempt_id: BodyElectionAttemptId,
    ) -> Result<BallotBodyContextV1, Error> {
        let body = self.active_election_body(election_attempt_id)?;
        if !is_binding(body) {
            return Err(Error::DecisionMode { body });
        }
        Ok(BallotBodyContextV1 {
            network_id: *network_id,
            governance_attempt_id: self.attempt.id,
            election_attempt_id,
            body_role: body,
        })
    }

    /// Accept the authority's binding seat with its credential (design §3.2), in a block of
    /// time `now_ms`, executing the signed input `source` names.
    ///
    /// The assignment is the authority's invitation in the open election; `records` are
    /// the credentials World keeps for that election, exactly one per accepted assignment.
    /// The cheap invitation and uniqueness checks run before the proof is verified, and
    /// nothing changes unless every check passes. The returned record is the credential
    /// World keeps for the assignment.
    ///
    /// # Errors
    /// [`Error`] for another or an inactive attempt, an election that is not a binding
    /// body's open one, a closed window, no invitation or a response already recorded, a
    /// source at height zero, [`Error::CredentialStore`] for records that are not the
    /// election's, and [`Error::Credential`] for a commitment already registered or a
    /// registration that does not verify or that the node could not verify
    /// ([`Error::is_local`]).
    pub fn register_credential(
        &mut self,
        network_id: &NetworkId,
        authority: &AccountId,
        input: &RegisterParliamentCredentialV1,
        records: &[(AssignmentId, ParliamentCredentialRecordV1)],
        source: ParliamentCredentialSourceV1,
        now_ms: u64,
    ) -> Result<(AssignmentId, ParliamentCredentialRecordV1), Error> {
        self.ensure_active(input.governance_attempt_id)?;
        let context = self.binding_body_context(network_id, input.election_attempt_id)?;
        let assignment = self.invitation(
            input.governance_attempt_id,
            input.election_attempt_id,
            context.body_role,
            authority,
            now_ms,
        )?;
        if source.height == 0 {
            return Err(Error::CredentialSource);
        }
        let registered = election_ledger(
            context,
            self.accepted_assignments(input.election_attempt_id)?,
            records,
        )?
        .register(authority, assignment, &input.registration)
        .map_err(|error| Error::Credential(Box::new(error)))?;
        let accepted = self.accept_binding_seat(
            input.governance_attempt_id,
            input.election_attempt_id,
            context.body_role,
            authority,
            now_ms,
        )?;
        Ok((
            accepted,
            ParliamentCredentialRecordV1 {
                commitment: registered.commitment,
                authorization: registered.authorization,
                source,
            },
        ))
    }

    /// The board a binding jury sealed with `roster` starts from: facts of committed State
    /// alone, and its seal output.
    fn board_of(
        &self,
        body: ParliamentBody,
        roster: &SealedRosterV1,
        body_instance_id: BodyInstanceId,
        roster_root: Digest512,
        sealed_timestamp_ms: u64,
    ) -> Result<AnonymousSealedBoardV1, Error> {
        let policy = self.policy.ballot_policy(self.attempt.risk_tier, body)?;
        let board = BallotBoardV1::sealed(
            roster,
            BoardSealFactsV1 {
                purpose: self.purpose,
                proposal_digest: self.proposal_digest,
                proposal_content_id: self.attempt.proposal_content_id,
                body_instance_id,
                roster_root,
                sealed_timestamp_ms,
                policy,
            },
        )
        .map_err(|error| Error::Board(Box::new(error)))?;
        let published = roster
            .published(body_instance_id, roster_root)
            .map_err(|error| Error::Seal(Box::new(error)))?;
        Ok(AnonymousSealedBoardV1 {
            board: board.record(),
            published,
        })
    }

    /// The seal of `item`, the `CloseInvitations` of the binding jury `body`, at `at`, with
    /// the board it starts, computed before the item changes anything; `None` when the close
    /// seals no roster. The occurrence and anchor are checked first, so a passed item
    /// lapses as it would without credentials, and `credentials` is asked only for a seal.
    fn prepare_binding_seal<F>(
        &self,
        network_id: &NetworkId,
        item: AnonymousLifecycleItemV1,
        body: ParliamentBody,
        at: GovernanceAnchorV1,
        credentials: &F,
    ) -> Result<Option<(SealedRosterV1, AnonymousSealedBoardV1)>, Error>
    where
        F: Fn(BodyElectionAttemptId, &BTreeSet<AssignmentId>) -> AnonymousElectionCredentialsV1,
    {
        self.check_occurrence(item)?;
        self.check_anchor(at)?;
        let seating = self.seating(body, at.timestamp_ms)?;
        if seating.no_roster.is_some() {
            return Ok(None);
        }
        let context = BallotBodyContextV1 {
            network_id: *network_id,
            governance_attempt_id: self.attempt.id,
            election_attempt_id: seating.election_attempt_id,
            body_role: body,
        };
        let seated: Vec<AssignmentId> = seating
            .seats
            .iter()
            .map(|seat| seat.assignment_id)
            .collect();
        let accepted = self.accepted_assignments(seating.election_attempt_id)?;
        let roster = election_ledger(
            context,
            accepted,
            &credentials(seating.election_attempt_id, accepted),
        )?
        .seal(&seated)
        .map_err(|error| Error::Seal(Box::new(error)))?;
        // The same preimage the reducer roots the seal in: the election, the seated
        // assignments in canonical order and the seal's binding.
        let roster_root = roster
            .roster_root(&seating.seats)
            .map_err(|error| Error::Seal(Box::new(error)))?;
        let body_instance_id =
            anonymous_body_instance_id_v1(seating.election_attempt_id, &roster_root);
        let board = self.board_of(
            body,
            &roster,
            body_instance_id,
            roster_root,
            at.timestamp_ms,
        )?;
        Ok(Some((roster, board)))
    }

    /// Apply the lifecycle `item` at `at`, sealing a binding jury from its credentials:
    /// `credentials` returns the records World keeps for the accepted assignments of an
    /// election and is asked only for a binding jury's seal. The seal and its board are built
    /// before the item changes anything, so a refused item changes nothing.
    ///
    /// # Errors
    /// As [`Self::apply_lifecycle_item`], and the error of the seal or the board.
    pub fn apply_item_with_credentials<F>(
        &mut self,
        network_id: &NetworkId,
        item: AnonymousLifecycleItemV1,
        at: GovernanceAnchorV1,
        credentials: F,
    ) -> Result<AnonymousItemEffectV1, Error>
    where
        F: Fn(BodyElectionAttemptId, &BTreeSet<AssignmentId>) -> AnonymousElectionCredentialsV1,
    {
        let prepared = match item {
            AnonymousLifecycleItemV1::CloseInvitations { body, .. } if is_binding(body) => {
                self.prepare_binding_seal(network_id, item, body, at, &credentials)?
            }
            _ => None,
        };
        let binding = prepared.as_ref().map(|(roster, _)| roster.binding());
        let close = self.apply_lifecycle_item(item, at, |_| binding.ok_or(Error::RosterBinding))?;
        let board = prepared
            .filter(|_| close == Some(AnonymousInvitationCloseV1::Sealed))
            .map(|(_, board)| Box::new(board));
        Ok(AnonymousItemEffectV1 { close, board })
    }

    /// The due items of this attempt in a block of time `timestamp_ms` at `height`, each
    /// trial-applied in order to a copy with the seal step the pass applies. Plan on the
    /// State after the pass's draws: the attempt's first item is anchored at
    /// `first_position`, and each applied item takes the next position.
    ///
    /// A plan is advice: the pass applies the same items with the same step, so a planned
    /// seal, `NoRoster`, lapse or refusal is what execution records for the same
    /// credentials.
    ///
    /// # Errors
    /// [`Error::Schedule`] if a deadline of the attempt cannot be expressed, and a local
    /// error ([`Error::is_local`]) the node met: a node that cannot decide an item does not
    /// plan it.
    pub fn plan_due_items<F>(
        &self,
        network_id: &NetworkId,
        height: u64,
        timestamp_ms: u64,
        first_position: u32,
        credentials: F,
    ) -> Result<Vec<AnonymousPlannedItemV1>, Error>
    where
        F: Fn(BodyElectionAttemptId, &BTreeSet<AssignmentId>) -> AnonymousElectionCredentialsV1,
    {
        let mut state = self.clone();
        let mut position = first_position;
        let mut plan = Vec::new();
        for due in self.due_lifecycle_items(timestamp_ms)? {
            let at = GovernanceAnchorV1 {
                height,
                position,
                timestamp_ms,
            };
            let outcome =
                match state.apply_item_with_credentials(network_id, due.item, at, &credentials) {
                    Ok(effect) => {
                        position = position.checked_add(1).ok_or(Error::Schedule)?;
                        AnonymousItemOutcomeV1::Applied(effect)
                    }
                    Err(error) if error.is_local() => return Err(error),
                    Err(error @ (Error::StaleItem | Error::NotActive { .. })) => {
                        AnonymousItemOutcomeV1::Lapsed(Box::new(error))
                    }
                    Err(error) => AnonymousItemOutcomeV1::Refused(Box::new(error)),
                };
            plan.push(AnonymousPlannedItemV1 {
                item: due.item,
                outcome,
            });
        }
        Ok(plan)
    }

    /// Check the credentials and boards World keeps for this attempt (design §12.3,
    /// slice 2) in a cut at committed `height`, where `credentials` returns the records World
    /// keeps for the accepted assignments of an election: every election of a binding jury
    /// holds exactly one credential per accepted assignment and the credentials rebuild its
    /// ledger, each credential names the sole instruction of an input executed from the
    /// election's draw until its close and no later than `height`, an election of a public
    /// body holds none, every sealed binding jury's seal is the one its credentials give,
    /// and `boards`, each with the scope it is stored under, are exactly one board per
    /// sealed binding jury, stored under its own scope, whose seal-time facts are the ones
    /// this State and its seal give.
    ///
    /// # Errors
    /// [`Error::CredentialStore`] for credentials that are not their election's,
    /// [`Error::Restore`] for a seal or board this State and its credentials do not give,
    /// and a local error ([`Error::is_local`]) when the node cannot rebuild a seal or board.
    pub fn validate_credentials<'b, F, B>(
        &self,
        network_id: &NetworkId,
        height: u64,
        credentials: F,
        boards: B,
    ) -> Result<(), Error>
    where
        F: Fn(BodyElectionAttemptId, &BTreeSet<AssignmentId>) -> AnonymousElectionCredentialsV1,
        B: IntoIterator<Item = (&'b BallotScopeV1, &'b ParliamentBallotBoardRecordV1)>,
    {
        for generation in &self.generations {
            let drawn_at = match &generation.status {
                AnonymousGenerationStatusV1::Drawn(draw) => Some(draw.at.height),
                _ => None,
            };
            for election in &generation.elections {
                let records = credentials(election.id(), &election.accepted);
                if !is_binding(election.body()) {
                    if !records.is_empty() {
                        return Err(Error::CredentialStore);
                    }
                    continue;
                }
                // A registration executes after the draw's block-start item and before the
                // close's, as the sole instruction of its input.
                let closed_at = election.closed.as_ref().map(|closed| closed.at.height);
                if records.iter().any(|(_, record)| {
                    let source = record.source;
                    source.instruction_index != 0
                        || source.height > height
                        || drawn_at.is_none_or(|drawn| source.height < drawn)
                        || closed_at.is_some_and(|closed| source.height >= closed)
                }) {
                    return Err(Error::CredentialStore);
                }
                let context = BallotBodyContextV1 {
                    network_id: *network_id,
                    governance_attempt_id: self.attempt.id,
                    election_attempt_id: election.id(),
                    body_role: election.body(),
                };
                election_ledger(context, &election.accepted, &records)?;
            }
        }
        let mut stored = BTreeMap::new();
        for (scope, board) in boards {
            if stored
                .insert(board.body_instance_id, (scope, board))
                .is_some()
            {
                return Err(restore("boards"));
            }
        }
        for (body, state) in &self.bodies {
            if !is_binding(*body) {
                continue;
            }
            let sealed = &state.sealed;
            let context = BallotBodyContextV1 {
                network_id: *network_id,
                governance_attempt_id: self.attempt.id,
                election_attempt_id: sealed.election_attempt_id,
                body_role: *body,
            };
            let seated: Vec<AssignmentId> =
                state.seats.iter().map(|seat| seat.assignment_id).collect();
            let accepted = self.accepted_assignments(sealed.election_attempt_id)?;
            let roster = election_ledger(
                context,
                accepted,
                &credentials(sealed.election_attempt_id, accepted),
            )?
            .seal(&seated)
            .map_err(|error| Error::Seal(Box::new(error)))?;
            if state.roster_binding != Some(roster.binding()) {
                return Err(restore("credentials"));
            }
            let expected = self
                .board_of(
                    *body,
                    &roster,
                    sealed.body_instance_id,
                    sealed.roster_root,
                    state.sealed_at.timestamp_ms,
                )?
                .board;
            let (scope, board) = stored
                .remove(&sealed.body_instance_id)
                .ok_or_else(|| restore("boards"))?;
            let restored = BallotBoardV1::restore(board.clone()).map_err(|error| {
                if error.is_local() {
                    Error::Board(Box::new(error))
                } else {
                    restore("boards")
                }
            })?;
            if restored.scope() != scope
                || board.purpose != expected.purpose
                || board.context != expected.context
                || board.proposal_content_id != expected.proposal_content_id
                || board.roster_root != expected.roster_root
                || board.registration_mapping_digest != expected.registration_mapping_digest
                || board.sealed_timestamp_ms != expected.sealed_timestamp_ms
            {
                return Err(restore("boards"));
            }
        }
        if stored.is_empty() {
            Ok(())
        } else {
            Err(restore("boards"))
        }
    }
}

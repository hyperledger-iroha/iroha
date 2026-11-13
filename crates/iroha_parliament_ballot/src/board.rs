//! The node's authoritative ballot board of the binding bodies (design §3.2–§3.8).
//!
//! This module is the one state machine a node runs for an anonymous ballot. It owns no
//! storage and no clock: the node feeds it the facts of committed State (transaction
//! authority, seat assignments, block heights and times) and persists what it returns.
//!
//! - [`RegistrationLedgerV1`]: the credentials registered at seat acceptance for one
//!   election attempt. A registration is verified with the real registration relation and
//!   recorded only when its seat and its commitment are both fresh, across primaries and
//!   alternates. The 160,148-byte proof is not retained: it stays in the signed input.
//! - [`RegistrationLedgerV1::seal`]: the sealed list `L` of exactly the seated
//!   assignments, its digest `R`, the registration mapping and the roster binding
//!   `(R, S, profile_id)` ([`SealedRosterV1`]). A roster below three seats is the
//!   objective `NoRoster`, never a seal.
//! - [`BallotBoardV1`]: one sealed body's ballot from seal to closure: the opening time
//!   fixed at Reflection, the opening anchor `s0`, the casting clock of `h_open + 1`, the
//!   accepted corpus and the closure record with its outcome.
//! - [`BoardsAtBlockV1`] and [`execute_block_ballot_v1`]: the committed-state answers of
//!   block execution ([`BlockBallotStateV1`]) and the one rollback-safe execution step.
//! - [`BoardsAtAdmissionV1`]: the committed-state answers of queue admission
//!   ([`AdmissionStateV1`]) at the node's local clock, with the queue's own pending index.
//!
//! Every transition checks before it changes anything, so a refused transition leaves
//! the board as it was. World keeps a board as its
//! [`ParliamentBallotBoardRecordV1`] ([`BallotBoardV1::record`]); a board restored from
//! that record ([`BallotBoardV1::restore`]) is recomputed, never trusted: the context, the
//! corpus root, the counts and the closure record are rebuilt and compared.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
    sync::Arc,
};

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AnonymousBallotCorpusEntryV1, AnonymousParliamentRosterRootPreimageV1,
            AnonymousRosterBindingV1, BallotBodyContextV1, BallotCastingClockV1,
            BallotClosureRecordV1, BallotScopeV1, CastingClockEstablishedV1, CastingOpenedV1,
            CredentialCommitmentV1, CredentialRegistrationV1, Digest512, FrozenBallotPolicyV1,
            MAX_SEALED_SEATS_V1, MIN_SEALED_SEATS_V1, NullifierV1,
            ParliamentBallotBoardRecordErrorV1, ParliamentBallotBoardRecordV1,
            ParliamentBallotPurposeErrorV1, ParliamentBallotPurposeV1, ProofProfileIdV1,
            RegistrationAuthorizationV1, SealedCredentialListBytesV1,
            SealedCredentialListPublishedV1, SealedCredentialListV1, SealedRegistrationPairV1,
            digests::anonymous_roster_root_v1,
        },
        types::{AssignmentId, BodyInstanceId, ParliamentSeatAssignmentV1, ProposalContentId},
    },
};

use crate::{
    ballot::{
        AdmissionScopeV1, AdmissionStateV1, BlockBallotDecisionV1, BlockBallotStateV1,
        CastingWindowStatusV1, CastingWindowV1, VerificationUnavailableV1, decide_block_ballot_v1,
    },
    context::{ContextErrorV1, ScopedBallotContextV1},
    corpus::{
        ClosureAnchorsV1, ClosureBuildErrorV1, CorpusAccumulatorV1, CorpusErrorV1,
        build_closure_record_v1, closing_ms_v1,
    },
    inputs::{body_digest_v1, list_digest_v1, registration_mapping_v1},
    registration::{RegistrationErrorV1, verify_registration_v1},
};

/// The compact record of one verified registration. The proof stays in the signed input
/// that carried it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisteredCredentialV1 {
    /// Credential commitment `C`.
    pub commitment: CredentialCommitmentV1,
    /// Registration authorization value `auth`, verified under `pi_reg`.
    pub authorization: RegistrationAuthorizationV1,
}

/// Why a registration is not recorded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistrationLedgerErrorV1 {
    /// The body context is not a binding body's, or `B` could not be computed.
    #[error(transparent)]
    Context(#[from] ContextErrorV1),
    /// The seat already registered a credential: one credential per seat.
    #[error("the seat already registered a credential")]
    SeatRegistered,
    /// The commitment is already registered for a seat of this election attempt.
    #[error("the commitment is already registered for a seat of this election attempt")]
    DuplicateCommitment,
    /// The registration does not verify, or the node could not verify it
    /// ([`RegistrationErrorV1::is_local`]).
    #[error(transparent)]
    Registration(#[from] RegistrationErrorV1),
}

impl RegistrationLedgerErrorV1 {
    /// Whether the node could not decide, as opposed to the registration being invalid.
    /// A local refusal is never a committed rejection.
    pub const fn is_local(&self) -> bool {
        match self {
            Self::Registration(error) => error.is_local(),
            Self::Context(error) => error.is_local(),
            Self::SeatRegistered | Self::DuplicateCommitment => false,
        }
    }
}

/// Why a roster cannot be sealed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SealErrorV1 {
    /// The seated roster is outside `3..=1000`: the objective `NoRoster` of design §3.3.
    #[error("a binding body seals 3 to 1000 seats")]
    NoRoster,
    /// A seated assignment appears twice.
    #[error("a seated assignment appears twice")]
    DuplicateAssignment,
    /// A seated assignment registered no credential. Acceptance of a binding seat is
    /// invalid without one, so this is corrupt input, not a lifecycle outcome.
    #[error("a seated assignment registered no credential")]
    MissingCredential,
    /// The list, its digest, the mapping or the roster root could not be built.
    #[error(transparent)]
    Context(#[from] ContextErrorV1),
}

impl SealErrorV1 {
    /// Whether the node could not build the seal, as opposed to the roster being one that
    /// cannot be sealed. A local failure is never a committed verdict.
    pub const fn is_local(&self) -> bool {
        match self {
            Self::Context(error) => error.is_local(),
            Self::NoRoster | Self::DuplicateAssignment | Self::MissingCredential => false,
        }
    }
}

/// The seal of one binding body: the list, the mapping and the roster binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedRosterV1 {
    body_context: BallotBodyContextV1,
    list: SealedCredentialListV1,
    list_digest: Digest512,
    pairs: Vec<SealedRegistrationPairV1>,
    registration_mapping_digest: Digest512,
}

impl SealedRosterV1 {
    /// The authoritative body context the roster was sealed for.
    pub fn body_context(&self) -> &BallotBodyContextV1 {
        &self.body_context
    }

    /// The sealed list `L`, strictly increasing.
    pub fn list(&self) -> &SealedCredentialListV1 {
        &self.list
    }

    /// The sealed-list digest `R`.
    pub fn list_digest(&self) -> &Digest512 {
        &self.list_digest
    }

    /// The sealed `(assignment_id, C)` pairs in raw assignment-identifier order.
    pub fn pairs(&self) -> &[SealedRegistrationPairV1] {
        &self.pairs
    }

    /// The registration-mapping digest of [`Self::pairs`].
    pub fn registration_mapping_digest(&self) -> &Digest512 {
        &self.registration_mapping_digest
    }

    /// The roster binding `(R, S, profile_id)` of the extended roster-root preimage.
    pub fn binding(&self) -> AnonymousRosterBindingV1 {
        AnonymousRosterBindingV1 {
            list_digest: self.list_digest,
            seats: self.list.seats,
            profile_id: self.list.profile_id,
        }
    }

    /// The 64-byte roster root of the seated `assignments` with this seal's binding.
    /// `assignments` are the seated assignments of [`Self::pairs`], in any order.
    ///
    /// # Errors
    /// [`SealErrorV1::MissingCredential`] or [`SealErrorV1::DuplicateAssignment`] if
    /// `assignments` are not exactly the sealed assignments, and
    /// [`SealErrorV1::Context`] if the preimage cannot be encoded.
    pub fn roster_root(
        &self,
        assignments: &[ParliamentSeatAssignmentV1],
    ) -> Result<Digest512, SealErrorV1> {
        let mut assignments = assignments.to_vec();
        assignments.sort_unstable_by(|left, right| {
            left.assignment_id
                .as_bytes()
                .cmp(right.assignment_id.as_bytes())
        });
        if assignments
            .windows(2)
            .any(|pair| pair[0].assignment_id == pair[1].assignment_id)
        {
            return Err(SealErrorV1::DuplicateAssignment);
        }
        if assignments.len() != self.pairs.len()
            || assignments
                .iter()
                .zip(&self.pairs)
                .any(|(assignment, pair)| assignment.assignment_id != pair.assignment_id)
        {
            return Err(SealErrorV1::MissingCredential);
        }
        let preimage = AnonymousParliamentRosterRootPreimageV1 {
            election_attempt_id: self.body_context.election_attempt_id,
            assignments,
            anonymous_ballot: Some(self.binding()),
        };
        Ok(anonymous_roster_root_v1(&preimage).map_err(ContextErrorV1::from)?)
    }

    /// The seal's block-start output: the complete canonical list bytes with the body
    /// instance and the roster root they are bound into.
    ///
    /// # Errors
    /// [`SealErrorV1::Context`] if the list cannot be encoded.
    pub fn published(
        &self,
        body_instance_id: BodyInstanceId,
        roster_root: Digest512,
    ) -> Result<SealedCredentialListPublishedV1, SealErrorV1> {
        Ok(SealedCredentialListPublishedV1 {
            body_context: self.body_context,
            body_instance_id,
            roster_root,
            list_digest: self.list_digest,
            registration_mapping_digest: self.registration_mapping_digest,
            sealed_list_bytes: SealedCredentialListBytesV1::from_list(&self.list)
                .map_err(ContextErrorV1::from)?,
        })
    }
}

/// The credentials registered for one election attempt of a binding body, before seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationLedgerV1 {
    body_context: BallotBodyContextV1,
    body_digest: Digest512,
    registered: BTreeMap<AssignmentId, RegisteredCredentialV1>,
    commitments: BTreeSet<CredentialCommitmentV1>,
}

impl RegistrationLedgerV1 {
    /// The empty ledger of `body_context`.
    ///
    /// # Errors
    /// [`RegistrationLedgerErrorV1::Context`] for a body that is not a binding body.
    pub fn new(body_context: BallotBodyContextV1) -> Result<Self, RegistrationLedgerErrorV1> {
        body_context.validate().map_err(ContextErrorV1::from)?;
        let body_digest = body_digest_v1(&body_context).map_err(ContextErrorV1::from)?;
        Ok(Self {
            body_context,
            body_digest,
            registered: BTreeMap::new(),
            commitments: BTreeSet::new(),
        })
    }

    /// Rebuild a ledger from the compact records of committed State. The proofs were
    /// verified when the records were committed; uniqueness is checked again.
    ///
    /// # Errors
    /// As [`Self::new`], [`RegistrationLedgerErrorV1::SeatRegistered`] or
    /// [`RegistrationLedgerErrorV1::DuplicateCommitment`] for records that could not
    /// have been committed.
    pub fn restore(
        body_context: BallotBodyContextV1,
        records: impl IntoIterator<Item = (AssignmentId, RegisteredCredentialV1)>,
    ) -> Result<Self, RegistrationLedgerErrorV1> {
        let mut ledger = Self::new(body_context)?;
        for (assignment_id, record) in records {
            ledger.check_fresh(&assignment_id, &record.commitment)?;
            ledger.insert(assignment_id, record);
        }
        Ok(ledger)
    }

    /// The authoritative body context.
    pub fn body_context(&self) -> &BallotBodyContextV1 {
        &self.body_context
    }

    /// The body-context digest `B`.
    pub fn body_digest(&self) -> &Digest512 {
        &self.body_digest
    }

    /// The credential registered for `assignment_id`.
    pub fn registered(&self, assignment_id: &AssignmentId) -> Option<&RegisteredCredentialV1> {
        self.registered.get(assignment_id)
    }

    /// Every recorded registration in raw assignment-identifier order.
    pub fn records(&self) -> impl Iterator<Item = (&AssignmentId, &RegisteredCredentialV1)> {
        self.registered.iter()
    }

    /// Registered seats, primaries and alternates together.
    pub fn len(&self) -> usize {
        self.registered.len()
    }

    /// Whether no seat has registered.
    pub fn is_empty(&self) -> bool {
        self.registered.is_empty()
    }

    fn check_fresh(
        &self,
        assignment_id: &AssignmentId,
        commitment: &CredentialCommitmentV1,
    ) -> Result<(), RegistrationLedgerErrorV1> {
        if self.registered.contains_key(assignment_id) {
            return Err(RegistrationLedgerErrorV1::SeatRegistered);
        }
        if self.commitments.contains(commitment) {
            return Err(RegistrationLedgerErrorV1::DuplicateCommitment);
        }
        Ok(())
    }

    fn insert(&mut self, assignment_id: AssignmentId, record: RegisteredCredentialV1) {
        self.commitments.insert(record.commitment);
        self.registered.insert(assignment_id, record);
    }

    /// Verify and record the credential that `authority` registers for its seat
    /// `assignment_id`. The node derives both from the transaction authority and its
    /// invitation; no caller-supplied body, seat or authority is trusted here.
    ///
    /// The cheap uniqueness checks run before the proof is verified. Nothing changes
    /// unless every check passes.
    ///
    /// # Errors
    /// [`RegistrationLedgerErrorV1::SeatRegistered`],
    /// [`RegistrationLedgerErrorV1::DuplicateCommitment`], or
    /// [`RegistrationLedgerErrorV1::Registration`] for a registration that does not
    /// verify or that the node could not verify
    /// ([`RegistrationLedgerErrorV1::is_local`]).
    pub fn register(
        &mut self,
        authority: &AccountId,
        assignment_id: AssignmentId,
        registration: &CredentialRegistrationV1,
    ) -> Result<RegisteredCredentialV1, RegistrationLedgerErrorV1> {
        self.check_fresh(&assignment_id, &registration.commitment)?;
        verify_registration_v1(&self.body_context, authority, &assignment_id, registration)?;
        let record = RegisteredCredentialV1 {
            commitment: registration.commitment,
            authorization: registration.authorization,
        };
        self.insert(assignment_id, record);
        Ok(record)
    }

    /// Seal exactly the `seated` assignments: an accepted alternate that is not seated
    /// contributes nothing to `L` or `R`.
    ///
    /// # Errors
    /// [`SealErrorV1::NoRoster`] for fewer than three or more than 1,000 seats,
    /// [`SealErrorV1::DuplicateAssignment`], [`SealErrorV1::MissingCredential`], or
    /// [`SealErrorV1::Context`] if the list or a digest cannot be built. Two equal
    /// commitments cannot reach the list: registration already refused the second.
    pub fn seal(&self, seated: &[AssignmentId]) -> Result<SealedRosterV1, SealErrorV1> {
        let minimum = usize::from(MIN_SEALED_SEATS_V1);
        let maximum = usize::from(MAX_SEALED_SEATS_V1);
        if !(minimum..=maximum).contains(&seated.len()) {
            return Err(SealErrorV1::NoRoster);
        }
        let mut pairs = Vec::with_capacity(seated.len());
        for assignment_id in seated {
            let record = self
                .registered
                .get(assignment_id)
                .ok_or(SealErrorV1::MissingCredential)?;
            pairs.push(SealedRegistrationPairV1 {
                assignment_id: *assignment_id,
                commitment: record.commitment,
            });
        }
        pairs.sort_unstable_by(|left, right| {
            left.assignment_id
                .as_bytes()
                .cmp(right.assignment_id.as_bytes())
        });
        if pairs
            .windows(2)
            .any(|pair| pair[0].assignment_id == pair[1].assignment_id)
        {
            return Err(SealErrorV1::DuplicateAssignment);
        }
        let list = SealedCredentialListV1::from_unsorted(
            ProofProfileIdV1::V1,
            self.body_digest,
            pairs.iter().map(|pair| pair.commitment).collect(),
        )
        .map_err(ContextErrorV1::from)?;
        let list_digest = list_digest_v1(&list).map_err(ContextErrorV1::from)?;
        let registration_mapping_digest =
            registration_mapping_v1(&list, &pairs).map_err(ContextErrorV1::from)?;
        Ok(SealedRosterV1 {
            body_context: self.body_context,
            list,
            list_digest,
            pairs,
            registration_mapping_digest,
        })
    }
}

/// Why a board transition is refused. A refused transition changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoardErrorV1 {
    /// The purpose is not one this release may record.
    #[error(transparent)]
    Purpose(#[from] ParliamentBallotPurposeErrorV1),
    /// The stored record is structurally invalid.
    #[error(transparent)]
    Record(#[from] ParliamentBallotBoardRecordErrorV1),
    /// The context could not be checked or built.
    #[error(transparent)]
    Context(#[from] ContextErrorV1),
    /// The corpus refused an entry or could not be rebuilt.
    #[error(transparent)]
    Corpus(#[from] CorpusErrorV1),
    /// The closure record could not be built.
    #[error(transparent)]
    Closure(#[from] ClosureBuildErrorV1),
    /// `sealed_timestamp_ms + vote_notice_ms` overflows.
    #[error("the opening time overflows")]
    OpeningTime,
    /// The opening time is already fixed; Reflection is entered once.
    #[error("the opening time is already fixed")]
    OpeningScheduled,
    /// The transition needs an opening time that Reflection has not fixed yet.
    #[error("the opening time is not fixed yet")]
    OpeningNotScheduled,
    /// Casting is already open.
    #[error("casting is already open")]
    AlreadyOpen,
    /// The block's time has not reached the opening time.
    #[error("the opening is not due until {opening_at_ms} ms")]
    OpeningNotDue {
        /// `opening_at`.
        opening_at_ms: u64,
    },
    /// The transition needs an open casting window.
    #[error("casting has not opened")]
    NotOpen,
    /// The casting clock is already established.
    #[error("the casting clock is already established")]
    ClockEstablished,
    /// The casting clock belongs to the first block after the opening block.
    #[error("the casting clock belongs to block {expected}, not {actual}")]
    ClockHeight {
        /// `h_open + 1`.
        expected: u64,
        /// The height offered.
        actual: u64,
    },
    /// The transition needs the casting clock.
    #[error("the casting clock is not established")]
    ClockNotEstablished,
    /// The block's time has not reached the closing time.
    #[error("the closure is not due until {closing_ms} ms")]
    ClosureNotDue {
        /// `closing_ms`.
        closing_ms: u64,
    },
    /// The ballot is closed: there is no reopening and no ballot retry.
    #[error("the ballot is closed")]
    Closed,
    /// An anchor does not follow the transition before it.
    #[error("the {transition} anchor does not follow the board's last transition")]
    Anchor {
        /// The transition whose anchor is out of order.
        transition: &'static str,
    },
    /// Stored parts do not recompute to themselves.
    #[error("the stored board does not recompute: {field}")]
    Restore {
        /// The stored field that differs from its recomputation.
        field: &'static str,
    },
}

impl BoardErrorV1 {
    /// Whether the node could not build or check the board (its context or a corpus
    /// digest), as opposed to the transition being refused. A local failure is never a
    /// committed verdict.
    pub const fn is_local(&self) -> bool {
        match self {
            Self::Context(error) => error.is_local(),
            Self::Corpus(error) => error.is_local(),
            Self::Purpose(_)
            | Self::Record(_)
            | Self::Closure(_)
            | Self::OpeningTime
            | Self::OpeningScheduled
            | Self::OpeningNotScheduled
            | Self::AlreadyOpen
            | Self::OpeningNotDue { .. }
            | Self::NotOpen
            | Self::ClockEstablished
            | Self::ClockHeight { .. }
            | Self::ClockNotEstablished
            | Self::ClosureNotDue { .. }
            | Self::Closed
            | Self::Anchor { .. }
            | Self::Restore { .. } => false,
        }
    }
}

/// The facts of committed State that a seal binds a board to. The node derives every one
/// of them; none is caller-chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSealFactsV1 {
    /// Purpose of the attempt the body belongs to, fixed when the attempt was created.
    pub purpose: ParliamentBallotPurposeV1,
    /// `proposal_digest` of the full canonical proposal.
    pub proposal_digest: Digest512,
    /// The proposal content identifier recorded at closure.
    pub proposal_content_id: ProposalContentId,
    /// The sealed body instance, derived from `roster_root`.
    pub body_instance_id: BodyInstanceId,
    /// The 64-byte roster root of the seal ([`SealedRosterV1::roster_root`]).
    pub roster_root: Digest512,
    /// Block time of the seal in milliseconds; the vote notice runs from it.
    pub sealed_timestamp_ms: u64,
    /// The body's frozen ballot-policy snapshot.
    pub policy: FrozenBallotPolicyV1,
}

/// One sealed binding body's ballot, from seal to closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BallotBoardV1 {
    purpose: ParliamentBallotPurposeV1,
    context: Arc<ScopedBallotContextV1>,
    proposal_content_id: ProposalContentId,
    roster_root: Digest512,
    registration_mapping_digest: Digest512,
    sealed_timestamp_ms: u64,
    opening_at_ms: Option<u64>,
    opening: Option<GovernanceAnchorV1>,
    casting_clock: Option<BallotCastingClockV1>,
    corpus: CorpusAccumulatorV1,
    entries: Vec<AnonymousBallotCorpusEntryV1>,
    closure: Option<BallotClosureRecordV1>,
}

impl BallotBoardV1 {
    /// The board of a body sealed with `seal`, bound to the facts of committed State the
    /// node derived for it.
    ///
    /// # Errors
    /// [`BoardErrorV1::Purpose`] for a purpose this release may not record,
    /// [`BoardErrorV1::Context`] if the context cannot be built (for example a policy of
    /// another body role), or [`BoardErrorV1::Corpus`] if the empty root cannot be
    /// computed.
    pub fn sealed(seal: &SealedRosterV1, facts: BoardSealFactsV1) -> Result<Self, BoardErrorV1> {
        facts.purpose.validate()?;
        let context = ScopedBallotContextV1::build(
            *seal.body_context(),
            facts.proposal_digest,
            facts.body_instance_id,
            seal.list(),
            facts.policy,
        )?;
        let corpus = CorpusAccumulatorV1::new(&context)?;
        Ok(Self {
            purpose: facts.purpose,
            context: Arc::new(context),
            proposal_content_id: facts.proposal_content_id,
            roster_root: facts.roster_root,
            registration_mapping_digest: *seal.registration_mapping_digest(),
            sealed_timestamp_ms: facts.sealed_timestamp_ms,
            opening_at_ms: None,
            opening: None,
            casting_clock: None,
            corpus,
            entries: Vec::new(),
            closure: None,
        })
    }

    /// Rebuild a board from its stored record, recomputing the context, the corpus and
    /// the closure record and checking the order of every anchor.
    ///
    /// # Errors
    /// [`BoardErrorV1`] for a record that no sequence of transitions could have produced.
    pub fn restore(record: ParliamentBallotBoardRecordV1) -> Result<Self, BoardErrorV1> {
        record.validate()?;
        let context = ScopedBallotContextV1::new(record.context, record.body_instance_id)?;
        let corpus = CorpusAccumulatorV1::restore(&context, &record.entries)?;
        let board = Self {
            purpose: record.purpose,
            context: Arc::new(context),
            proposal_content_id: record.proposal_content_id,
            roster_root: record.roster_root,
            registration_mapping_digest: record.registration_mapping_digest,
            sealed_timestamp_ms: record.sealed_timestamp_ms,
            opening_at_ms: record.opening_at_ms,
            opening: record.opening,
            casting_clock: record.casting_clock,
            corpus,
            entries: record.entries,
            closure: None,
        };
        board.check_restored_anchors()?;
        let Some(stored) = record.closure else {
            return Ok(board);
        };
        let rebuilt = board.build_closure(stored.closure_anchor)?;
        if rebuilt != stored {
            return Err(BoardErrorV1::Restore { field: "closure" });
        }
        Ok(Self {
            closure: Some(stored),
            ..board
        })
    }

    fn check_restored_anchors(&self) -> Result<(), BoardErrorV1> {
        let restore = |field| Err(BoardErrorV1::Restore { field });
        if let Some(opening_at_ms) = self.opening_at_ms
            && self
                .notice_end_ms()
                .is_none_or(|notice_end| opening_at_ms < notice_end)
        {
            return restore("opening_at_ms");
        }
        match (self.opening_at_ms, self.opening) {
            (None, Some(_)) => return restore("opening_at_ms"),
            (Some(opening_at_ms), Some(opening)) if opening.timestamp_ms < opening_at_ms => {
                return restore("opening");
            }
            _ => {}
        }
        match (self.opening, self.casting_clock) {
            (None, Some(_)) => return restore("casting_clock"),
            (Some(opening), Some(clock))
                if opening.height.checked_add(1) != Some(clock.height)
                    || clock.timestamp_ms < opening.timestamp_ms =>
            {
                return restore("casting_clock");
            }
            _ => {}
        }
        if let Some(first) = self.entries.first() {
            // An accepted ballot was inside the window of its block.
            let Some(opening) = self.opening else {
                return restore("entries");
            };
            if first.height < opening.height {
                return restore("entries");
            }
            if self
                .entries
                .iter()
                .any(|entry| entry.height > opening.height && self.casting_clock.is_none())
            {
                return restore("entries");
            }
        }
        Ok(())
    }

    /// The record World keeps for this board.
    pub fn record(&self) -> ParliamentBallotBoardRecordV1 {
        ParliamentBallotBoardRecordV1 {
            purpose: self.purpose,
            context: self.context.context().clone(),
            body_instance_id: *self.context.body_instance_id(),
            proposal_content_id: self.proposal_content_id,
            roster_root: self.roster_root,
            registration_mapping_digest: self.registration_mapping_digest,
            sealed_timestamp_ms: self.sealed_timestamp_ms,
            opening_at_ms: self.opening_at_ms,
            opening: self.opening,
            casting_clock: self.casting_clock,
            entries: self.entries.clone(),
            closure: self.closure,
        }
    }

    /// Purpose of the attempt the body belongs to.
    pub fn purpose(&self) -> &ParliamentBallotPurposeV1 {
        &self.purpose
    }

    /// The proposal content identifier fixed by the original seal and recorded at closure.
    pub const fn proposal_content_id(&self) -> ProposalContentId {
        self.proposal_content_id
    }

    /// The 64-byte roster root the body instance was derived from.
    pub fn roster_root(&self) -> &Digest512 {
        &self.roster_root
    }

    /// Block time of the seal in milliseconds.
    pub fn sealed_timestamp_ms(&self) -> u64 {
        self.sealed_timestamp_ms
    }

    /// End of the vote notice, `sealed_at + vote_notice_ms`.
    fn notice_end_ms(&self) -> Option<u64> {
        self.sealed_timestamp_ms
            .checked_add(self.context.policy().binding.vote_notice_ms)
    }

    /// The scope's checked verification context.
    pub fn context(&self) -> &Arc<ScopedBallotContextV1> {
        &self.context
    }

    /// The ballot scope.
    pub fn scope(&self) -> &BallotScopeV1 {
        self.context.scope()
    }

    /// `opening_at`, once the body entered Reflection.
    pub fn opening_at_ms(&self) -> Option<u64> {
        self.opening_at_ms
    }

    /// The opening anchor `s0`, once casting opened.
    pub fn opening(&self) -> Option<&GovernanceAnchorV1> {
        self.opening.as_ref()
    }

    /// The casting clock, once the block after the opening block established it.
    pub fn casting_clock(&self) -> Option<&BallotCastingClockV1> {
        self.casting_clock.as_ref()
    }

    /// `closing_ms`, once the casting clock is established.
    pub fn closing_ms(&self) -> Option<u64> {
        let clock = self.casting_clock?;
        closing_ms_v1(
            clock.timestamp_ms,
            self.context.policy().binding.casting_window_ms,
        )
    }

    /// The accepted corpus.
    pub fn corpus(&self) -> &CorpusAccumulatorV1 {
        &self.corpus
    }

    /// The accepted corpus entries in canonical order.
    pub fn entries(&self) -> &[AnonymousBallotCorpusEntryV1] {
        &self.entries
    }

    /// The closure record, once the ballot closed.
    pub fn closure(&self) -> Option<&BallotClosureRecordV1> {
        self.closure.as_ref()
    }

    /// The casting window, once casting opened.
    pub fn window(&self) -> Option<CastingWindowV1> {
        Some(CastingWindowV1 {
            opening_height: self.opening?.height,
            closing_ms: self.closing_ms(),
            closure_height: self
                .closure
                .as_ref()
                .map(|record| record.closure_anchor.height),
        })
    }

    /// Whether a ballot executed in the block at `height` with block time `timestamp_ms`
    /// is inside the casting window.
    pub fn casting_open_at(&self, height: u64, timestamp_ms: u64) -> bool {
        self.window()
            .is_some_and(|window| window.is_open_at(height, timestamp_ms))
    }

    /// The window status queue admission reads, with the node's local clock `now_ms`
    /// deciding only whether an unopened window is due. Execution always rechecks against
    /// committed state.
    pub fn status_at(&self, now_ms: u64) -> CastingWindowStatusV1 {
        if self.closure.is_some() || self.closing_ms().is_some_and(|closing| now_ms >= closing) {
            return CastingWindowStatusV1::Closed;
        }
        if self.opening.is_some() {
            return CastingWindowStatusV1::Open;
        }
        match self.opening_at_ms {
            Some(opening_at_ms) if now_ms >= opening_at_ms => CastingWindowStatusV1::LocallyDue,
            _ => CastingWindowStatusV1::NotYetDue,
        }
    }

    /// Fix `opening_at = max(t_r, sealed_at + vote_notice_ms)` when the body enters
    /// Reflection at block time `reflection_timestamp_ms`.
    ///
    /// # Errors
    /// [`BoardErrorV1::OpeningScheduled`] if Reflection was already entered, or
    /// [`BoardErrorV1::OpeningTime`] on overflow.
    pub fn schedule_opening(&mut self, reflection_timestamp_ms: u64) -> Result<u64, BoardErrorV1> {
        if self.opening_at_ms.is_some() {
            return Err(BoardErrorV1::OpeningScheduled);
        }
        let notice_end = self.notice_end_ms().ok_or(BoardErrorV1::OpeningTime)?;
        let opening_at_ms = reflection_timestamp_ms.max(notice_end);
        self.opening_at_ms = Some(opening_at_ms);
        Ok(opening_at_ms)
    }

    /// Whether the opening transition is due in a block of time `block_timestamp_ms`.
    /// The node additionally applies the concurrency bound `K` (design §3.6).
    pub fn opening_due(&self, block_timestamp_ms: u64) -> bool {
        self.opening.is_none()
            && self
                .opening_at_ms
                .is_some_and(|opening_at_ms| block_timestamp_ms >= opening_at_ms)
    }

    /// Open casting in the block-start pass of the block of `anchor`; the anchor is `s0`.
    /// Ballots of that block are inside the window.
    ///
    /// # Errors
    /// [`BoardErrorV1::OpeningNotScheduled`], [`BoardErrorV1::AlreadyOpen`] or
    /// [`BoardErrorV1::OpeningNotDue`].
    pub fn open(&mut self, anchor: GovernanceAnchorV1) -> Result<CastingOpenedV1, BoardErrorV1> {
        let opening_at_ms = self
            .opening_at_ms
            .ok_or(BoardErrorV1::OpeningNotScheduled)?;
        if self.opening.is_some() {
            return Err(BoardErrorV1::AlreadyOpen);
        }
        if anchor.timestamp_ms < opening_at_ms {
            return Err(BoardErrorV1::OpeningNotDue { opening_at_ms });
        }
        self.opening = Some(anchor);
        Ok(CastingOpenedV1 {
            body_context: *self.context.body_context(),
            body_instance_id: *self.context.body_instance_id(),
            scope: *self.context.scope(),
            policy_hash: *self.context.policy_hash(),
            opening_at_ms,
        })
    }

    /// Whether the block at `height` must establish the casting clock: it is the first
    /// block after the opening block.
    pub fn casting_clock_due(&self, height: u64) -> bool {
        self.casting_clock.is_none()
            && self
                .opening
                .is_some_and(|opening| opening.height.checked_add(1) == Some(height))
    }

    /// Freeze `closing_ms = t(h_open + 1) + casting_window_ms` in the prelude of the first
    /// block after the opening block, before that block's transactions. A stale opening
    /// therefore never shortens voting.
    ///
    /// # Errors
    /// [`BoardErrorV1::NotOpen`], [`BoardErrorV1::ClockEstablished`],
    /// [`BoardErrorV1::ClockHeight`], [`BoardErrorV1::Anchor`] for a block time before
    /// the opening block's, or [`BoardErrorV1::Closure`] if the closing time overflows.
    pub fn establish_casting_clock(
        &mut self,
        height: u64,
        timestamp_ms: u64,
    ) -> Result<CastingClockEstablishedV1, BoardErrorV1> {
        let opening = self.opening.ok_or(BoardErrorV1::NotOpen)?;
        if self.casting_clock.is_some() {
            return Err(BoardErrorV1::ClockEstablished);
        }
        let expected = opening.height.checked_add(1).ok_or(BoardErrorV1::Anchor {
            transition: "casting clock",
        })?;
        if height != expected {
            return Err(BoardErrorV1::ClockHeight {
                expected,
                actual: height,
            });
        }
        if timestamp_ms < opening.timestamp_ms {
            return Err(BoardErrorV1::Anchor {
                transition: "casting clock",
            });
        }
        let casting_window_ms = self.context.policy().binding.casting_window_ms;
        let closing_ms = closing_ms_v1(timestamp_ms, casting_window_ms)
            .ok_or(ClosureBuildErrorV1::ClosingTime)?;
        self.casting_clock = Some(BallotCastingClockV1 {
            height,
            timestamp_ms,
        });
        Ok(CastingClockEstablishedV1 {
            scope: *self.context.scope(),
            casting_window_ms,
            closing_ms,
        })
    }

    /// Whether the closure is due in a block of time `block_timestamp_ms`.
    pub fn closure_due(&self, block_timestamp_ms: u64) -> bool {
        self.closure.is_none()
            && self
                .closing_ms()
                .is_some_and(|closing_ms| block_timestamp_ms >= closing_ms)
    }

    fn build_closure(
        &self,
        closure: GovernanceAnchorV1,
    ) -> Result<BallotClosureRecordV1, BoardErrorV1> {
        let opening = self.opening.ok_or(BoardErrorV1::NotOpen)?;
        let clock = self
            .casting_clock
            .ok_or(BoardErrorV1::ClockNotEstablished)?;
        Ok(build_closure_record_v1(
            &self.context,
            self.proposal_content_id,
            self.registration_mapping_digest,
            &ClosureAnchorsV1 {
                opening,
                casting_clock_height: clock.height,
                casting_clock_timestamp_ms: clock.timestamp_ms,
                closure,
            },
            &self.corpus,
        )?)
    }

    /// Close the ballot in the block-start pass of the block of `anchor`, before that
    /// block's transactions: freeze the corpus and derive the outcome. There is no early
    /// close and no reopening.
    ///
    /// # Errors
    /// [`BoardErrorV1::Closed`], [`BoardErrorV1::NotOpen`],
    /// [`BoardErrorV1::ClockNotEstablished`], [`BoardErrorV1::ClosureNotDue`], or
    /// [`BoardErrorV1::Closure`] if the record cannot be built.
    pub fn close(
        &mut self,
        anchor: GovernanceAnchorV1,
    ) -> Result<&BallotClosureRecordV1, BoardErrorV1> {
        if self.closure.is_some() {
            return Err(BoardErrorV1::Closed);
        }
        if self.opening.is_none() {
            return Err(BoardErrorV1::NotOpen);
        }
        let closing_ms = self.closing_ms().ok_or(BoardErrorV1::ClockNotEstablished)?;
        if anchor.timestamp_ms < closing_ms {
            return Err(BoardErrorV1::ClosureNotDue { closing_ms });
        }
        let record = self.build_closure(anchor)?;
        Ok(self.closure.insert(record))
    }

    /// Append an accepted ballot's entry: the corpus root, the nullifier index and the
    /// counts change together or not at all.
    fn accept(&mut self, entry: &AnonymousBallotCorpusEntryV1) -> Result<u32, BoardErrorV1> {
        if self.closure.is_some() {
            return Err(BoardErrorV1::Closed);
        }
        let index = self.corpus.append(entry)?;
        self.entries.push(*entry);
        Ok(index)
    }
}

/// The boards a node holds, by scope. The node's State implements this over its own
/// storage; a map of boards implements it for verification and tests.
pub trait BoardStoreV1 {
    /// The board of `scope`, if a sealed binding body has that scope.
    fn board(&self, scope: &BallotScopeV1) -> Option<&BallotBoardV1>;

    /// The board of `scope` for the one mutation of an accepted ballot.
    fn board_mut(&mut self, scope: &BallotScopeV1) -> Option<&mut BallotBoardV1>;
}

impl BoardStoreV1 for BTreeMap<BallotScopeV1, BallotBoardV1> {
    fn board(&self, scope: &BallotScopeV1) -> Option<&BallotBoardV1> {
        self.get(scope)
    }

    fn board_mut(&mut self, scope: &BallotScopeV1) -> Option<&mut BallotBoardV1> {
        self.get_mut(scope)
    }
}

/// The committed-state answers of block execution at one block: every answer reflects
/// the earlier inputs of the same block, because accepted ballots are appended to their
/// boards as they execute.
#[derive(Debug)]
pub struct BoardsAtBlockV1<'boards, S: BoardStoreV1 + ?Sized> {
    network_id: NetworkId,
    height: u64,
    timestamp_ms: u64,
    boards: &'boards S,
}

impl<'boards, S: BoardStoreV1 + ?Sized> BoardsAtBlockV1<'boards, S> {
    /// The view of `boards` for the block at `height` with block time `timestamp_ms` on
    /// the network `network_id`.
    pub const fn new(
        network_id: NetworkId,
        height: NonZeroU64,
        timestamp_ms: u64,
        boards: &'boards S,
    ) -> Self {
        Self {
            network_id,
            height: height.get(),
            timestamp_ms,
            boards,
        }
    }
}

impl<S: BoardStoreV1 + ?Sized> BlockBallotStateV1 for BoardsAtBlockV1<'_, S> {
    fn network_id(&self) -> NetworkId {
        self.network_id
    }

    fn scope_context(&self, scope: &BallotScopeV1) -> Option<Arc<ScopedBallotContextV1>> {
        self.boards
            .board(scope)
            .map(|board| Arc::clone(board.context()))
    }

    fn casting_open(&self, scope: &BallotScopeV1) -> bool {
        self.boards
            .board(scope)
            .is_some_and(|board| board.casting_open_at(self.height, self.timestamp_ms))
    }

    fn nullifier_accepted(&self, scope: &BallotScopeV1, nullifier: &NullifierV1) -> bool {
        self.boards
            .board(scope)
            .is_some_and(|board| board.corpus().index_of(nullifier).is_some())
    }
}

/// The committed-state answers of queue admission at the node's local clock.
///
/// The local clock decides only whether an unopened window is locally due; execution
/// rechecks every answer against committed state. `pending` is the queue's own index of
/// verified pending ballots: the board holds committed state only and never reserves a
/// nullifier for a ballot that has not verified.
pub struct BoardsAtAdmissionV1<'boards, S: BoardStoreV1 + ?Sized, P> {
    network_id: NetworkId,
    now_ms: u64,
    boards: &'boards S,
    pending: P,
}

impl<'boards, S, P> BoardsAtAdmissionV1<'boards, S, P>
where
    S: BoardStoreV1 + ?Sized,
    P: Fn(&BallotScopeV1, &NullifierV1) -> bool,
{
    /// The admission view of `boards` on the network `network_id` at local time `now_ms`,
    /// with the queue's verified-pending lookup `pending`.
    pub const fn new(network_id: NetworkId, now_ms: u64, boards: &'boards S, pending: P) -> Self {
        Self {
            network_id,
            now_ms,
            boards,
            pending,
        }
    }
}

impl<S, P> AdmissionStateV1 for BoardsAtAdmissionV1<'_, S, P>
where
    S: BoardStoreV1 + ?Sized,
    P: Fn(&BallotScopeV1, &NullifierV1) -> bool,
{
    fn network_id(&self) -> NetworkId {
        self.network_id
    }

    fn scope(&self, scope: &BallotScopeV1) -> Option<AdmissionScopeV1> {
        self.boards.board(scope).map(|board| AdmissionScopeV1 {
            context: Arc::clone(board.context()),
            window: board.status_at(self.now_ms),
        })
    }

    fn nullifier_committed(&self, scope: &BallotScopeV1, nullifier: &NullifierV1) -> bool {
        self.boards
            .board(scope)
            .is_some_and(|board| board.corpus().index_of(nullifier).is_some())
    }

    fn verified_pending_holder(&self, scope: &BallotScopeV1, nullifier: &NullifierV1) -> bool {
        (self.pending)(scope, nullifier)
    }
}

/// Why the node could not execute an anonymous-ballot input. Neither variant is a
/// decision about the input: the caller must not accept or reject the block on its
/// account.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BallotExecutionErrorV1 {
    /// The node could not verify the input.
    #[error(transparent)]
    Unavailable(#[from] VerificationUnavailableV1),
    /// An accepted decision could not be applied to its board, which a consistent board
    /// never refuses.
    #[error("an accepted ballot could not be applied to its board: {0}")]
    Board(#[from] BoardErrorV1),
}

/// Execute one anonymous-ballot input of the block at `height` (block time
/// `timestamp_ms`), at position `input_index` of its global network-input projection.
///
/// The input is decided completely against `boards` ([`decide_block_ballot_v1`]: size,
/// framing, network, scope, profile, stamp and proof, then the window before the
/// duplicate check). An accepted ballot is appended to its board in the same step, so the
/// next input of the block sees it. A rejected or block-invalid input changes nothing.
///
/// # Errors
/// [`BallotExecutionErrorV1`] when the node cannot decide or apply; nothing changed.
pub fn execute_block_ballot_v1<S: BoardStoreV1 + ?Sized>(
    network_id: NetworkId,
    boards: &mut S,
    envelope_bytes: &[u8],
    height: NonZeroU64,
    timestamp_ms: u64,
    input_index: u32,
) -> Result<BlockBallotDecisionV1, BallotExecutionErrorV1> {
    let decision = decide_block_ballot_v1(
        &BoardsAtBlockV1::new(network_id, height, timestamp_ms, &*boards),
        envelope_bytes,
        height,
        input_index,
    )?;
    if let BlockBallotDecisionV1::Accepted(entry) = &decision {
        let board = boards
            .board_mut(&entry.scope)
            .ok_or(VerificationUnavailableV1::ScopeLookup)?;
        board.accept(entry)?;
    }
    Ok(decision)
}

#[cfg(test)]
mod tests;

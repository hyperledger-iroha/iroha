//! Signerless anonymous-ballot inputs, executed against the boards World keeps
//! (`specs/parliament_private_ballot_design.md` §3.5 to §3.7 and §12.3 slice 4).
//!
//! An anonymous-ballot input carries one canonical envelope. It has no signer, no fee, no
//! creation time and no instruction: its proof alone authorizes it, so it cannot move the
//! governance clock, invoke a contract or trigger, or name an account. A block of the
//! global chain executes it after that block's governance pass, in input order, and the
//! decision is the protocol crate's
//! ([`iroha_parliament_ballot::board::execute_block_ballot_v1`]):
//!
//! 1. size, canonical framing with exact fields, network, a known scope, the V1 profile, the
//!    stamp at the scope's frozen difficulty and the proof are checked completely. A failure
//!    of any of them makes the block that carries the input invalid
//!    ([`BallotInputErrorV1::BlockInvalid`]): an honest proposer never includes such an
//!    input, and nothing is recorded for it;
//! 2. a valid ballot outside its scope's casting window is recorded as
//!    [`AnonymousBallotRejectionV1::OutsideCastingWindow`]. The window is the board's: open
//!    from the opening pass of its block, and closed from the block whose pass processed the
//!    closure or whose time reached the closing time (§3.4);
//! 3. a valid ballot inside the window whose nullifier the scope's corpus already holds is
//!    recorded as [`AnonymousBallotRejectionV1::DuplicateNullifier`]: the first ballot in
//!    canonical execution order wins its nullifier, across blocks and inside one;
//! 4. otherwise the ballot is accepted: its compact entry joins the board's corpus at
//!    `(height, input index)` and the board is written back, in the caller's transaction, so
//!    the next input of the block sees it and a dropped block forgets it.
//!
//! A ballot is identified by `(scope, nullifier)`, never by the hash of its carrier: the
//! same bytes may be relayed by anyone and may reach a later block, where the window and the
//! corpus decide them again (§3.5, §5 item 3).
//!
//! The board is recomputed from its stored record before it decides anything and is never
//! trusted. A stored board that does not recompute, a verifier the node cannot run and a
//! pinned frame it cannot produce are the node's own failures ([`BallotInputErrorV1::is_local`]):
//! the block is deferred, never judged.
//!
//! Only a block whose governance pass ran Parliament's items executes ballots: the caller
//! refuses the input on every other root and carrier
//! (`crate::state::StateBlock::parliament_pass_ran`), where no board exists.
//!
//! **Per-block budget** ([`BallotBlockBudgetV1`], design §3.6 "Per block" and §3.10). A block
//! carries at most `B_work` ballots and `B_bytes` verifier-input bytes, both read from the
//! `gov.parliament_ballot` value committed on the block's parent, or its default. Every
//! ballot input is charged one whole verification reservation and its copied input bytes,
//! the canonical envelope and the canonical authenticated context of its scope, whatever
//! its outcome. The proposer's selection and block validation count with the same budget,
//! so a block its proposer built always fits.
//!
//! TODO(design §3.6 "Capacity rule", `iroha_data_model::parameter::parliament_ballot`): the
//! reserved ballot slots and wire bytes are not yet subtracted from the block's ordinary
//! input-count and wire limits, and an update of `gov.parliament_ballot` is not validated
//! against outstanding capacity commitments. Both belong to inclusion qualification.

use std::cell::OnceCell;

use iroha_data_model::{
    governance::anonymous_ballot::{
        AnonymousBallotCodecErrorV1, AnonymousBallotRejectionV1, BallotScopeV1,
        CanonicalAnonymousBallotEnvelopeV1, EncodedBallotContextV1, ParliamentBallotBoardRecordV1,
    },
    parameter::Parameters,
};
use iroha_parliament_ballot::{
    ballot::{BlockBallotDecisionV1, BlockInvalidReasonV1, VerificationUnavailableV1},
    board::{
        BallotBoardV1, BallotExecutionErrorV1, BoardErrorV1, BoardStoreV1, execute_block_ballot_v1,
    },
};
use mv::storage::StorageReadOnly;

use super::world_pass::committed_ballot_parameters;
use crate::state::{StateTransaction, parliament_anonymous_tables::store_board};

/// What block execution recorded for one valid anonymous-ballot input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BallotInputOutcomeV1 {
    /// The ballot joined its scope's corpus: the input's row is an empty success.
    Accepted,
    /// The proof is valid, but the ballot is outside its window or its nullifier is taken:
    /// the input's row is this rejection and nothing changed.
    Rejected(AnonymousBallotRejectionV1),
}

/// Why an anonymous-ballot input has no outcome.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum BallotInputErrorV1 {
    /// The input makes the block that carries it invalid.
    #[error("an anonymous-ballot input makes its block invalid: {0}")]
    BlockInvalid(BlockInvalidReasonV1),
    /// The node could not verify the input.
    #[error("the node cannot verify an anonymous-ballot input: {0}")]
    Unavailable(VerificationUnavailableV1),
    /// The board World keeps for the ballot's scope does not recompute, or refused an
    /// entry its own decision had accepted. State that restore accepted never causes it.
    #[error("the stored ballot board of the input's scope is unusable: {0}")]
    Board(BoardErrorV1),
}

impl BallotInputErrorV1 {
    /// Whether the node could not decide. Only [`Self::BlockInvalid`] is a verdict on the
    /// block; every other refusal defers it.
    pub(crate) const fn is_local(&self) -> bool {
        !matches!(self, Self::BlockInvalid(_))
    }
}

/// The one board an input can name, recomputed from its stored record on first use.
///
/// The protocol crate asks for the board of the scope the envelope carries, after it has
/// decoded the envelope: the store answers from World then, so the envelope is decoded
/// once. A record that does not recompute is kept as the refusal it is and read by the
/// caller, because "no board" would wrongly judge the input. Block execution and queue
/// admission both read their board through it.
pub(crate) struct ScopeBoardV1<'boards, Boards> {
    boards: &'boards Boards,
    loaded: OnceCell<Option<(BallotScopeV1, Result<BallotBoardV1, BoardErrorV1>)>>,
}

impl<'boards, Boards> ScopeBoardV1<'boards, Boards>
where
    Boards: StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
{
    /// A store over the board table `boards`; nothing is read until a scope is asked for.
    pub(crate) const fn new(boards: &'boards Boards) -> Self {
        Self {
            boards,
            loaded: OnceCell::new(),
        }
    }

    /// The refusal of the stored record, when the scope has one that does not recompute.
    pub(crate) fn refusal(&self) -> Option<&BoardErrorV1> {
        match self.loaded.get()? {
            Some((_, Err(error))) => Some(error),
            _ => None,
        }
    }

    /// The board after the decision, when the scope has one.
    fn into_board(self) -> Option<(BallotScopeV1, BallotBoardV1)> {
        match self.loaded.into_inner()? {
            Some((scope, Ok(board))) => Some((scope, board)),
            _ => None,
        }
    }
}

impl<Boards> BoardStoreV1 for ScopeBoardV1<'_, Boards>
where
    Boards: StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
{
    fn board(&self, scope: &BallotScopeV1) -> Option<&BallotBoardV1> {
        let loaded = self.loaded.get_or_init(|| {
            self.boards
                .get(scope)
                .map(|record| (*scope, BallotBoardV1::restore(record.clone())))
        });
        match loaded {
            Some((held, Ok(board))) if held == scope => Some(board),
            _ => None,
        }
    }

    fn board_mut(&mut self, scope: &BallotScopeV1) -> Option<&mut BallotBoardV1> {
        match self.loaded.get_mut()? {
            Some((held, Ok(board))) if held == scope => Some(board),
            _ => None,
        }
    }
}

/// Execute the anonymous-ballot input `envelope` at position `input_index` of the block
/// `tx` belongs to, against the board World keeps for its scope.
///
/// An accepted ballot's board is stored in `tx`; a rejected, invalid or undecided input
/// changes nothing. The caller owns the transaction: it applies it with the input's row and
/// drops it otherwise.
///
/// # Errors
/// [`BallotInputErrorV1::BlockInvalid`] for an input no valid block carries, and a local
/// refusal ([`BallotInputErrorV1::is_local`]) when the node cannot decide.
pub(crate) fn execute_ballot_input(
    tx: &mut StateTransaction<'_, '_>,
    envelope: &CanonicalAnonymousBallotEnvelopeV1,
    input_index: u32,
) -> Result<BallotInputOutcomeV1, BallotInputErrorV1> {
    let header = tx._curr_block;
    let network_id = tx.network_id;
    let mut store = ScopeBoardV1::new(&tx.world.parliament_anonymous_boards);
    let decision = execute_block_ballot_v1(
        network_id,
        &mut store,
        envelope.as_bytes(),
        header.height(),
        header.creation_time_ms,
        input_index,
    );
    // A stored board that does not recompute answered "no board": the decision made on
    // that answer is not one.
    if let Some(refusal) = store.refusal() {
        return Err(BallotInputErrorV1::Board(refusal.clone()));
    }
    match decision {
        Ok(BlockBallotDecisionV1::Accepted(entry)) => {
            let (scope, board) = store
                .into_board()
                .filter(|(scope, _)| *scope == entry.scope)
                .ok_or(BallotInputErrorV1::Unavailable(
                    VerificationUnavailableV1::ScopeLookup,
                ))?;
            store_board(&mut tx.world, scope, board.record());
            Ok(BallotInputOutcomeV1::Accepted)
        }
        Ok(BlockBallotDecisionV1::Rejected(reason)) => Ok(BallotInputOutcomeV1::Rejected(reason)),
        Ok(BlockBallotDecisionV1::BlockInvalid(reason)) => {
            Err(BallotInputErrorV1::BlockInvalid(reason))
        }
        Err(BallotExecutionErrorV1::Unavailable(error)) => {
            Err(BallotInputErrorV1::Unavailable(error))
        }
        Err(BallotExecutionErrorV1::Board(error)) => Err(BallotInputErrorV1::Board(error)),
    }
}

/// Why the anonymous-ballot inputs of a block do not fit its budget, or cannot be charged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum BallotBudgetErrorV1 {
    /// The committed `gov.parliament_ballot` value does not decode or is out of range.
    #[error("committed ballot policy: {0}")]
    Policy(&'static str),
    /// An input is not one canonical envelope frame.
    #[error("an anonymous-ballot input is not one canonical envelope: {0}")]
    Malformed(AnonymousBallotCodecErrorV1),
    /// No sealed binding body of the block's parent State has the ballot's scope.
    #[error("an anonymous-ballot input names a scope without a sealed board")]
    UnknownScope,
    /// The block carries one byte-identical carrier twice.
    #[error("the block carries one anonymous-ballot carrier twice")]
    DuplicateCarrier,
    /// The block carries more ballots than `B_work`.
    #[error("the block carries more than {limit} anonymous ballots")]
    Reservations {
        /// `B_work`.
        limit: u32,
    },
    /// The ballots of the block copy more verifier-input bytes than `B_bytes`.
    #[error("the block's anonymous ballots exceed {limit} verifier-input bytes")]
    Bytes {
        /// `B_bytes`.
        limit: u64,
    },
    /// The node could not decode an envelope or encode a context with its own resources.
    #[error("the node cannot size an anonymous-ballot input: {0}")]
    Unavailable(AnonymousBallotCodecErrorV1),
}

impl BallotBudgetErrorV1 {
    /// Whether the node could not decide; every other variant is a verdict on the block.
    pub(crate) const fn is_local(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
}

/// The verifier-input bytes one verification of `envelope` copies: its canonical envelope and
/// the canonical authenticated context of its scope, whose board `board_of` answers.
///
/// # Errors
/// [`BallotBudgetErrorV1::Malformed`], [`BallotBudgetErrorV1::UnknownScope`] or a local
/// [`BallotBudgetErrorV1::Unavailable`].
pub(crate) fn verifier_input_bytes<'board>(
    board_of: impl FnOnce(&BallotScopeV1) -> Option<&'board ParliamentBallotBoardRecordV1>,
    envelope: &CanonicalAnonymousBallotEnvelopeV1,
) -> Result<u64, BallotBudgetErrorV1> {
    let classify = |error: AnonymousBallotCodecErrorV1| match error {
        AnonymousBallotCodecErrorV1::DecodeUnavailable(_) => {
            BallotBudgetErrorV1::Unavailable(error)
        }
        error => BallotBudgetErrorV1::Malformed(error),
    };
    let scope = envelope.envelope().map_err(classify)?.ballot.scope;
    let board = board_of(&scope).ok_or(BallotBudgetErrorV1::UnknownScope)?;
    // A stored context always encodes within its cap: a failure here is the node's own.
    let context =
        EncodedBallotContextV1::new(&board.context).map_err(BallotBudgetErrorV1::Unavailable)?;
    let bytes = envelope
        .as_bytes()
        .len()
        .saturating_add(context.as_bytes().len());
    Ok(u64::try_from(bytes).unwrap_or(u64::MAX))
}

/// What remains of one block's ballot budget (`B_work` reservations and `B_bytes` bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BallotBlockBudgetV1 {
    reservations: u32,
    bytes: u64,
    reservation_limit: u32,
    byte_limit: u64,
}

impl BallotBlockBudgetV1 {
    /// The budget of a block whose parent commits `parameters`: their `gov.parliament_ballot`
    /// value, or the default while none is committed.
    ///
    /// # Errors
    /// [`BallotBudgetErrorV1::Policy`] for a committed value that does not decode or is out
    /// of range.
    pub(crate) fn of(parameters: &Parameters) -> Result<Self, BallotBudgetErrorV1> {
        let ballot = committed_ballot_parameters(parameters).map_err(|_| {
            BallotBudgetErrorV1::Policy("gov.parliament_ballot does not decode or is out of range")
        })?;
        Ok(Self {
            reservations: ballot.max_verification_reservations_per_block,
            bytes: ballot.max_verifier_input_bytes_per_block,
            reservation_limit: ballot.max_verification_reservations_per_block,
            byte_limit: ballot.max_verifier_input_bytes_per_block,
        })
    }

    /// Charge one ballot whose verification copies `verifier_input_bytes`: one whole
    /// reservation and those bytes. A ballot that does not fit charges nothing.
    ///
    /// # Errors
    /// [`BallotBudgetErrorV1::Reservations`] or [`BallotBudgetErrorV1::Bytes`].
    pub(crate) fn charge(&mut self, verifier_input_bytes: u64) -> Result<(), BallotBudgetErrorV1> {
        // SPEC: §13.4 HC812 — every ballot input takes one whole reservation of `B_work`.
        let reservations = if cfg!(all(test, sumeragi_core_mutation = "HC812")) {
            self.reservations
        } else {
            self.reservations
                .checked_sub(1)
                .ok_or(BallotBudgetErrorV1::Reservations {
                    limit: self.reservation_limit,
                })?
        };
        let bytes =
            self.bytes
                .checked_sub(verifier_input_bytes)
                .ok_or(BallotBudgetErrorV1::Bytes {
                    limit: self.byte_limit,
                })?;
        self.reservations = reservations;
        self.bytes = bytes;
        Ok(())
    }
}

/// Check that the anonymous-ballot inputs `ballots` of one block, in input order, fit the
/// budget its parent's `parameters` commit, and that the block carries each carrier once.
/// `board_of` answers the board the parent keeps for a scope.
///
/// A second byte-identical carrier in one block is refused: every Network input owns one
/// FASTPQ source invocation under its execution call, and an honest queue holds a carrier
/// once. Distinct carriers under one nullifier are decided in canonical order instead.
///
/// # Errors
/// The first [`BallotBudgetErrorV1`]; a local one is never a verdict on the block.
pub(crate) fn check_block_ballot_budget<'ballots, 'board>(
    parameters: &Parameters,
    board_of: impl Fn(&BallotScopeV1) -> Option<&'board ParliamentBallotBoardRecordV1>,
    ballots: impl IntoIterator<Item = &'ballots CanonicalAnonymousBallotEnvelopeV1>,
) -> Result<(), BallotBudgetErrorV1> {
    let mut budget = None;
    let mut carriers = std::collections::BTreeSet::new();
    for envelope in ballots {
        if !carriers.insert(envelope.as_bytes()) {
            return Err(BallotBudgetErrorV1::DuplicateCarrier);
        }
        let budget = match &mut budget {
            Some(budget) => budget,
            // A block without ballots never reads the committed policy.
            empty => empty.insert(BallotBlockBudgetV1::of(parameters)?),
        };
        budget.charge(verifier_input_bytes(&board_of, envelope)?)?;
    }
    Ok(())
}

// The queue's ballot partition tests admit these fixture ballots too.
#[cfg(test)]
#[path = "ballot_input_tests.rs"]
pub(crate) mod tests;

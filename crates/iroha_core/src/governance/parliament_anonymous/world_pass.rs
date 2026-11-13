//! The anonymous family's own items of the positioned pass, applied to World, and the
//! driver of the one block-start pass (design §8.1, §9 and §12.3 slice 3; `specs/sccp.md`
//! §4.7.2).
//!
//! [`run_governance_pass`] runs once per block of the global chain. It establishes the
//! mandatory casting clocks, then drives G1 (the height-progress ticket and the family's
//! draws), G2a (the family's lifecycle items merged with the other owner's rank-three
//! items), G2b closures, G2c openings and G3 (the other owner's enactments) through the one
//! [`SharedGovernancePassV1`] cursor. Each segment is listed from the State the earlier
//! segments left, so a closure frees its casting slot for an opening of the same pass and a
//! Reflection entry whose opening time has been reached opens in that pass. Each item is
//! applied in its own World transaction: an attempt, its board and their outputs change
//! together, and a refused item changes nothing.
//!
//! **Outputs.** The pass owns the governance-start partition of its block
//! ([`GovernanceStartOutputBudgetV1`]), reserved before anything mutates. The owner asks the
//! partition whether an item's rows fit before the item is charged, and an item that does
//! not fit stays due ([`GovernanceOccurrenceV1::NoOutputCapacity`]). An item's rows enter
//! the partition before its World transaction is applied: its compact receipt
//! ([`LifecycleAppliedV1`]), then at most one ballot output.
//!
//! An item whose attempt ended or whose occurrence passed lapses, uncharged. Every other
//! refusal of a listed item is an error of the pass: the caller abandons the block. A
//! refusal the node cannot decide ([`AnonymousPassWorldErrorV1::is_local`]) defers the
//! block and is never a verdict.
//!
//! TODO(design §12.3, slice 3, step 4 of the dispatch work): listing reads every attempt and
//! board, terminal ones included. A derived due index bounded by `K`, rebuilt on restore,
//! replaces `draw_items`, `lifecycle_items`, `closure_items`, `opening_items` and the clock
//! scan of `establish_casting_clocks`. It stopped here: nonterminal attempts are bounded by
//! `K` only through the admission cap of the attempt-creation interface, which does not exist
//! yet, and the index is a new World member with its own restore, snapshot and inventory
//! owners. Production enactment (G3) belongs to slice 5; this release certifies no
//! production attempt.

use core::cell::RefCell;

use iroha_data_model::{
    block::output_budget::{
        GovernanceStartItemCeilingV1, GovernanceStartOutputBudgetErrorV1,
        GovernanceStartOutputBudgetV1,
    },
    governance::{
        GovernanceAnchorV1,
        anonymous_ballot::{
            AdvanceToNativeSlotV1, AnonymousBallotCodecErrorV1, BallotScopeV1,
            GovernanceBlockStartEventV1, GovernanceBlockStartOutputV1, GovernanceLifecycleItemV1,
            GovernanceOutputAnchorV1, LifecycleAppliedV1, LifecycleBoardDigestV1,
            NativeSlotAdvancedV1, ParliamentBallotBoardRecordV1,
            digests::{attempt_state_digest_v1, board_record_digest_v1},
        },
        types::{BodyInstanceId, GovernanceAttemptId, GovernanceAttemptStatusV1, ParliamentBody},
    },
    parameter::ParliamentBallotParametersV1,
};
use iroha_parliament_ballot::board::BallotBoardV1;
use mv::storage::StorageReadOnly;

use super::{
    AnonymousLifecycleItemV1, AnonymousParliamentErrorV1 as Error, AnonymousSortitionPulseV1,
    is_binding,
    pass::{
        GovernanceOccurrenceV1, GovernancePassErrorV1, GovernancePassItemV1,
        GovernancePassSegmentV1, SharedGovernancePassV1,
    },
    schedule::AnonymousDueItemV1,
    stage_for_body,
};
use crate::state::{
    StateBlock, StateReadOnly, StateStorageAdmissionError, StateTransaction, WorldReadOnly,
    parliament_anonymous_tables::{election_credentials, eligible_citizens},
};

/// Why Parliament's items of a pass did not complete. The block is abandoned in every case.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AnonymousPassWorldErrorV1 {
    /// World refused a transaction: a local storage refusal.
    #[error("World storage refused a Parliament item: {0}")]
    Storage(StateStorageAdmissionError),
    /// A committed parameter failed decoding or validation.
    #[error("committed governance policy: {0}")]
    Policy(&'static str),
    /// The reducer or the board refused an item that is due and live.
    #[error("{0}")]
    Reducer(Box<Error>),
    /// The three tables disagree with one another. State that restore accepted never
    /// produces it.
    #[error("the anonymous Parliament tables disagree: {0}")]
    World(&'static str),
    /// The governance-start partition refused rows whose capacity the owner had checked.
    #[error("governance-start partition: {0}")]
    Output(GovernanceStartOutputBudgetErrorV1),
    /// A receipt digest could not be framed with the node's pinned codec.
    #[error("receipt digest: {0}")]
    Receipt(AnonymousBallotCodecErrorV1),
}

impl AnonymousPassWorldErrorV1 {
    /// Whether the node could not decide: a storage refusal, or a proof profile, pinned
    /// frame or verifier backend the reducer could not use. The block is deferred.
    pub(crate) fn is_local(&self) -> bool {
        match self {
            Self::Storage(_) | Self::Receipt(_) => true,
            Self::Reducer(error) => error.is_local(),
            Self::Output(error) => *error == GovernanceStartOutputBudgetErrorV1::Allocation,
            Self::Policy(_) | Self::World(_) => false,
        }
    }
}

impl From<Error> for AnonymousPassWorldErrorV1 {
    fn from(error: Error) -> Self {
        Self::Reducer(Box::new(error))
    }
}

use super::AnonymousParliamentAttemptStateV1 as Attempt;
use AnonymousPassWorldErrorV1 as PassError;

/// What one block-start pass did.
#[derive(Debug, Default)]
pub(crate) struct AnonymousPassReportV1 {
    /// The governance-start partition of the block with the rows the pass emitted, in
    /// emission order: the prelude's casting clocks, then each positioned Parliament item's
    /// receipt and ballot output in processing order.
    pub(crate) rows: GovernanceStartOutputBudgetV1,
    /// Casting clocks the prelude established.
    pub(crate) clocks: u32,
    /// G1–G3 items the pass processed, the other owner's included.
    pub(crate) processed: u32,
}

impl AnonymousPassReportV1 {
    /// `governance_due_applied` of `specs/sccp.md` §4.7.2: the prelude established a casting
    /// clock or at least one positioned item was processed.
    pub(crate) const fn governance_due_applied(&self) -> bool {
        self.clocks != 0 || self.processed != 0
    }

    /// The committed outputs with (`true`) or without the receipts of the positioned items,
    /// in emission order.
    #[cfg(test)]
    fn emitted(&self, receipts: bool) -> Vec<GovernanceBlockStartOutputV1> {
        self.rows
            .rows()
            .iter()
            .filter_map(|row| row.governance_block_start())
            .filter(|output| output.event.is_receipt() == receipts)
            .cloned()
            .collect()
    }

    /// The prelude's casting clocks, then the ballot outputs of the positioned items.
    #[cfg(test)]
    pub(crate) fn outputs(&self) -> Vec<GovernanceBlockStartOutputV1> {
        self.emitted(false)
    }

    /// The receipts of the positioned items, in processing order.
    #[cfg(test)]
    pub(crate) fn receipts(&self) -> Vec<GovernanceBlockStartOutputV1> {
        self.emitted(true)
    }
}

/// Another owner's items of the same pass: SCCP's rank-three items of G2a and its pause
/// enactments of G3 (`specs/sccp.md` §4.7.2). The driver lists them into the one cursor, so
/// they share its budget and positions.
pub(crate) trait ForeignPassOwnerV1 {
    /// The owner's refusal; the block is abandoned.
    type Error;

    /// Whether the owner has due work on the State the block started on. When neither it nor
    /// Parliament has any, no cursor is begun.
    fn has_due_work(&self, state: &StateBlock<'_>) -> bool;

    /// Its G2a occurrences due in this block on the State the earlier segments left, in
    /// order, at most `limit`: more cannot be reached by the budget.
    ///
    /// # Errors
    /// The owner's refusal.
    fn g2a_items(
        &mut self,
        state: &StateBlock<'_>,
        limit: usize,
    ) -> Result<Vec<GovernancePassItemV1>, Self::Error>;

    /// Its G3 items, after every Parliament enactment.
    ///
    /// # Errors
    /// The owner's refusal.
    fn g3_items(
        &mut self,
        state: &StateBlock<'_>,
    ) -> Result<Vec<GovernancePassItemV1>, Self::Error>;

    /// Whether the occurrence of `item` still exists, immediately before it is charged.
    ///
    /// # Errors
    /// The owner's refusal; an item of another owner is one.
    fn is_live(
        &self,
        state: &StateBlock<'_>,
        item: GovernancePassItemV1,
    ) -> Result<bool, Self::Error>;

    /// Apply the live `item` at its position `at`, atomically.
    ///
    /// # Errors
    /// The owner's refusal; an item of another owner is one.
    fn apply(
        &mut self,
        state: &mut StateBlock<'_>,
        item: GovernancePassItemV1,
        at: GovernanceAnchorV1,
    ) -> Result<(), Self::Error>;
}

/// No other owner: Parliament's items alone.
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoForeignOwnerV1;

/// The refusal of an item no owner listed.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnownedItemV1;

#[cfg(test)]
impl ForeignPassOwnerV1 for NoForeignOwnerV1 {
    type Error = UnownedItemV1;

    fn has_due_work(&self, _: &StateBlock<'_>) -> bool {
        false
    }

    fn g2a_items(
        &mut self,
        _: &StateBlock<'_>,
        _: usize,
    ) -> Result<Vec<GovernancePassItemV1>, UnownedItemV1> {
        Ok(Vec::new())
    }

    fn g3_items(&mut self, _: &StateBlock<'_>) -> Result<Vec<GovernancePassItemV1>, UnownedItemV1> {
        Ok(Vec::new())
    }

    fn is_live(&self, _: &StateBlock<'_>, _: GovernancePassItemV1) -> Result<bool, UnownedItemV1> {
        Err(UnownedItemV1)
    }

    fn apply(
        &mut self,
        _: &mut StateBlock<'_>,
        _: GovernancePassItemV1,
        _: GovernanceAnchorV1,
    ) -> Result<(), UnownedItemV1> {
        Err(UnownedItemV1)
    }
}

/// The owner whose refusal ended a pass.
#[derive(Debug)]
pub(crate) enum GovernanceOwnerErrorV1<E> {
    /// Parliament refused one of its items.
    Parliament(AnonymousPassWorldErrorV1),
    /// The other owner refused one of its items, or the step before the enactments failed.
    Foreign(E),
}

/// `K`, the casting windows that may be open at once: `max_nonterminal_attempts` of the
/// committed `gov.parliament_ballot` parameter, or of its default while none is committed.
///
/// # Errors
/// [`PassError::Policy`] for a committed value that does not decode or is out of range.
pub(crate) fn casting_window_limit(
    world: &(impl WorldReadOnly + ?Sized),
) -> Result<usize, PassError> {
    let ballot = world
        .parameters()
        .custom()
        .get(ParliamentBallotParametersV1::parameter_id())
        .map(ParliamentBallotParametersV1::from_custom_parameter)
        .transpose()
        .map_err(|_| PassError::Policy("gov.parliament_ballot does not decode"))?
        .unwrap_or_default();
    ballot
        .validate_structure()
        .map_err(|_| PassError::Policy("gov.parliament_ballot is out of range"))?;
    Ok(usize::from(ballot.max_nonterminal_attempts))
}

/// Casting windows that count as open: opened, with no closure processed.
fn open_casting_windows(
    boards: &impl StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
) -> usize {
    boards
        .iter()
        .filter(|(_, board)| board.opening.is_some() && board.closure.is_none())
        .count()
}

/// The board World keeps for the sealed `instance` of `attempt`, with its scope.
fn stored_board(
    boards: &impl StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
    attempt: GovernanceAttemptId,
    instance: BodyInstanceId,
) -> Option<(BallotScopeV1, ParliamentBallotBoardRecordV1)> {
    boards
        .iter()
        .find(|(_, board)| {
            board.context.body_context.governance_attempt_id == attempt
                && board.body_instance_id == instance
        })
        .map(|(scope, board)| (*scope, board.clone()))
}

/// The board of the sealed jury `body` of the attempt `id`, as World keeps it.
fn jury_board(
    tx: &StateTransaction<'_, '_>,
    id: GovernanceAttemptId,
    body: ParliamentBody,
) -> Result<(BallotScopeV1, ParliamentBallotBoardRecordV1), PassError> {
    let instance = tx
        .world
        .parliament_anonymous_attempts
        .get(&id)
        .ok_or(PassError::World("a ballot item names no stored attempt"))?
        .sealed_body(body)
        .ok_or(PassError::World("a ballot item names an unsealed jury"))?
        .body_instance_id;
    stored_board(&tx.world.parliament_anonymous_boards, id, instance).ok_or(PassError::World(
        "a sealed binding jury has no stored board",
    ))
}

/// The typed board of a stored record, recomputed and never trusted.
fn typed_board(record: ParliamentBallotBoardRecordV1) -> Result<BallotBoardV1, PassError> {
    BallotBoardV1::restore(record).map_err(|error| Error::Board(Box::new(error)).into())
}

/// The prelude `EstablishCastingClock`: every ballot opened in the parent block takes its
/// clock from this block, in `(governance_attempt_id, body)` order. Mandatory, unpositioned
/// and outside the budget; its rows use the slots the partition reserves for it.
fn establish_casting_clocks(
    state: &mut StateBlock<'_>,
    report: &mut AnonymousPassReportV1,
) -> Result<(), PassError> {
    let header = state._curr_block;
    let (height, timestamp_ms) = (header.height().get(), header.creation_time_ms);
    let mut due = Vec::new();
    for (scope, board) in state.world.parliament_anonymous_boards.iter() {
        let Some(opening) = board.opening else {
            continue;
        };
        if board.casting_clock.is_some() {
            continue;
        }
        if opening.height.checked_add(1) != Some(height) {
            return Err(PassError::World(
                "an opened ballot missed its successor-block casting clock",
            ));
        }
        let context = board.context.body_context;
        due.push((context.governance_attempt_id, context.body_role, *scope));
    }
    if due.is_empty() {
        return Ok(());
    }
    due.sort_unstable();
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let network_id = *tx.network_id();
    for (id, _, scope) in due {
        let (record, established) = {
            let record = tx
                .world
                .parliament_anonymous_boards
                .get(&scope)
                .cloned()
                .ok_or(PassError::World("a listed board left World"))?;
            let attempt = tx
                .world
                .parliament_anonymous_attempts
                .get(&id)
                .ok_or(PassError::World("a board belongs to no stored attempt"))?;
            let mut board = typed_board(record)?;
            let established = attempt.establish_ballot_board_clock(
                &mut board,
                &network_id,
                height,
                timestamp_ms,
            )?;
            (board.record(), established)
        };
        tx.world.parliament_anonymous_boards.insert(scope, record);
        report
            .rows
            .admit_prelude(GovernanceBlockStartOutputV1 {
                anchor: GovernanceOutputAnchorV1::CastingClockPrelude {
                    height,
                    timestamp_ms,
                },
                event: GovernanceBlockStartEventV1::CastingClockEstablished(established),
            })
            .map_err(PassError::Output)?;
        report.clocks = report
            .clocks
            .checked_add(1)
            .ok_or(PassError::World("casting clocks exceed the count domain"))?;
    }
    tx.apply();
    Ok(())
}

/// G1: the generations of active attempts awaiting a slot at or below `height` whose
/// exact-slot pulse is available.
fn draw_items(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, Attempt>,
    height: u64,
    pulse_available: &impl Fn(u64) -> bool,
) -> Vec<GovernancePassItemV1> {
    let mut items = Vec::new();
    for (id, attempt) in attempts.iter() {
        for slot in attempt.awaiting_slots() {
            if slot <= height && pulse_available(slot) {
                items.push(GovernancePassItemV1::Draw { slot, attempt: *id });
            }
        }
    }
    items
}

/// G2a ranks 0 to 2: every attempt's lifecycle items due at `now_ms`.
fn lifecycle_items(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, Attempt>,
    now_ms: u64,
) -> Result<Vec<GovernancePassItemV1>, PassError> {
    let mut items = Vec::new();
    for (_, attempt) in attempts.iter() {
        items.extend(
            attempt
                .due_lifecycle_items(now_ms)?
                .into_iter()
                .map(GovernancePassItemV1::Lifecycle),
        );
    }
    Ok(items)
}

/// G2b: every clocked ballot whose closing time `now_ms` has reached.
fn closure_items(
    boards: &impl StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
    now_ms: u64,
) -> Vec<GovernancePassItemV1> {
    boards
        .iter()
        .filter(|(_, board)| board.closure.is_none())
        .filter_map(|(_, board)| {
            let due_ms = board
                .casting_clock?
                .timestamp_ms
                .checked_add(board.context.policy.binding.casting_window_ms)?;
            let context = board.context.body_context;
            (due_ms <= now_ms).then_some(GovernancePassItemV1::Close {
                due_ms,
                attempt: context.governance_attempt_id,
                body: context.body_role,
            })
        })
        .collect()
}

/// Whether the attempt `id` is active at the stage of `body`: only then may its ballot open.
fn at_jury_stage(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, Attempt>,
    id: GovernanceAttemptId,
    body: ParliamentBody,
) -> Result<bool, PassError> {
    let attempt = attempts
        .get(&id)
        .ok_or(PassError::World("a board belongs to no stored attempt"))?
        .attempt();
    Ok(
        attempt.status == GovernanceAttemptStatusV1::Active
            && attempt.stage == stage_for_body(body),
    )
}

/// G2c: every scheduled, unopened ballot whose opening time `now_ms` has reached and whose
/// attempt is still at its jury. The board of an attempt that ended before its ballot opened
/// never opens.
fn opening_items(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, Attempt>,
    boards: &impl StorageReadOnly<BallotScopeV1, ParliamentBallotBoardRecordV1>,
    now_ms: u64,
) -> Result<Vec<GovernancePassItemV1>, PassError> {
    let mut items = Vec::new();
    for (_, board) in boards.iter() {
        let Some(due_ms) = board.opening_at_ms else {
            continue;
        };
        if board.opening.is_some() || board.closure.is_some() || due_ms > now_ms {
            continue;
        }
        let context = board.context.body_context;
        if at_jury_stage(attempts, context.governance_attempt_id, context.body_role)? {
            items.push(GovernancePassItemV1::Open {
                due_ms,
                attempt: context.governance_attempt_id,
                body: context.body_role,
            });
        }
    }
    Ok(items)
}

/// The height-progress ticket of the block at `height` and time `now_ms` (design §9): the
/// step to the nearest slot an active attempt still awaits at or above `height`, due once
/// the parent's time plus the committed block cadence is reached. It is derived from the
/// parent and never stored; the block of the slot itself consumes the last one, so a slot
/// that passed without its pulse produces none.
fn height_progress_ticket(
    attempts: &impl StorageReadOnly<GovernanceAttemptId, Attempt>,
    height: u64,
    now_ms: u64,
    parent_time_ms: Option<u64>,
    block_cadence_ms: u64,
) -> Option<AdvanceToNativeSlotV1> {
    let parent_height = height.checked_sub(1)?;
    let due_ms = parent_time_ms?.checked_add(block_cadence_ms)?;
    // SPEC: §13.4 HC802 — a ticket is consumed only once its cadence deadline is reached.
    if due_ms > now_ms && !cfg!(all(test, sumeragi_core_mutation = "HC802")) {
        return None;
    }
    let target = attempts
        .iter()
        .flat_map(|(_, attempt)| attempt.awaiting_slots())
        .filter(|slot| *slot > parent_height)
        .min()?;
    AdvanceToNativeSlotV1::derive_v1(parent_height, target)
}

/// The rows `item` may emit, known before it mutates anything: every Parliament item emits
/// its receipt, and a binding jury's seal, a closure and an opening one ballot output more.
fn output_ceiling(item: GovernancePassItemV1) -> GovernanceStartItemCeilingV1 {
    match item {
        GovernancePassItemV1::Lifecycle(AnonymousDueItemV1 {
            item: AnonymousLifecycleItemV1::CloseInvitations { body, .. },
            ..
        }) if is_binding(body) => GovernanceStartItemCeilingV1::ReceiptAndOutput,
        GovernancePassItemV1::Close { .. } | GovernancePassItemV1::Open { .. } => {
            GovernanceStartItemCeilingV1::ReceiptAndOutput
        }
        _ => GovernanceStartItemCeilingV1::Receipt,
    }
}

/// The receipt of a positioned Parliament `item` of `attempt` at `at`: the attempt's status,
/// stage and State digest as the item left them, and the board it stored or changed.
fn receipt(
    at: GovernanceAnchorV1,
    item: GovernanceLifecycleItemV1,
    attempt: &Attempt,
    board: Option<(&BallotScopeV1, &ParliamentBallotBoardRecordV1)>,
) -> Result<GovernanceBlockStartOutputV1, PassError> {
    let board = board
        .map(|(scope, record)| {
            board_record_digest_v1(record).map(|record_digest| LifecycleBoardDigestV1 {
                scope: *scope,
                record_digest,
            })
        })
        .transpose()
        .map_err(PassError::Receipt)?;
    Ok(GovernanceBlockStartOutputV1 {
        anchor: GovernanceOutputAnchorV1::Pass(at),
        event: GovernanceBlockStartEventV1::LifecycleApplied(LifecycleAppliedV1 {
            governance_attempt_id: attempt.attempt().id,
            item,
            status: attempt.attempt().status,
            stage: attempt.attempt().stage,
            attempt_state_digest: attempt_state_digest_v1(attempt).map_err(PassError::Receipt)?,
            board,
        }),
    })
}

/// The committed name of a lifecycle item of G2a.
const fn lifecycle_receipt_item(item: AnonymousLifecycleItemV1) -> GovernanceLifecycleItemV1 {
    match item {
        AnonymousLifecycleItemV1::StartQualification => {
            GovernanceLifecycleItemV1::StartQualification
        }
        AnonymousLifecycleItemV1::EndQualification => GovernanceLifecycleItemV1::EndQualification,
        AnonymousLifecycleItemV1::AdvancePhase { body, phase } => {
            GovernanceLifecycleItemV1::AdvancePhase { body, phase }
        }
        AnonymousLifecycleItemV1::RejectExhausted { slot } => {
            GovernanceLifecycleItemV1::RejectExhausted { slot }
        }
        AnonymousLifecycleItemV1::CloseInvitations { body, election } => {
            GovernanceLifecycleItemV1::CloseInvitations { body, election }
        }
        AnonymousLifecycleItemV1::ConcludeFinding { body, instance } => {
            GovernanceLifecycleItemV1::ConcludeFinding { body, instance }
        }
    }
}

/// The owner's observation of `item` immediately before it is charged. A live item whose
/// rows do not fit the governance-start partition `rows` stays due.
fn inspect(
    state: &StateBlock<'_>,
    item: GovernancePassItemV1,
    casting_limit: usize,
    rows: &GovernanceStartOutputBudgetV1,
) -> Result<GovernanceOccurrenceV1, PassError> {
    let occurrence = occurrence(state, item, casting_limit)?;
    // SPEC: §13.4 HC801 — an item is charged only if its rows fit the reserved partition.
    let fits = rows.fits(output_ceiling(item)) || cfg!(all(test, sumeragi_core_mutation = "HC801"));
    Ok(if occurrence == GovernanceOccurrenceV1::Live && !fits {
        GovernanceOccurrenceV1::NoOutputCapacity
    } else {
        occurrence
    })
}

/// Whether the occurrence of `item` still exists and, for an opening, has a casting slot.
fn occurrence(
    state: &StateBlock<'_>,
    item: GovernancePassItemV1,
    casting_limit: usize,
) -> Result<GovernanceOccurrenceV1, PassError> {
    let attempts = &*state.world.parliament_anonymous_attempts;
    let stored = |id: GovernanceAttemptId| {
        attempts
            .get(&id)
            .ok_or(PassError::World("an item names no stored attempt"))
    };
    let live = |live: bool| {
        if live {
            GovernanceOccurrenceV1::Live
        } else {
            GovernanceOccurrenceV1::Lapsed
        }
    };
    match item {
        GovernancePassItemV1::Draw { slot, attempt } => {
            Ok(live(stored(attempt)?.awaiting_slots().contains(&slot)))
        }
        GovernancePassItemV1::Lifecycle(due) => {
            let attempt = stored(due.governance_attempt_id)?;
            Ok(live(
                attempt.attempt().status == GovernanceAttemptStatusV1::Active
                    && attempt.check_occurrence(due.item).is_ok(),
            ))
        }
        GovernancePassItemV1::Close { attempt, .. } => {
            // Nothing ends an attempt while its jury casts: the closure decides it.
            if stored(attempt)?.attempt().status == GovernanceAttemptStatusV1::Active {
                Ok(GovernanceOccurrenceV1::Live)
            } else {
                Err(PassError::World("an open ballot outlived its attempt"))
            }
        }
        GovernancePassItemV1::Open { attempt, body, .. } => {
            if !at_jury_stage(attempts, attempt, body)? {
                Ok(GovernanceOccurrenceV1::Lapsed)
            } else if open_casting_windows(&*state.world.parliament_anonymous_boards)
                >= casting_limit
            {
                Ok(GovernanceOccurrenceV1::NoCastingSlot)
            } else {
                Ok(GovernanceOccurrenceV1::Live)
            }
        }
        // The ticket is derived for this block alone: nothing can end it within the pass.
        GovernancePassItemV1::HeightProgress => Ok(GovernanceOccurrenceV1::Live),
        GovernancePassItemV1::PauseConclusion { .. }
        | GovernancePassItemV1::Reconciliation { .. }
        | GovernancePassItemV1::EnactParliament { .. }
        | GovernancePassItemV1::EnactPause { .. } => Err(PassError::World(
            "an item of another owner reached Parliament",
        )),
    }
}

/// The attempt `id` as World keeps it, to be changed and stored again.
fn stored_attempt(
    tx: &StateTransaction<'_, '_>,
    id: GovernanceAttemptId,
) -> Result<Attempt, PassError> {
    tx.world
        .parliament_anonymous_attempts
        .get(&id)
        .cloned()
        .ok_or(PassError::World("an item names no stored attempt"))
}

/// G1 height progress: the ticket of this block is consumed. The cursor is the height
/// itself, so the receipt is the whole effect.
fn apply_height_progress(
    ticket: Option<AdvanceToNativeSlotV1>,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    let ticket = ticket.ok_or(PassError::World(
        "a height-progress item has no ticket in its block",
    ))?;
    rows.admit_item(
        GovernanceStartItemCeilingV1::Receipt,
        vec![GovernanceBlockStartOutputV1 {
            anchor: GovernanceOutputAnchorV1::Pass(at),
            event: GovernanceBlockStartEventV1::NativeSlotAdvanced(NativeSlotAdvancedV1::of(
                ticket,
            )),
        }],
    )
    .map_err(PassError::Output)
}

/// G1 `DrawGeneration`.
fn apply_draw(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    pulse: AnonymousSortitionPulseV1,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let network_id = *tx.network_id();
    let mut attempt = stored_attempt(&tx, id)?;
    attempt.draw_generation(&network_id, pulse, at)?;
    let item = GovernanceLifecycleItemV1::DrawGeneration { slot: pulse.height };
    rows.admit_item(
        GovernanceStartItemCeilingV1::Receipt,
        vec![receipt(at, item, &attempt, None)?],
    )
    .map_err(PassError::Output)?;
    tx.world.parliament_anonymous_attempts.insert(id, attempt);
    tx.apply();
    Ok(())
}

/// One G2a lifecycle item. A binding jury's seal stores its board and publishes its list;
/// a binding jury's Reflection entry fixes its board's opening time before G2c is listed.
fn apply_lifecycle(
    state: &mut StateBlock<'_>,
    due: AnonymousDueItemV1,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    let id = due.governance_attempt_id;
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let network_id = *tx.network_id();
    let mut attempt = stored_attempt(&tx, id)?;
    let effect =
        attempt.apply_item_with_credentials(&network_id, due.item, at, |_, accepted| {
            election_credentials(&tx.world.parliament_anonymous_registrations, accepted)
        })?;
    // The board the item stored or changed, and the list a seal publishes.
    let (mut board, published) = match effect.board {
        Some(sealed) => {
            let sealed = *sealed;
            let scope = *typed_board(sealed.board.clone())?.scope();
            if tx.world.parliament_anonymous_boards.get(&scope).is_some() {
                return Err(PassError::World("a seal's scope already holds a board"));
            }
            (
                Some((scope, sealed.board)),
                Some(GovernanceBlockStartOutputV1 {
                    anchor: GovernanceOutputAnchorV1::Pass(at),
                    event: GovernanceBlockStartEventV1::SealedCredentialListPublished(
                        sealed.published,
                    ),
                }),
            )
        }
        None => (None, None),
    };
    if let AnonymousLifecycleItemV1::AdvancePhase { body, .. } = due.item
        && is_binding(body)
        && attempt.opening_at_ms(body).is_some()
    {
        let instance = attempt
            .sealed_body(body)
            .ok_or(PassError::World("a jury in Reflection is not sealed"))?
            .body_instance_id;
        let (scope, record) = stored_board(&tx.world.parliament_anonymous_boards, id, instance)
            .ok_or(PassError::World(
                "a sealed binding jury has no stored board",
            ))?;
        if record.opening_at_ms.is_none() {
            let mut scheduled = typed_board(record)?;
            attempt.schedule_ballot_board(&mut scheduled, &network_id)?;
            board = Some((scope, scheduled.record()));
        }
    }
    let mut outputs = vec![receipt(
        at,
        lifecycle_receipt_item(due.item),
        &attempt,
        board.as_ref().map(|(scope, record)| (scope, record)),
    )?];
    outputs.extend(published);
    rows.admit_item(
        output_ceiling(GovernancePassItemV1::Lifecycle(due)),
        outputs,
    )
    .map_err(PassError::Output)?;
    if let Some((scope, record)) = board {
        tx.world.parliament_anonymous_boards.insert(scope, record);
    }
    tx.world.parliament_anonymous_attempts.insert(id, attempt);
    tx.apply();
    Ok(())
}

/// G2b `CloseBallot`: the board's closure record decides its attempt in the same step.
fn apply_close(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    body: ParliamentBody,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let network_id = *tx.network_id();
    let (scope, record) = jury_board(&tx, id, body)?;
    let mut board = typed_board(record)?;
    let mut attempt = stored_attempt(&tx, id)?;
    let minimum_bond = attempt
        .policy()
        .sortition_policy
        .minimum_citizenship_bond
        .clone();
    let (_, closure) = attempt.close_ballot_board(&mut board, &network_id, at, || {
        eligible_citizens(&tx.world.citizens, &minimum_bond)
    })?;
    let record = board.record();
    rows.admit_item(
        GovernanceStartItemCeilingV1::ReceiptAndOutput,
        vec![
            receipt(
                at,
                GovernanceLifecycleItemV1::CloseBallot { body },
                &attempt,
                Some((&scope, &record)),
            )?,
            GovernanceBlockStartOutputV1 {
                anchor: GovernanceOutputAnchorV1::Pass(at),
                event: GovernanceBlockStartEventV1::BallotClosed(closure),
            },
        ],
    )
    .map_err(PassError::Output)?;
    tx.world.parliament_anonymous_boards.insert(scope, record);
    tx.world.parliament_anonymous_attempts.insert(id, attempt);
    tx.apply();
    Ok(())
}

/// G2c `OpenBallot`: casting opens at `s0 = at`.
fn apply_open(
    state: &mut StateBlock<'_>,
    id: GovernanceAttemptId,
    body: ParliamentBody,
    at: GovernanceAnchorV1,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    let mut tx = state.try_transaction().map_err(PassError::Storage)?;
    let network_id = *tx.network_id();
    let (scope, record) = jury_board(&tx, id, body)?;
    let record = {
        let attempt = tx
            .world
            .parliament_anonymous_attempts
            .get(&id)
            .ok_or(PassError::World("an item names no stored attempt"))?;
        let mut board = typed_board(record)?;
        let opened = attempt.open_ballot_board(&mut board, &network_id, at)?;
        let record = board.record();
        rows.admit_item(
            GovernanceStartItemCeilingV1::ReceiptAndOutput,
            vec![
                receipt(
                    at,
                    GovernanceLifecycleItemV1::OpenBallot { body },
                    attempt,
                    Some((&scope, &record)),
                )?,
                GovernanceBlockStartOutputV1 {
                    anchor: GovernanceOutputAnchorV1::Pass(at),
                    event: GovernanceBlockStartEventV1::CastingOpened(opened),
                },
            ],
        )
        .map_err(PassError::Output)?;
        record
    };
    tx.world.parliament_anonymous_boards.insert(scope, record);
    tx.apply();
    Ok(())
}

/// Apply the live Parliament `item` at its position `at`, atomically: its rows enter `rows`
/// before its World transaction is applied.
fn apply(
    state: &mut StateBlock<'_>,
    item: GovernancePassItemV1,
    at: GovernanceAnchorV1,
    pulse_at: &impl Fn(u64) -> Option<AnonymousSortitionPulseV1>,
    ticket: Option<AdvanceToNativeSlotV1>,
    rows: &mut GovernanceStartOutputBudgetV1,
) -> Result<(), PassError> {
    match item {
        GovernancePassItemV1::HeightProgress => apply_height_progress(ticket, at, rows),
        GovernancePassItemV1::Draw { slot, attempt } => {
            let pulse = pulse_at(slot).ok_or(PassError::World("a listed pulse left its source"))?;
            apply_draw(state, attempt, pulse, at, rows)
        }
        GovernancePassItemV1::Lifecycle(due) => apply_lifecycle(state, due, at, rows),
        GovernancePassItemV1::Close { attempt, body, .. } => {
            apply_close(state, attempt, body, at, rows)
        }
        GovernancePassItemV1::Open { attempt, body, .. } => {
            apply_open(state, attempt, body, at, rows)
        }
        GovernancePassItemV1::PauseConclusion { .. }
        | GovernancePassItemV1::Reconciliation { .. }
        | GovernancePassItemV1::EnactParliament { .. }
        | GovernancePassItemV1::EnactPause { .. } => Err(PassError::World(
            "an item of another owner reached Parliament",
        )),
    }
}

/// Whether `item` belongs to the other owner of the pass.
const fn is_foreign(item: GovernancePassItemV1) -> bool {
    matches!(
        item,
        GovernancePassItemV1::PauseConclusion { .. }
            | GovernancePassItemV1::Reconciliation { .. }
            | GovernancePassItemV1::EnactPause { .. }
    )
}

/// What Parliament needs of a block to run its items.
pub(crate) struct ParliamentPassInputsV1<P> {
    /// The authenticated native pulse of an exact slot, when this block carries it or an
    /// earlier block committed it; a slot without its pulse is not due.
    pub(crate) pulse_at: P,
    /// The time of the parent block, which the height-progress ticket is due from; `None`
    /// on a block without a parent, which has no ticket.
    pub(crate) parent_time_ms: Option<u64>,
}

/// The inputs of a root that does not install the family: `None::<NoParliamentV1>`.
#[cfg(test)]
pub(crate) type NoParliamentV1 =
    ParliamentPassInputsV1<fn(u64) -> Option<AnonymousSortitionPulseV1>>;

/// Run the block-start pass of this block: the casting-clock prelude, then G1 to G3 under
/// the shared budget and positions.
///
/// `parliament` is `Some` exactly on a root whose selection installs the anonymous family;
/// without it no Parliament item exists and the partition stays empty. `foreign` supplies
/// the other owner's rank-three items of G2a and its enactments of G3, and
/// `before_enactments` runs between G2c and G3 on the same block.
///
/// # Errors
/// [`GovernancePassErrorV1`] with the refusal of the owner of the refused item. The block
/// must be abandoned, items already applied included.
pub(crate) fn run_governance_pass<F: ForeignPassOwnerV1>(
    state: &mut StateBlock<'_>,
    parliament: Option<ParliamentPassInputsV1<impl Fn(u64) -> Option<AnonymousSortitionPulseV1>>>,
    foreign: &mut F,
    before_enactments: impl FnOnce(&mut StateBlock<'_>) -> Result<(), F::Error>,
) -> Result<AnonymousPassReportV1, GovernancePassErrorV1<GovernanceOwnerErrorV1<F::Error>>> {
    use GovernancePassSegmentV1 as Segment;

    let parliament_error =
        |error| GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Parliament(error));
    let foreign_error =
        |error| GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Foreign(error));
    let header = state._curr_block;
    let (height, now_ms) = (header.height().get(), header.creation_time_ms);
    let mut report = AnonymousPassReportV1::default();
    if parliament.is_some() {
        establish_casting_clocks(state, &mut report).map_err(parliament_error)?;
    }
    report.rows.close_prelude();
    let parliament_work = parliament.is_some()
        && (state
            .world
            .parliament_anonymous_attempts
            .iter()
            .next()
            .is_some()
            || state
                .world
                .parliament_anonymous_boards
                .iter()
                .next()
                .is_some());
    if !parliament_work && !foreign.has_due_work(state) {
        // Nothing can be due: no cursor, and the clock policy is not read.
        before_enactments(state).map_err(foreign_error)?;
        return Ok(report);
    }
    let casting_limit = if parliament.is_some() {
        casting_window_limit(&state.world).map_err(parliament_error)?
    } else {
        0
    };
    let mut pass = SharedGovernancePassV1::begin(state)?;
    // No owner's item costs less than one unit: more than the budget plus the
    // unconditional first item cannot be reached.
    let limit = usize::try_from(pass.remaining())
        .unwrap_or(usize::MAX)
        .saturating_add(1);
    let ticket = parliament.as_ref().and_then(|inputs| {
        height_progress_ticket(
            &*state.world.parliament_anonymous_attempts,
            height,
            now_ms,
            inputs.parent_time_ms,
            state.world.parameters().sumeragi().block_cadence_ms.get(),
        )
    });
    let pulse_at = |slot: u64| -> Option<AnonymousSortitionPulseV1> {
        parliament
            .as_ref()
            .and_then(|inputs| (inputs.pulse_at)(slot))
    };
    let rows = RefCell::new(core::mem::replace(
        &mut report.rows,
        GovernanceStartOutputBudgetV1::new(),
    ));
    let foreign = RefCell::new(foreign);
    let mut before_enactments = Some(before_enactments);
    for segment in [
        Segment::G1,
        Segment::G2a,
        Segment::G2b,
        Segment::G2c,
        Segment::G3,
    ] {
        if segment == Segment::G3
            && let Some(before_enactments) = before_enactments.take()
        {
            before_enactments(state).map_err(foreign_error)?;
        }
        let mut items = if parliament.is_some() {
            let attempts = &*state.world.parliament_anonymous_attempts;
            let boards = &*state.world.parliament_anonymous_boards;
            match segment {
                Segment::G1 => {
                    let mut items = draw_items(attempts, height, &|slot| pulse_at(slot).is_some());
                    items.extend(ticket.map(|_| GovernancePassItemV1::HeightProgress));
                    items
                }
                Segment::G2a => lifecycle_items(attempts, now_ms).map_err(parliament_error)?,
                Segment::G2b => closure_items(boards, now_ms),
                Segment::G2c => {
                    opening_items(attempts, boards, now_ms).map_err(parliament_error)?
                }
                Segment::G3 => Vec::new(),
            }
        } else {
            Vec::new()
        };
        match segment {
            Segment::G2a => items.extend(
                foreign
                    .borrow_mut()
                    .g2a_items(state, limit)
                    .map_err(foreign_error)?,
            ),
            Segment::G3 => {
                items.extend(
                    foreign
                        .borrow_mut()
                        .g3_items(state)
                        .map_err(foreign_error)?,
                );
            }
            Segment::G1 | Segment::G2b | Segment::G2c => (),
        }
        pass.run_segment(
            state,
            segment,
            items,
            |state, item| {
                if is_foreign(item) {
                    foreign
                        .borrow()
                        .is_live(state, item)
                        .map(|live| {
                            if live {
                                GovernanceOccurrenceV1::Live
                            } else {
                                GovernanceOccurrenceV1::Lapsed
                            }
                        })
                        .map_err(GovernanceOwnerErrorV1::Foreign)
                } else {
                    inspect(state, item, casting_limit, &rows.borrow())
                        .map_err(GovernanceOwnerErrorV1::Parliament)
                }
            },
            |state, item, at| {
                if is_foreign(item) {
                    foreign
                        .borrow_mut()
                        .apply(state, item, at)
                        .map_err(GovernanceOwnerErrorV1::Foreign)
                } else {
                    apply(state, item, at, &pulse_at, ticket, &mut rows.borrow_mut())
                        .map_err(GovernanceOwnerErrorV1::Parliament)
                }
            },
        )?;
    }
    report.rows = rows.into_inner();
    report.processed = pass.processed();
    Ok(report)
}

/// Run Parliament's items alone on a block whose root installs the family: the
/// casting-clock prelude, then G1 to G2c under the shared budget and positions. `pulse_at`
/// returns the authenticated native pulse of an exact slot when this block carries it or an
/// earlier block committed it, and `parent_time_ms` is the time of the parent block.
///
/// # Errors
/// [`GovernancePassErrorV1`] with the original [`AnonymousPassWorldErrorV1`] of a refused
/// item. The block must be abandoned, items already applied included.
#[cfg(test)]
pub(crate) fn run_parliament_pass(
    state: &mut StateBlock<'_>,
    pulse_at: impl Fn(u64) -> Option<AnonymousSortitionPulseV1>,
    parent_time_ms: Option<u64>,
) -> Result<AnonymousPassReportV1, GovernancePassErrorV1<PassError>> {
    run_governance_pass(
        state,
        Some(ParliamentPassInputsV1 {
            pulse_at,
            parent_time_ms,
        }),
        &mut NoForeignOwnerV1,
        |_| Ok(()),
    )
    .map_err(|error| match error {
        GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Parliament(error)) => {
            GovernancePassErrorV1::Owner(error)
        }
        GovernancePassErrorV1::Owner(GovernanceOwnerErrorV1::Foreign(UnownedItemV1)) => {
            GovernancePassErrorV1::Owner(PassError::World(
                "an item of another owner reached Parliament",
            ))
        }
        GovernancePassErrorV1::Policy(reason) => GovernancePassErrorV1::Policy(reason),
        GovernancePassErrorV1::Source => GovernancePassErrorV1::Source,
        GovernancePassErrorV1::Segment => GovernancePassErrorV1::Segment,
        GovernancePassErrorV1::Items => GovernancePassErrorV1::Items,
        GovernancePassErrorV1::Occurrence => GovernancePassErrorV1::Occurrence,
        GovernancePassErrorV1::Position => GovernancePassErrorV1::Position,
    })
}

/// Parliament's terms of the due predicate `governance_due` (`specs/sccp.md` §4.7.3) for a
/// block at `height` and time `timestamp_ms` on the committed parent `world`, whose block
/// time is `parent_time_ms`: a casting window opened in the parent block awaits its clock, a
/// generation awaits a slot at or below `height` whose pulse is available, the
/// height-progress ticket of `height` is due, a lifecycle item or a ballot closure is due,
/// or a ballot opening is due while fewer than `K` casting windows are open.
///
/// It lists what [`run_governance_pass`] lists, so the pass of a block for which it holds
/// establishes a clock or processes an item, and an idle chain makes no block for an opening
/// that has no slot. `pulse_available` is the caller's view of an exact slot: execution asks
/// whether the block carries the pulse or an earlier block committed it, and admission also
/// counts the slot of the candidate height itself, whose pulse rule K4 requires in the block
/// (`specs/sccp.md` §4.7.4).
///
/// # Errors
/// As [`run_governance_pass`] lists: a deadline that cannot be expressed, tables that
/// disagree, or a committed `K` that does not decode.
pub(crate) fn parliament_due(
    world: &(impl WorldReadOnly + ?Sized),
    height: u64,
    timestamp_ms: u64,
    parent_time_ms: Option<u64>,
    pulse_available: impl Fn(u64) -> bool,
) -> Result<bool, PassError> {
    let attempts = world.parliament_anonymous_attempts();
    let boards = world.parliament_anonymous_boards();
    let clock_due = boards.iter().any(|(_, board)| {
        board.casting_clock.is_none()
            && board
                .opening
                .is_some_and(|opening| opening.height.checked_add(1) == Some(height))
    });
    if clock_due
        || !draw_items(attempts, height, &pulse_available).is_empty()
        || height_progress_ticket(
            attempts,
            height,
            timestamp_ms,
            parent_time_ms,
            world.parameters().sumeragi().block_cadence_ms.get(),
        )
        .is_some()
        || !lifecycle_items(attempts, timestamp_ms)?.is_empty()
        || !closure_items(boards, timestamp_ms).is_empty()
    {
        return Ok(true);
    }
    Ok(!opening_items(attempts, boards, timestamp_ms)?.is_empty()
        && open_casting_windows(boards) < casting_window_limit(world)?)
}

#[cfg(test)]
#[path = "world_pass_tests.rs"]
mod tests;

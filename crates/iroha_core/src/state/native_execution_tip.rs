//! Original native execution identity, outside World to avoid an R self-reference.
//!
//! Decoded snapshot values are claims. Only the original execution owner or a
//! fully verified native prefix can install the opaque value in its funded Cell.

use super::*;
use crate::{
    execution_attempt::ExecutionAttemptError,
    sumeragi::certified_chain::{CertifiedBlock, CertifiedChain, ChainReadError, CommittedBlock},
};
use iroha_allocation::{AllocationBudget, AllocationCharge};
use iroha_sumeragi::types::Hash32;

/// Fixed execution identity retained by the original publication generation.
/// This type deliberately has no decoder or public constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeExecutionTip(NativeExecutionTipRecord);

/// Snapshot claim; decoding this record grants no execution authority.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::json::JsonSerialize, norito::json::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct NativeExecutionTipRecord {
    pub(crate) root_scope: iroha_data_model::block::consensus::SumeragiRootScope,
    pub(crate) height: u64,
    pub(crate) creation_time_ms: u64,
    pub(crate) iroha_hash: HashOf<BlockHeader>,
    pub(crate) core_hash: [u8; 32],
    pub(crate) result: [u8; 32],
}

impl norito::json::JsonSerialize for NativeExecutionTip {
    fn json_serialize(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

impl NativeExecutionTip {
    /// Immutable root scope inherited from the original authenticated execution owner.
    #[must_use]
    pub const fn root_scope(self) -> iroha_data_model::block::consensus::SumeragiRootScope {
        self.0.root_scope
    }

    /// One-based original executed height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.0.height
    }
    /// Ledger time from the exact original authenticated carrier.
    #[must_use]
    pub const fn creation_time_ms(self) -> u64 {
        self.0.creation_time_ms
    }
    /// Exact Iroha header identity whose outputs were sealed.
    #[must_use]
    pub const fn iroha_hash(self) -> HashOf<BlockHeader> {
        self.0.iroha_hash
    }
    /// Exact native header identity admitted by the original owner.
    #[must_use]
    pub const fn core_hash(self) -> Hash32 {
        Hash32(self.0.core_hash)
    }
    /// Exact certified result of the original execution.
    #[must_use]
    pub const fn result(self) -> Hash32 {
        Hash32(self.0.result)
    }
}

/// Complete fixed current/undo snapshot projection, still unauthenticated.
#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct NativeExecutionTipSnapshot {
    revert: Option<NativeExecutionTipUndo>,
    blocks: Option<NativeExecutionTipRecord>,
}

/// Explicit undo wrapper preserves `Some(None)` for the genesis predecessor.
#[derive(norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NativeExecutionTipUndo {
    value: Option<NativeExecutionTipRecord>,
}

pub type TipCell = Cell<Option<NativeExecutionTip>, AllocationCharge>;

/// Reserve actual initial payload/control allocations before moving fixed values.
pub(in crate::state) fn empty_cell(
    budget: &AllocationBudget,
) -> Result<TipCell, MergeLedgerCommitError> {
    initialize(budget, None, None).map_err(Into::into)
}

#[expect(
    clippy::option_option,
    reason = "MV stores an optional native tip with an independent optional undo; unchanged, genesis absence, and a prior tip must remain distinct."
)]
fn initialize(
    budget: &AllocationBudget,
    current: Option<NativeExecutionTip>,
    undo: Option<Option<NativeExecutionTip>>,
) -> Result<TipCell, StateStorageAdmissionError> {
    let original = mv::cell::CellInitialization::try_reserve(budget).map_err(|error| {
        StateStorageAdmissionError::World(match error {
            mv::cell::CellInitializationError::Admission(error) => {
                mv::storage::AdmittedStorageError::Allocation(error)
            }
            mv::cell::CellInitializationError::Allocator { layout } => {
                mv::storage::AdmittedStorageError::Allocator { layout }
            }
        })
    })?;
    Ok(original.initialize(current, undo))
}

/// Resource refusal remains distinct from an invalid snapshot claim.
#[derive(Debug)]
pub(in crate::state) enum TipRestoreError {
    /// Only the original fresh-State genesis execution can authenticate this result.
    GenesisReplayRequired,
    /// Kura holds no certified successor of the claimed tip, so nothing certifies the World
    /// published at that height.
    SuccessorRequired {
        /// Height of the claimed tip.
        height: u64,
    },
    /// Authenticated history contradicts or cannot establish the decoded claim.
    History(String),
    /// A snapshot field checked against the same certified walk contradicts that history.
    Field {
        /// The snapshot field.
        field: &'static str,
        /// How the certified history refutes it.
        message: String,
    },
    /// The exact local pool cannot fund the original Cell.
    Admission(StateStorageAdmissionError),
    /// Original native history resources refused this unfinished local attempt.
    Deferred(crate::execution_attempt::ExecutionDeferred),
}
impl From<ExecutionAttemptError<ChainReadError>> for TipRestoreError {
    fn from(error: ExecutionAttemptError<ChainReadError>) -> Self {
        match error {
            ExecutionAttemptError::Rejected(error) => Self::History(error.to_string()),
            #[cfg(all(test, sumeragi_core_mutation = "HC216"))]
            ExecutionAttemptError::Deferred(original) => Self::History(
                ExecutionAttemptError::<ChainReadError>::Deferred(original).to_string(),
            ),
            #[cfg(not(all(test, sumeragi_core_mutation = "HC216")))]
            ExecutionAttemptError::Deferred(original) => Self::Deferred(original),
        }
    }
}
impl From<String> for TipRestoreError {
    fn from(message: String) -> Self {
        Self::History(message)
    }
}
impl From<&str> for TipRestoreError {
    fn from(message: &str) -> Self {
        Self::History(message.into())
    }
}

impl NativeExecutionTipSnapshot {
    /// Capture the exact original current and undo values without losing nested absence.
    #[expect(
        clippy::option_option,
        reason = "MV stores an optional native tip with an independent optional undo; unchanged, genesis absence, and a prior tip must remain distinct."
    )]
    pub(crate) fn from_original(
        current: Option<NativeExecutionTip>,
        previous: Option<Option<NativeExecutionTip>>,
    ) -> Self {
        Self {
            blocks: current.map(|tip| tip.0),
            revert: previous.map(|tip| NativeExecutionTipUndo {
                value: tip.map(|tip| tip.0),
            }),
        }
    }

    /// Compare serialized claims with the exact originals captured alongside the World.
    /// This grants no restore authority and never constructs an execution owner.
    #[expect(
        clippy::option_option,
        reason = "MV stores an optional native tip with an independent optional undo; unchanged, genesis absence, and a prior tip must remain distinct."
    )]
    pub(crate) fn matches_original(
        &self,
        current: Option<NativeExecutionTip>,
        previous: Option<Option<NativeExecutionTip>>,
    ) -> bool {
        self.blocks == current.map(|tip| tip.0)
            && self.revert.as_ref().map(|undo| undo.value)
                == previous.map(|tip| tip.map(|tip| tip.0))
    }

    /// Restore authority by checking original native history, never by trusting
    /// the decoded Cell or World. H1-only restart must replay signed genesis.
    ///
    /// The certified successor of the tip in Kura extends the same verified walk: its
    /// `parent_world_state_root` is the root of the World published at the tip, which the
    /// caller must match against a cold capture of the decoded World (E51, E56).
    ///
    /// Every certified block from genesis through the tip also passes through `observe`, so
    /// callers that check other snapshot fields against the same history share this walk.
    pub(in crate::state) fn restore(
        self,
        budget: &AllocationBudget,
        chain_id: &iroha_model_base::chain::ChainId,
        network: &iroha_data_model::NetworkId,
        hashes: &[HashOf<BlockHeader>],
        kura: &Kura,
        observe: &mut dyn FnMut(&CertifiedBlock) -> Result<(), TipRestoreError>,
    ) -> Result<RestoredNativeTip, TipRestoreError> {
        let height = u64::try_from(hashes.len()).map_err(|error| error.to_string())?;
        if height == 0 {
            if self.blocks.is_some() || self.revert.is_some() {
                return Err("empty history cannot carry native execution tip or undo".into());
            }
            return Ok(RestoredNativeTip {
                cell: initialize(budget, None, None).map_err(TipRestoreError::Admission)?,
                world_root: None,
            });
        }
        if height == 1 {
            // Strict daemon startup classifies this separately and constructs
            // fresh State before node::prepare replays the stored signed genesis.
            // A decoded H1 result never authorizes World or bypasses original replay.
            return Err(TipRestoreError::GenesisReplayRequired);
        }
        let successor = height + 1;
        let successor_hash = usize::try_from(successor)
            .ok()
            .and_then(core::num::NonZeroUsize::new)
            .and_then(|successor| kura.get_block_hash(successor))
            .ok_or(TipRestoreError::SuccessorRequired { height })?;
        let mut pinned = Vec::with_capacity(hashes.len() + 1);
        pinned.extend_from_slice(hashes);
        pinned.push(successor_hash);
        let chain = CertifiedChain::from_pinned(chain_id, network, &pinned, kura, budget)
            .map_err(TipRestoreError::from)?;
        // One verified walk over the native prefix authenticates the tip and its
        // predecessor and records history checkpoints for off-chain readers.
        let checkpoints = kura.history_checkpoints();
        let mut previous = if height == 2 {
            Some(restored_genesis_record(&chain)?)
        } else {
            None
        };
        let mut current = None;
        let mut walk = chain.walk(1, height);
        observe_restored_genesis(&mut walk, observe)?;
        while let Some(block) = next_restored_record(&mut walk, observe)? {
            checkpoints.record_sparse(block.height, checkpoint_of(block));
            previous = current.replace(block).or(previous);
        }
        let (Some(current), Some(previous)) = (current, previous) else {
            return Err("verified native prefix ended before the snapshot tip".into());
        };
        if self.blocks != Some(current)
            || self.revert.as_ref().map(|undo| undo.value) != Some(Some(previous))
        {
            return Err(
                "snapshot native execution tip/undo differs from verified native prefix".into(),
            );
        }
        drop(walk);
        // The sequential cursor stands at the tip, so the successor costs one certificate.
        let world_root = restored_successor_world_root(&chain, successor, current)?;
        Ok(RestoredNativeTip {
            cell: initialize(
                budget,
                Some(NativeExecutionTip(current)),
                Some(Some(NativeExecutionTip(previous))),
            )
            .map_err(TipRestoreError::Admission)?,
            world_root: Some(world_root),
        })
    }
}

/// An authenticated native tip and the certified root of the World published at it.
pub(in crate::state) struct RestoredNativeTip {
    /// The funded tip Cell with its authenticated predecessor undo.
    pub(in crate::state) cell: TipCell,
    /// `parent_world_state_root` of the tip's certified successor; `None` for empty history.
    pub(in crate::state) world_root: Option<Hash>,
}

// The successor's certified header must extend the authenticated tip; its certified result
// then binds the World published at the tip as its parent World state root.
#[inline(never)]
fn restored_successor_world_root(
    chain: &CertifiedChain<'_, StateView<'_>>,
    successor: u64,
    tip: NativeExecutionTipRecord,
) -> Result<Hash, TipRestoreError> {
    let certified = chain
        .certified(successor)
        .map_err(TipRestoreError::from)?
        .into_authenticated_execution()
        .map_err(|error| error.to_string())?;
    let committed = certified.committed();
    if committed.block().header().prev_block_hash() != Some(tip.iroha_hash)
        || !committed.header().is_some_and(|header| {
            header.parent_hash == Hash32(tip.core_hash)
                && header.parent_result == Hash32(tip.result)
        })
    {
        return Err("certified successor does not extend the snapshot tip".into());
    }
    Ok(committed.commitment().execution.parent_world_state_root)
}

// Keep the complete H1 receipt outside the restore frame. It retires before the
// successor walk starts, leaving only the authenticated fixed-size identity.
#[inline(never)]
fn restored_genesis_record(
    chain: &CertifiedChain<'_, StateView<'_>>,
) -> Result<NativeExecutionTipRecord, TipRestoreError> {
    let genesis = chain
        .authenticated_execution(1)
        .map_err(TipRestoreError::from)?;
    Ok(record(genesis.committed()))
}

// Genesis passes only through the observer. Its body signature alone cannot authenticate
// its execution R, so an H2 tip takes its genesis predecessor from the H2 anchor instead.
#[inline(never)]
fn observe_restored_genesis(
    walk: &mut impl Iterator<Item = Result<CertifiedBlock, ExecutionAttemptError<ChainReadError>>>,
    observe: &mut dyn FnMut(&CertifiedBlock) -> Result<(), TipRestoreError>,
) -> Result<(), TipRestoreError> {
    let genesis = walk
        .next()
        .ok_or("verified native prefix ended before the snapshot tip")?
        .map_err(TipRestoreError::from)?;
    observe(&genesis)
}

// The iterator owns the same source-bound prefix and advances in the same order.
// Complete certificate/result return temporaries belong to one step, rather than
// remaining reserved in restore's frame while its next native decode runs.
#[inline(never)]
fn next_restored_record(
    walk: &mut impl Iterator<Item = Result<CertifiedBlock, ExecutionAttemptError<ChainReadError>>>,
    observe: &mut dyn FnMut(&CertifiedBlock) -> Result<(), TipRestoreError>,
) -> Result<Option<NativeExecutionTipRecord>, TipRestoreError> {
    let Some(certified) = walk.next() else {
        return Ok(None);
    };
    let certified = certified.map_err(TipRestoreError::from)?;
    observe(&certified)?;
    let block = certified
        .into_authenticated_execution()
        .map_err(|error| error.to_string())?;
    Ok(Some(record(block.committed())))
}

/// The history checkpoint of an authenticated native execution identity.
fn checkpoint_of(
    record: NativeExecutionTipRecord,
) -> crate::kura::history_checkpoints::HistoryCheckpoint {
    crate::kura::history_checkpoints::HistoryCheckpoint {
        iroha_hash: record.iroha_hash,
        core_hash: Hash32(record.core_hash),
        result: Hash32(record.result),
    }
}
fn record(block: &CommittedBlock) -> NativeExecutionTipRecord {
    NativeExecutionTipRecord {
        root_scope: block.root_scope(),
        height: block.height(),
        creation_time_ms: u64::try_from(block.block().header().creation_time().as_millis())
            .expect("block creation time is a u64 millisecond value"),
        iroha_hash: block.block_hash(),
        core_hash: block.core_hash().0,
        result: block.result().0,
    }
}

impl StateBlock<'_> {
    /// Check the opaque owner before consuming its original output seal.
    pub(in crate::state) fn validate_native_execution_authorization(
        &self,
        authorization: &crate::sumeragi::executor::NativeExecutionAuthorization,
        block: &crate::block::CommittedBlock,
        certificate: &iroha_data_model::block::CommitCertificate,
    ) -> Result<NativeExecutionTipRecord, String> {
        let (next, parent) = authorization.for_state(self.state_ref)?;
        let iroha = block.as_ref();
        if next.height != iroha.header().height().get()
            || u128::from(next.creation_time_ms) != iroha.header().creation_time().as_millis()
            || next.iroha_hash != iroha.hash()
            || Some(next.result)
                != crate::sumeragi::commitment::result_of_preimage(certificate.result_preimage())
                    .map(|result| result.0)
        {
            return Err("native execution owner differs from the original sealed carrier".into());
        }
        let certified_core = if parent.is_none() {
            crate::sumeragi::startup::core_hash_of(iroha)
        } else {
            let header: iroha_sumeragi::message::BlockHeader =
                norito::decode_canonical(certificate.consensus_header())
                    .map_err(|error| error.to_string())?;
            if Some((header.parent_hash, header.parent_result)) != parent
                || header.height != next.height
            {
                return Err("native certificate changed the original execution parent".into());
            }
            header.hash(&crate::sumeragi::crypto::BlsCrypto::new())
        };
        if certified_core.0 != next.core_hash {
            return Err("native certificate changed the original executed header".into());
        }
        match (*self.native_execution_tip.get(), parent) {
            (None, None)
                if next.height == 1
                    && iroha.header().prev_block_hash().is_none()
                    && self.block_hashes.is_empty()
                    && certificate.consensus_header().is_empty()
                    && certificate.commit_qc().is_empty() => {}
            (Some(previous), Some((parent_hash, parent_result)))
                if previous.root_scope() == next.root_scope
                    && previous.height().checked_add(1) == Some(next.height)
                    && usize::try_from(previous.height()).ok() == Some(self.block_hashes.len())
                    && self.block_hashes.last().copied() == Some(previous.iroha_hash())
                    && iroha.header().prev_block_hash() == Some(previous.iroha_hash())
                    && parent_hash == previous.core_hash()
                    && parent_result == previous.result() => {}
            _ => {
                return Err(
                    "native execution owner does not extend this original State tip".into(),
                );
            }
        }
        Ok(next)
    }

    /// Stage only after the immutable witness surface has been revalidated.
    pub(in crate::state) fn advance_native_execution_tip(
        &mut self,
        authorization: &crate::sumeragi::executor::NativeExecutionAuthorization,
        block: &crate::block::CommittedBlock,
    ) -> Result<(), String> {
        let (next, _) = authorization.for_state(self.state_ref)?;
        if next.iroha_hash != block.as_ref().hash()
            || next.height != self._curr_block.height().get()
        {
            return Err("native execution tip changed during metadata preparation".into());
        }
        *self.native_execution_tip.get_mut() = Some(NativeExecutionTip(next));
        // An abandoned block's checkpoint is harmless: readers trust a checkpoint
        // only while it matches their committed hash journal.
        self.kura
            .history_checkpoints()
            .record_sparse(next.height, checkpoint_of(next));
        Ok(())
    }
}

/// Fund both exact future EBR generations and the original successor before a writer exists.
pub(in crate::state) fn original_cell<'a>(
    target: &'a TipCell,
    budget: &AllocationBudget,
    parent: &mut iroha_allocation::AllocationReservation,
) -> Result<
    mv::cell::BlockAcquisitionSlot<'a, Option<NativeExecutionTip>, AllocationCharge>,
    mv::storage::AdmittedStorageError,
> {
    use mv::storage::AdmittedStorageError as Error;
    let layouts = TipCell::allocation_layouts();
    let mut generations = budget
        .try_reserve_layouts(layouts)
        .map_err(Error::Allocation)?;
    let current = generations
        .try_split(layouts[0])
        .expect("original native tip current layout");
    let undo = generations
        .try_split(layouts[1])
        .expect("original native tip undo layout");
    let backing = mv::cell::CellGenerationBacking::try_from_charges(
        budget,
        mv::cell::CellAllocationCharges::new(current, undo),
    )
    .map_err(|(_charges, error)| match error {
        mv::cell::CellGenerationBackingError::Allocator { layout } => Error::Allocator { layout },
        _ => Error::PolicyIdentity,
    })?;
    let charge = parent
        .try_split(mv::cell::CellPublicationSuccessor::allocation_layout())
        .map_err(|error| Error::PolicyDemand {
            expected_bytes: error.requested_bytes,
            remaining_bytes: error.remaining_bytes,
        })?;
    let successor = mv::cell::CellPublicationSuccessor::try_from_charge(budget, charge).map_err(
        |(_charge, error)| match error {
            mv::cell::CellPublicationSuccessorError::Allocator { layout } => {
                Error::Allocator { layout }
            }
            _ => Error::PolicyIdentity,
        },
    )?;
    target
        .try_block_acquisition_with_backing(backing, successor, budget)
        .map_err(|_| Error::PolicyIdentity)
}

impl State {
    /// Root scope established by original native execution or authenticated recovery.
    ///
    /// This copies only the opaque native-tip Cell. Successors preserve this scope,
    /// so reading it needs neither a compound State view nor a publication retry.
    /// It grants no height, result, freshness, or World authority. Absence means
    /// native execution has not established a scope; callers must not infer Global.
    #[must_use]
    pub fn authenticated_root_scope(
        &self,
    ) -> Option<iroha_data_model::block::consensus::SumeragiRootScope> {
        self.native_execution_tip
            .view()
            .get()
            .map(NativeExecutionTip::root_scope)
    }
}

impl StateView<'_> {
    pub(in crate::state) fn native_execution_tip_value(&self) -> Option<NativeExecutionTip> {
        *self.native_execution_tip.get()
    }
}
impl StateBlock<'_> {
    pub(in crate::state) fn native_execution_tip_value(&self) -> Option<NativeExecutionTip> {
        *self.native_execution_tip.get()
    }
}
impl StateQueryView<'_> {
    pub(in crate::state) fn native_execution_tip_value(&self) -> Option<NativeExecutionTip> {
        *self.native_execution_tip.get()
    }
}
impl StateTransaction<'_, '_> {
    pub(in crate::state) fn native_execution_tip_value(&self) -> Option<NativeExecutionTip> {
        self.native_execution_tip
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: complete State capture work and custody before consuming the World-only prerequisite"
    )
)]
mod finalized_world;

#[cfg(test)]
mod tests;

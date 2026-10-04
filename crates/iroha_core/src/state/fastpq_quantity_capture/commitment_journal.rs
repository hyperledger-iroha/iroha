//! Mandatory rollback-local quantity commitments independent of optional proof tapes.
//!
//! Physical shortage defers the original local attempt before the business callback.
//! Unsupported mutation coverage remains a deterministic nonexportable status; it
//! must not be replaced with an optional archive-capacity observation in D7.

use super::super::fastpq_quantity_write_plan::QuantityWritePlanError;
use super::*;
mod finalized_source;
use crate::execution_attempt::ExecutionDeferred;
use finalized_source::FinalizedQuantitySource;
pub(crate) use finalized_source::{
    AdmittedQuantityArchive, CapturedExecWitness, CapturedQuantityEntry,
};
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBufferError};
use iroha_data_model::fastpq::{FastpqExecutionEffectCommitmentV1, FastpqExecutionEffectRefV1};

/// One fixed-width source journal and its parent occurrence at original preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    commitment: FastpqExecutionEffectCommitmentV1,
    baseline: Option<FastpqExecutionEffectCommitmentV1>,
}

/// Exact deterministic coverage evidence from the original mutation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CoverageGap {
    /// An original quantity storage owner observed an untyped actual write or mutable lease.
    RawQuantityWrite,
    /// At least one exact typed port applied, but the complete operation did not finish.
    PartialTypedOperation,
}

/// Original source journal; optional tape refusal never sets this status.
#[derive(Default)]
pub(super) struct QuantityCommitmentJournal {
    entries: QuantityArchiveMap<Entry>,
    parent_backing: Option<ChargedBuffer<(Hash, Entry)>>,
    unsupported: Option<CoverageGap>,
    /// Local custody violation; never serialized as unsupported operation semantics.
    invalid: bool,
    sealed: Option<iroha_allocation::ChargedShared<FinalizedQuantitySource>>,
}

impl QuantityCommitmentJournal {
    pub(super) fn unsupported(&mut self, issue: CoverageGap) {
        if self.unsupported.is_none() {
            self.unsupported = Some(issue);
        }
    }

    pub(super) fn invalidate(&mut self) {
        self.invalid = true;
    }

    #[cfg(test)]
    pub(super) fn coverage_gap(&self) -> Option<CoverageGap> {
        self.unsupported
    }
    #[cfg(test)]
    pub(super) fn is_invalid(&self) -> bool {
        self.invalid
    }

    /// Verify original typed coverage before projecting any optional tape source seal.
    pub(super) fn original(
        &self,
        context: FastpqExecutionEffectContextV1,
    ) -> Result<FastpqExecutionEffectCommitmentV1, QuantityCaptureIssue> {
        if self.invalid || self.unsupported.is_some() {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let commitment = self
            .get(&context.entry.entry_hash)
            .copied()
            .unwrap_or_else(|| FastpqExecutionEffectCommitmentV1::new(context));
        if commitment.context() != &context {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        Ok(commitment)
    }

    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn get(&self, hash: &Hash) -> Option<&FastpqExecutionEffectCommitmentV1> {
        self.entries.get(hash).map(|entry| &entry.commitment)
    }

    /// Apply exactly the pre-admitted child replacements after original World apply.
    /// No admission, allocation, serialization or payload clone occurs here.
    pub(super) fn apply(&mut self, mut pending: Self) {
        if self.sealed.is_some() || pending.sealed.is_some() {
            self.invalid = true;
            return;
        }
        self.invalid |= pending.invalid;
        if let Some(issue) = pending.unsupported {
            self.unsupported(issue);
        }
        for (hash, entry) in pending.entries.iter() {
            if entry.baseline.as_ref() != self.get(hash) {
                self.invalid = true;
                return;
            }
        }
        if pending.entries.is_empty() {
            return;
        }
        let Some(backing) = pending.parent_backing.take() else {
            self.invalid = true;
            return;
        };
        if self
            .entries
            .len()
            .checked_add(pending.entries.len())
            .is_none_or(|count| count > backing.capacity())
        {
            self.invalid = true;
            return;
        }
        pending.entries.for_each_mut(|entry| entry.baseline = None);
        self.entries.apply_pending(pending.entries, backing);
    }
}

/// Original local retry is never converted to consensus-visible unsupported semantics.
pub(super) enum PrepareError {
    Unsupported(QuantityCaptureIssue),
    Deferred(ExecutionDeferred),
}
impl From<QuantityCaptureIssue> for PrepareError {
    fn from(issue: QuantityCaptureIssue) -> Self {
        Self::Unsupported(issue)
    }
}
fn buffer_error(error: ChargedBufferError) -> PrepareError {
    PrepareError::Deferred(match error {
        ChargedBufferError::Admission(refusal) => refusal.into(),
        ChargedBufferError::Allocator { .. } => {
            ivm::error::ExecutionDeferral::AllocationUnavailable.into()
        }
    })
}

/// Prepared next fixed journal row and exact original write permits.
pub(super) struct PreparedQuantityJournal {
    hash: Hash,
    before: Option<FastpqExecutionEffectCommitmentV1>,
    row: Entry,
    pending_backing: ChargedBuffer<(Hash, Entry)>,
    parent_backing: ChargedBuffer<(Hash, Entry)>,
    pub(super) write_plan: Option<QuantityWritePlan<QuantityWriteKey, Quantity>>,
}

impl PreparedQuantityJournal {
    pub(super) fn prepare<'a, I>(
        state: &StateTransaction<'_, '_>,
        context: FastpqExecutionEffectContextV1,
        authority_digest: Hash,
        authorization_context: Hash,
        kinds: I,
    ) -> Result<Self, PrepareError>
    where
        I: Clone + ExactSizeIterator<Item = Result<QuantityKindInput<'a>, QuantityCaptureIssue>>,
    {
        let hash = context.entry.entry_hash;
        let parent = &state.block_fastpq_quantity_candidate.commitments;
        let pending = &state.pending_fastpq_quantity_candidate.commitments;
        let baseline = parent.get(&hash).copied();
        let before = pending.get(&hash).copied().or(baseline);
        if let Some(row) = pending.entries.get(&hash) {
            if row.baseline != baseline {
                return Err(QuantityCaptureIssue::InvalidFacts.into());
            }
        }
        let mut commitment =
            before.unwrap_or_else(|| FastpqExecutionEffectCommitmentV1::new(context));
        if commitment.context() != &context {
            return Err(QuantityCaptureIssue::InvalidFacts.into());
        }
        for kind in kinds.clone() {
            commitment
                .append(FastpqExecutionEffectRefV1 {
                    ordinal: commitment.count(),
                    authority_digest,
                    authorization_context,
                    kind: kind?.commitment_input(),
                })
                .map_err(|_| PrepareError::Unsupported(QuantityCaptureIssue::InvalidFacts))?;
        }
        let budget: &AllocationBudget = state.pipeline_ivm_prepared_cache.execution_budget();
        let pending_capacity = pending
            .entries
            .len()
            .checked_add(usize::from(pending.get(&hash).is_none()))
            .ok_or_else(|| PrepareError::Deferred(AllocationRefusal::DemandOverflow.into()))?;
        let parent_capacity = parent
            .entries
            .len()
            .checked_add(pending_capacity)
            .ok_or_else(|| PrepareError::Deferred(AllocationRefusal::DemandOverflow.into()))?;
        let pending_backing = ChargedBuffer::new(pending_capacity, budget).map_err(buffer_error)?;
        let parent_backing = ChargedBuffer::new(parent_capacity, budget).map_err(buffer_error)?;
        let max_ports = kinds
            .len()
            .checked_mul(2)
            .ok_or_else(|| PrepareError::Deferred(AllocationRefusal::DemandOverflow.into()))?;
        let write_plan = QuantityWritePlan::from_inputs(kinds, max_ports, budget).map_err(
            |error| match error {
                QuantityWritePlanError::Deferred(refusal) => PrepareError::Deferred(refusal),
                _ => PrepareError::Unsupported(QuantityCaptureIssue::InvalidFacts),
            },
        )?;
        if !state.quantity_pre_state_matches(&write_plan) {
            return Err(QuantityCaptureIssue::InvalidFacts.into());
        }
        Ok(Self {
            hash,
            before,
            row: Entry {
                commitment,
                baseline,
            },
            pending_backing,
            parent_backing,
            write_plan: Some(write_plan),
        })
    }

    /// Publish only after exact original permits and live poststate were verified.
    pub(super) fn publish(
        self,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), QuantityCaptureIssue> {
        let parent = &state.block_fastpq_quantity_candidate.commitments;
        let pending = &mut state.pending_fastpq_quantity_candidate.commitments;
        if self.row.baseline.as_ref() != parent.get(&self.hash)
            || self.before.as_ref() != pending.get(&self.hash).or_else(|| parent.get(&self.hash))
        {
            return Err(QuantityCaptureIssue::InterruptedScope);
        }
        pending.entries.grow(self.pending_backing);
        pending.entries.insert_reserved(self.hash, self.row);
        pending.parent_backing = Some(self.parent_backing);
        Ok(())
    }
}

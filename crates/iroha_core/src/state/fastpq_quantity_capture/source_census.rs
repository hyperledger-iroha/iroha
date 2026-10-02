//! Original completed source census joined to immutable candidate quantity tapes.
//!
//! Every producer invocation remains explicit, including rejected and zero-effect
//! calls. This local owner grants no finality, proof availability or publication.
//! TODO: close lifecycle, mandatory supply and full physical source ownership, then
//! integrate the complete relation and atomic D7/finalized custody before export.

use super::*;
use iroha_allocation::AllocationBudget;
use iroha_data_model::fastpq::FastpqSourceStatementContextV1;
use std::{io, sync::Arc};

/// A failed or interrupted original attempt cannot be replaced with another census.
#[derive(Default)]
pub(super) enum QuantitySourceCensusState {
    #[default]
    Unsealed,
    Preparing,
    Sealed(QuantitySourceCensus),
    Failed,
}

impl QuantitySourceCensusState {
    pub(super) fn is_preparing_or_sealed(&self) -> bool {
        matches!(self, Self::Preparing | Self::Sealed(_))
    }
}

/// Fixed-size facts for one position in the original producer's complete order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct QuantitySourceEntrySeal {
    pub(super) context: FastpqExecutionEffectContextV1,
    pub(super) effect_count: u32,
    pub(super) frame_bytes: u64,
    pub(super) effects_digest: Hash,
}

/// Move-only retention of the exact original inventory and its quantity projection.
///
/// The new fixed backing is charged before allocation. The inventory and frozen
/// source contexts retain their existing Arc allocations without cloning contents.
/// Original inventory allocation and canonical serializer scratch remain their
/// existing owners' obligations; this does not claim complete physical capture.
pub(super) struct QuantitySourceCensus {
    entries: ChargedBuffer<QuantitySourceEntrySeal>,
    inventory: Arc<FastpqSourceInventoryV1>,
    frozen: Arc<crate::fastpq::FastpqBlockStartSourceContext>,
    creation_time_ms: u64,
    applied_world_transactions: u64,
    usage: QuantityCandidateUsage,
}

fn entry_seal(
    archive: &QuantityCandidateArchive,
    source: FastpqSourceStatementContextV1,
    entry: FastpqSourceExecutionEntryV1,
) -> Result<(QuantitySourceEntrySeal, QuantityCandidateUsage), QuantityCaptureIssue> {
    let context = FastpqExecutionEffectContextV1 { source, entry };
    let empty = FastpqExecutionEffectsV1 {
        context,
        effects: Vec::new(),
    };
    let captured = archive.entries.get(&entry.entry_hash);
    let wire = captured.map_or(&empty, |captured| &**captured);
    let effect_count =
        u32::try_from(wire.effects.len()).map_err(|_| QuantityCaptureIssue::Capacity)?;
    let frame_bytes = u64::try_from(
        norito::canonical_frame_len(wire).map_err(|_| QuantityCaptureIssue::InvalidFacts)?,
    )
    .map_err(|_| QuantityCaptureIssue::Capacity)?;
    let usage = if let Some(captured) = captured {
        if captured.context != context
            || wire.context != context
            || effect_count == 0
            || captured.measurement.baseline != QuantityCandidateUsage::default()
            || captured.measurement.full.entries != 1
            || captured.measurement.full.deltas != u64::from(effect_count)
            || captured.measurement.full.statement_bytes != frame_bytes
            || wire
                .effects
                .iter()
                .enumerate()
                .any(|(index, effect)| usize::try_from(effect.ordinal).ok() != Some(index))
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        captured.measurement.full
    } else {
        QuantityCandidateUsage::default()
    };
    let effects_digest = Hash::new_from_writer(|writer| {
        norito::core::write_canonical_to_writer(wire, writer).map_err(io::Error::other)
    })
    .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
    Ok((
        QuantitySourceEntrySeal {
            context,
            effect_count,
            frame_bytes,
            effects_digest,
        },
        usage,
    ))
}

impl QuantitySourceCensus {
    pub(super) fn prepare(
        block: &StateBlock<'_>,
        budget: &AllocationBudget,
    ) -> Result<Self, QuantityCaptureIssue> {
        let archive = &block.fastpq_quantity_candidate;
        if let Some(issue) = archive.issue {
            return Err(issue);
        }
        let inventory = block
            .verified_fastpq_source_inventory_for_capture()
            .map_err(|_| QuantityCaptureIssue::UnsupportedOwner)?;
        let frozen = block
            .fastpq_source_context
            .as_ref()
            .ok_or(QuantityCaptureIssue::UnsupportedOwner)?;
        let limits = block.fastpq_source_policy_at_block_start().0.block;
        if u64::try_from(inventory.entries().len()).map_err(|_| QuantityCaptureIssue::Capacity)?
            > u64::from(limits.max_executed_entries)
            || archive.usage.entries > u64::from(limits.max_executed_entries)
            || archive.usage.deltas > u64::from(limits.max_deltas)
            || archive.usage.input_bytes > limits.max_input_transcript_bytes
            || archive.usage.statement_bytes > limits.max_total_statement_bytes
        {
            return Err(QuantityCaptureIssue::Capacity);
        }
        // Exact original credit is acquired before the only new backing allocation.
        let mut entries = ChargedBuffer::new(inventory.entries().len(), budget)
            .map_err(|_| QuantityCaptureIssue::Capacity)?;
        let mut usage = QuantityCandidateUsage::default();
        for entry in inventory.entries() {
            let (seal, contribution) = entry_seal(archive, inventory.source(), *entry)?;
            usage = usage
                .checked_add(contribution)
                .ok_or(QuantityCaptureIssue::Capacity)?;
            entries.push_reserved(seal);
        }
        if usage != archive.usage
            || usize::try_from(usage.entries).ok() != Some(archive.entries.len())
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        Ok(Self {
            entries,
            inventory,
            frozen: Arc::clone(frozen),
            creation_time_ms: block._curr_block.creation_time_ms,
            applied_world_transactions: archive.applied_world_transactions,
            usage,
        })
    }

    fn verify_current(&self, block: &StateBlock<'_>) -> Result<(), QuantityCaptureIssue> {
        let archive = &block.fastpq_quantity_candidate;
        let current = block
            .verified_fastpq_source_inventory_for_capture()
            .map_err(|_| QuantityCaptureIssue::UnsupportedOwner)?;
        if !Arc::ptr_eq(&self.inventory, &current)
            || block
                .fastpq_source_context
                .as_ref()
                .is_none_or(|frozen| !Arc::ptr_eq(&self.frozen, frozen))
            || self.creation_time_ms != block._curr_block.creation_time_ms
            || self.applied_world_transactions != archive.applied_world_transactions
            || self.usage != archive.usage
            || self.entries.as_slice().len() != self.inventory.entries().len()
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let mut usage = QuantityCandidateUsage::default();
        for (retained, original) in self.entries.as_slice().iter().zip(self.inventory.entries()) {
            let (current, contribution) = entry_seal(archive, self.inventory.source(), *original)?;
            if retained != &current {
                return Err(QuantityCaptureIssue::InvalidFacts);
            }
            usage = usage
                .checked_add(contribution)
                .ok_or(QuantityCaptureIssue::Capacity)?;
        }
        if usage != self.usage || usize::try_from(usage.entries).ok() != Some(archive.entries.len())
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn entries(&self) -> &[QuantitySourceEntrySeal] {
        self.entries.as_slice()
    }
}

impl StateBlock<'_> {
    /// Called solely by the original completed output producer's successful finalizer.
    pub(in crate::state) fn retain_quantity_source_census(&mut self) {
        self.observe_quantity_block_journals();
        if self.fastpq_quantity_candidate.issue.is_some() {
            return;
        }
        if !matches!(
            self.fastpq_quantity_candidate.source_census,
            QuantitySourceCensusState::Unsealed
        ) {
            self.fastpq_quantity_candidate
                .poison(QuantityCaptureIssue::InvalidFacts);
            return;
        }
        // A caught unwind leaves Preparing; observation refuses it and cannot retry.
        self.fastpq_quantity_candidate.source_census = QuantitySourceCensusState::Preparing;
        let prepared = QuantitySourceCensus::prepare(
            self,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        );
        match prepared {
            Ok(census) => {
                self.fastpq_quantity_candidate.source_census =
                    QuantitySourceCensusState::Sealed(census);
            }
            Err(issue) => self.fastpq_quantity_candidate.poison(issue),
        }
    }

    pub(in crate::state) fn reject_quantity_source_census(&mut self) {
        self.fastpq_quantity_candidate
            .poison(QuantityCaptureIssue::UnsupportedOwner);
    }

    pub(super) fn observe_quantity_source_census(&mut self) {
        let result = match &self.fastpq_quantity_candidate.source_census {
            QuantitySourceCensusState::Preparing => Err(QuantityCaptureIssue::InterruptedScope),
            QuantitySourceCensusState::Sealed(census) => census.verify_current(self),
            QuantitySourceCensusState::Unsealed | QuantitySourceCensusState::Failed => Ok(()),
        };
        if let Err(issue) = result {
            self.fastpq_quantity_candidate.poison(issue);
        }
    }
}

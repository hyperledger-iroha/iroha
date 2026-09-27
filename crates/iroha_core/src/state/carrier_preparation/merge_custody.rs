//! Move the original certified merge execution through the private carrier tail.
//!
//! The preparing phase retains the original mutable State owners. Completion
//! moves those same owners into the immutable prefix, after metadata and World
//! preparation. Neither phase creates decision, durability or publication rights.

use super::*;
use crate::state::merge_execution_prefix::MergeExecutionPrefixSeal;

pub(super) struct PreparingMergeSource {
    entry_hash: HashOf<MergeLedgerEntry>,
    execution: Option<Arc<MergeExecutionPrefixSeal>>,
}

pub(super) struct MergeSourceCustody {
    entry: MergeLedgerEntry,
    execution: Option<Arc<MergeExecutionPrefixSeal>>,
    // This move-only authorization was minted by actual certified reexecution.
    // It survives publication alongside the original entry, outputs and witness.
    authorization: Option<CanonicalWsvMergeCommitAuthorization>,
}

fn same_execution(
    original: Option<&Arc<MergeExecutionPrefixSeal>>,
    actual: Option<&Arc<MergeExecutionPrefixSeal>>,
) -> bool {
    match (original, actual) {
        (None, None) => true,
        (Some(original), Some(actual)) => Arc::ptr_eq(original, actual),
        _ => false,
    }
}

impl PreparingMergeSource {
    pub(super) fn capture(
        state: &mut StateBlock<'_>,
        block: &SignedBlock,
    ) -> Result<Self, MergeLedgerCommitError> {
        let invalid =
            |message: &str| MergeLedgerCommitError::ExecutionBatchInvalid(message.to_owned());
        if state.native_lane_stage.is_some()
            || block
                .execution_context()
                .is_some_and(|bundle| bundle.native_lane_decisions.is_some())
            || state
                .canonical_carrier_commit_metadata_authorization
                .is_some()
        {
            return Err(invalid(
                "merge source contains competing execution or premature finality authority",
            ));
        }
        state
            .verify_merge_prefix_carrier(block)
            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
        state.validate_staged_merge_execution_authorization()?;
        let entry = state
            .staged_merge_entry
            .as_ref()
            .ok_or_else(|| invalid("merge preparation lost its original certified entry"))?;
        if block
            .execution_context()
            .and_then(|bundle| bundle.merge_entry.as_ref())
            .is_none_or(|reference| !reference.matches_entry(entry))
            || entry.execution_batch.is_some() != state.merge_prefix_seal().is_some()
            || entry.execution_batch.is_some()
                != state.canonical_wsv_merge_commit_authorization.is_some()
        {
            return Err(invalid(
                "merge preparation differs from its original certified execution",
            ));
        }
        Ok(Self {
            entry_hash: entry.canonical_hash(),
            execution: state.merge_prefix_seal().cloned(),
        })
    }

    pub(super) fn retains_state(&self, state: &StateBlock<'_>) -> bool {
        state.native_lane_stage.is_none()
            && state
                .canonical_carrier_commit_metadata_authorization
                .is_none()
            && same_execution(self.execution.as_ref(), state.merge_prefix_seal())
            && state.staged_merge_entry.as_ref().is_some_and(|entry| {
                entry.canonical_hash() == self.entry_hash
                    && entry.execution_batch.is_some() == self.execution.is_some()
                    && entry.execution_batch.is_some()
                        == state.canonical_wsv_merge_commit_authorization.is_some()
            })
    }
}

impl MergeSourceCustody {
    pub(super) fn retains_carrier(
        &self,
        block: &SignedBlock,
        sources: &output_capacity::OwnedExecutionSources,
    ) -> bool {
        if sources.is_native()
            || !same_execution(self.execution.as_ref(), sources.merge_prefix())
            || block
                .execution_context()
                .is_some_and(|bundle| bundle.native_lane_decisions.is_some())
            || block
                .execution_context()
                .and_then(|bundle| bundle.merge_entry.as_ref())
                .is_none_or(|reference| !reference.matches_entry(&self.entry))
        {
            return false;
        }
        match (
            &self.execution,
            &self.authorization,
            &self.entry.execution_batch,
        ) {
            (None, None, None) => true,
            (Some(execution), Some(authorization), Some(batch)) => {
                execution.verify_carrier_entry(block, &self.entry).is_ok()
                    && authorization.entry_hash == self.entry.canonical_hash()
                    && authorization.carrier_height == block.header().height().get()
                    && authorization.carrier_hash == block.hash()
                    && authorization.batch_hash == batch.batch_hash
                    && authorization.validated_publication_event_bytes.is_some()
            }
            _ => false,
        }
    }

    pub(super) fn retains_closed_state(
        &self,
        state: &StateBlock<'_>,
        sources: &output_capacity::OwnedExecutionSources,
    ) -> bool {
        state.native_lane_stage.is_none()
            && state.staged_merge_entry.is_none()
            && state.canonical_wsv_merge_commit_authorization.is_none()
            && state
                .canonical_carrier_commit_metadata_authorization
                .is_none()
            && same_execution(self.execution.as_ref(), state.merge_prefix_seal())
            && same_execution(self.execution.as_ref(), sources.merge_prefix())
    }
}

impl ValidatedExecutionPrefix {
    pub(super) fn merge_entry(&self) -> Option<&MergeLedgerEntry> {
        match &self.authority {
            PrefixSourceAuthority::Merge(merge) => Some(&merge.entry),
            _ => None,
        }
    }
}

impl PrefixPreparation<'_> {
    /// Move authority only after the complete original metadata and World tail.
    pub(super) fn finish_merge_source(
        &mut self,
        block: &SignedBlock,
    ) -> Result<(), MergeLedgerCommitError> {
        let PrefixSourceAuthority::PreparingMerge(preparing) = &self.prefix.authority else {
            return Ok(());
        };
        if !self.prefix.retains_closed_state(&self.state)
            || !self
                .state
                .block_hashes
                .pending()
                .iter()
                .copied()
                .eq([block.hash()])
            || !self.state.world.external_event_buf.is_empty()
        {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "merge source lost its original completed carrier tail".into(),
            ));
        }
        self.state
            .verify_merge_prefix_carrier(block)
            .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
        if self
            .state
            .staged_merge_entry
            .as_ref()
            .is_some_and(|entry| entry.execution_batch.is_some())
        {
            // Preserve the existing certified-execution composition boundary.
            // The mandatory signed RS16 carrier layout is independent of these
            // optional runtime DA/pin/relay effects.
            self.state.validate_merge_runtime_catalog_effects()?;
            if self.state.pending_da_commitments.is_some()
                || self.state.pending_da_pin_intents.is_some()
                || !self.state.verified_lane_relay_records.is_empty()
            {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "prepared autonomous carrier staged an incompatible runtime effect".into(),
                ));
            }
            let height = block.header().height().try_into().map_err(|_| {
                MergeLedgerCommitError::ExecutionBatchInvalid(
                    "prepared merge height exceeds storage bounds".into(),
                )
            })?;
            if !self
                .state
                .transactions
                .has_exact_staged_block(height, &self.state.merge_carrier_entrypoints)
            {
                return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                    "prepared merge membership differs from its exact certified entrypoints".into(),
                ));
            }
        }
        let original_execution = preparing.execution.clone();
        let entry = self.state.staged_merge_entry.take().ok_or_else(|| {
            MergeLedgerCommitError::ExecutionBatchInvalid(
                "merge source disappeared at capture".into(),
            )
        })?;
        let custody = MergeSourceCustody {
            entry,
            execution: original_execution,
            authorization: self.state.canonical_wsv_merge_commit_authorization.take(),
        };
        if !custody.retains_carrier(block, self.prefix.sources()) {
            return Err(MergeLedgerCommitError::ExecutionBatchInvalid(
                "completed merge source differs from its original carrier".into(),
            ));
        }
        self.prefix.authority = PrefixSourceAuthority::Merge(Box::new(custody));
        Ok(())
    }
}

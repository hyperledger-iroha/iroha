//! State-owned, whole-inventory construction reservations after execution has finished.
//!
//! A prospective reservation always remeasures the complete sealed archive. No occurrence
//! prefix is added to an earlier statement size. Preparing or dropping an attempt leaves
//! committed accounting untouched; successful materialization replaces it atomically.
//! This local seam does not admit execution, reserve WSV effects, or authenticate policy.
//! TODO: integrate authenticated intrinsic/block policy, transaction savepoints and mandatory
//! work accounting before using source usage for proposal packing or runtime admission.

use std::{collections::BTreeMap, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqOrdinarySourceStatementLeafV1, FastpqOrdinarySourceStatementManifestV1,
    TransferTranscript,
};
use mv::storage::StorageReadOnly;

use super::{FastpqSourceInventoryV1, StateBlock};
use crate::fastpq::{
    FastpqSourceStatementBuildLimits, FastpqSourceTranscriptUsage,
    measure_fastpq_source_statement_usage,
};

/// Exact local construction usage for one complete execution-owned inventory.
///
/// This diagnostic value is not serialized or accepted as a reservation input. Public
/// fields do not confer source, policy, admission, proof or finality authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastpqSourceStatementUsageV1 {
    /// All owned entries, including nontransfer and rejected external/time entries.
    pub executed_entries: u32,
    /// Original ordered transcript occurrences across all nonempty entry bundles.
    pub transcripts: usize,
    /// Original transfer deltas, without deduplicating repeated occurrences.
    pub deltas: usize,
    /// Complete canonical input transcript frames, including supplied private paths.
    pub input_transcript_bytes: usize,
    /// Largest complete canonical statement frame, one statement per nonempty entry.
    pub max_statement_bytes: usize,
    /// Sum of complete statement frames over disjoint entry identities.
    pub total_statement_bytes: usize,
}

impl FastpqSourceStatementUsageV1 {
    fn from_measured(executed_entries: u32, measured: FastpqSourceTranscriptUsage) -> Self {
        Self {
            executed_entries,
            transcripts: measured.transcripts,
            deltas: measured.deltas,
            input_transcript_bytes: measured.input_transcript_bytes,
            max_statement_bytes: measured.max_statement_bytes,
            total_statement_bytes: measured.total_statement_bytes,
        }
    }
}

/// Local construction budget bound to one privately finalized State inventory allocation.
///
/// Only State constructs this owner. Limits are explicit local construction caps; they
/// are not authenticated execution policy. Until an attempt materializes successfully,
/// there is no committed usage. Retrying the same inventory replaces its whole usage,
/// rather than charging it twice. This object does not mutate or reserve WSV resources.
/// Creating several such local objects cannot authorize additional execution or writes.
#[derive(Debug)]
pub struct FastpqSourceStatementBudgetV1 {
    inventory: Arc<FastpqSourceInventoryV1>,
    limits: FastpqSourceStatementBuildLimits,
    committed: Option<FastpqSourceStatementUsageV1>,
}

/// Prepared complete-archive attempt whose input cannot change while it is borrowed.
///
/// Dropping this value aborts the attempt without changing the budget. No private SMT,
/// statement, proof or witness output exists at preparation time. The mutable budget
/// borrow prevents overlapping commits; the transcript borrow prevents input mutation.
#[derive(Debug)]
pub struct FastpqSourceStatementAttemptV1<'budget, 'transcripts> {
    budget: &'budget mut FastpqSourceStatementBudgetV1,
    transcripts: &'transcripts BTreeMap<Hash, Vec<TransferTranscript>>,
    usage: FastpqSourceStatementUsageV1,
}

fn checked_entry_count(count: usize) -> Result<u32, String> {
    u32::try_from(count).map_err(|_| "FASTPQ owned entry count exceeds u32".into())
}

fn owned_inventory(block: &StateBlock<'_>) -> Result<Arc<FastpqSourceInventoryV1>, String> {
    if block.authenticated_replay_commit {
        return Err("authenticated replay cannot reserve new ordinary FASTPQ statements".into());
    }
    block.verified_fastpq_source_inventory_for_capture()
}

impl StateBlock<'_> {
    /// Bind a local construction budget to this block's complete finalized source inventory.
    ///
    /// The complete E count comes from State, not transcript keys or a caller-provided
    /// archive. No source default or policy digest is invented. This does not publish a
    /// witness, drain the recorder, latch a capture error, or perform private materialization.
    ///
    /// # Errors
    /// Rejects replay, missing/failed/stale ownership, an unrepresentable count or transcript
    /// ceiling, and an exceeded E cap. The remaining dimensions are checked during preparation.
    pub fn fastpq_source_statement_budget(
        &self,
        limits: FastpqSourceStatementBuildLimits,
    ) -> Result<FastpqSourceStatementBudgetV1, String> {
        let inventory = owned_inventory(self)?;
        let entries = checked_entry_count(inventory.entries().len())?;
        FastpqSourceTranscriptUsage::default().check_limits(entries, limits)?;
        Ok(FastpqSourceStatementBudgetV1 {
            inventory,
            limits,
            committed: None,
        })
    }
}

impl FastpqSourceStatementBudgetV1 {
    /// Last successfully materialized whole-inventory usage, or no committed attempt.
    pub const fn committed_usage(&self) -> Option<FastpqSourceStatementUsageV1> {
        self.committed
    }

    fn verify_current(&self, block: &StateBlock<'_>) -> Result<(), String> {
        let inventory = owned_inventory(block)?;
        if !Arc::ptr_eq(&self.inventory, &inventory) {
            return Err("FASTPQ source reservation inventory owner changed".into());
        }
        Ok(())
    }

    /// Stage exact usage for the complete, finalized, execution-owned transcript archive.
    ///
    /// All six inclusive caps are checked through the canonical measurement owner before
    /// a statement leaf or private SMT is allocated. This remeasures every full same-entry
    /// bundle, including common scales, repeated-key chronology and original occurrences.
    /// Exact public-seal verification follows bounded measurement. Private paths are excluded
    /// from that seal but included in I, so changed paths always require new measurement.
    /// No entry count, usage delta, prefix size or permission root is accepted from a caller.
    ///
    /// # Errors
    /// Rejects stale/foreign State ownership, changed public content or grouping, incomplete
    /// entry bundles, invalid full-domain transfers, overflow or any exceeded construction cap.
    /// A failed call preserves every previous committed value and leaves State/input untouched.
    pub fn prepare<'budget, 'transcripts>(
        &'budget mut self,
        block: &StateBlock<'_>,
        transcripts: &'transcripts BTreeMap<Hash, Vec<TransferTranscript>>,
    ) -> Result<FastpqSourceStatementAttemptV1<'budget, 'transcripts>, String> {
        self.verify_current(block)?;
        let entries = checked_entry_count(self.inventory.entries().len())?;
        let measured = measure_fastpq_source_statement_usage(entries, transcripts, self.limits)?;
        self.inventory
            .verify_finalized_transcript_map(transcripts)?;
        Ok(FastpqSourceStatementAttemptV1 {
            usage: FastpqSourceStatementUsageV1::from_measured(entries, measured),
            budget: self,
            transcripts,
        })
    }
}

impl FastpqSourceStatementAttemptV1<'_, '_> {
    /// Exact prospective whole-inventory usage; not yet committed to the local budget.
    pub const fn usage(&self) -> FastpqSourceStatementUsageV1 {
        self.usage
    }

    /// Materialize using the current State context and atomically publish local usage on success.
    ///
    /// State ownership is rechecked after preparation. Slot and permission root come from
    /// the current block header and role table, never a supplied root. The unchanged strict
    /// producer rechecks complete public contents and construction limits. Only its successful
    /// return updates accounting. State/recorder/witness publication remains the caller's job.
    /// Permission-table traversal has its existing separate cost; the six source caps do not
    /// bound unrelated role-table size. This is not a source-finality or permission proof.
    ///
    /// # Errors
    /// Rejects stale/foreign/replay State ownership or any strict producer failure, leaving
    /// committed usage unchanged. No partially materialized output is returned on error.
    pub fn materialize(
        self,
        block: &StateBlock<'_>,
    ) -> Result<
        (
            FastpqOrdinarySourceStatementManifestV1,
            Vec<FastpqOrdinarySourceStatementLeafV1>,
        ),
        String,
    > {
        let output = (|| {
            self.budget.verify_current(block)?;
            let slot = block._curr_block.creation_time_ms.saturating_mul(1_000_000);
            let perm_root = crate::fastpq::permission_table_root(block.world.roles.iter());
            self.budget.inventory.derive_manifest(
                slot,
                perm_root,
                self.transcripts,
                self.budget.limits,
            )
        })();
        self.publish_if_success(output)
    }

    fn publish_if_success<T>(self, output: Result<T, String>) -> Result<T, String> {
        let output = output?;
        self.budget.committed = Some(self.usage);
        Ok(output)
    }
}

#[cfg(test)]
mod tests;

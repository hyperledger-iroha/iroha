//! Test-only qualification of complete local D7 facts without publishing capture state.
//!
//! Caller-supplied construction caps are explicit local inputs. This seam does not choose
//! production defaults, authenticate a source policy, or bypass proposal/mandatory-work
//! liveness requirements. The caller must finish execution and restore the final carrier
//! header before preparation, and retain/check the context before later publication.

use super::{FastpqSourceInventoryV1, StateBlock};
use crate::fastpq::FastpqSourceStatementBuildLimits;
use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqOrdinarySourceStatementLeafV1, FastpqOrdinarySourceStatementManifestV1,
    TransferTranscript,
};
use mv::storage::StorageReadOnly;
use std::{collections::BTreeMap, sync::Arc};

/// Locally owned context retained independently of the returned leaf allocation.
///
/// Private construction binds the original inventory allocation and the completed execution
/// role-table context. The manifest remains here after leaves move to the archive owner.
/// This is neither a wire object nor finality, policy or per-transfer authorization evidence.
/// Only D7 input context is retained: unrelated carrier-header fields, empty roles and
/// account-role membership are outside this record. Finality must bind the final block hash.
#[derive(Debug)]
pub(crate) struct OwnedFastpqD7CaptureContext {
    inventory: Arc<FastpqSourceInventoryV1>,
    creation_time_ms: u64,
    slot: u64,
    perm_root: [u8; 32],
    limits: FastpqSourceStatementBuildLimits,
    manifest: FastpqOrdinarySourceStatementManifestV1,
}

/// Complete bounded statement output prepared from the execution-owned public seal.
#[derive(Debug)]
pub(crate) struct PreparedOwnedFastpqD7Capture {
    context: OwnedFastpqD7CaptureContext,
    leaves: Vec<FastpqOrdinarySourceStatementLeafV1>,
}

impl PreparedOwnedFastpqD7Capture {
    /// Move the complete leaves while preserving their manifest in the retained context.
    pub(crate) fn into_parts(
        self,
    ) -> (
        FastpqOrdinarySourceStatementManifestV1,
        Vec<FastpqOrdinarySourceStatementLeafV1>,
        OwnedFastpqD7CaptureContext,
    ) {
        (self.context.manifest, self.leaves, self.context)
    }
}

impl OwnedFastpqD7CaptureContext {
    /// Original immutable execution inventory, including nontransfer identities.
    pub(crate) fn inventory(&self) -> &FastpqSourceInventoryV1 {
        &self.inventory
    }

    /// Exact carrier timestamp retained even when the derived slot saturates.
    pub(crate) const fn creation_time_ms(&self) -> u64 {
        self.creation_time_ms
    }

    /// Existing FASTPQ timestamp slot: carrier milliseconds multiplied by one million.
    pub(crate) const fn slot(&self) -> u64 {
        self.slot
    }

    /// Completed role-ID/permission/epoch table root; not account-membership authorization.
    pub(crate) const fn permission_root(&self) -> [u8; 32] {
        self.perm_root
    }

    /// Exact local construction limits used by this preparation, without policy authority.
    pub(crate) const fn limits(&self) -> FastpqSourceStatementBuildLimits {
        self.limits
    }

    /// Original complete manifest retained for later exact D7 value comparison.
    pub(crate) const fn manifest(&self) -> &FastpqOrdinarySourceStatementManifestV1 {
        &self.manifest
    }

    /// Check the retained execution source, exact timestamp and role-table context.
    ///
    /// This does not inspect a later witness, authenticate finality, or revalidate caller
    /// archive bytes. Future capture/extraction/commit integration must also compare its D7
    /// write and final transcript contents. A later authenticated-replay bookkeeping flag
    /// does not invalidate a record already prepared by ordinary reexecution.
    pub(crate) fn verify_current(&self, block: &StateBlock<'_>) -> Result<(), String> {
        let inventory = block.verified_fastpq_source_inventory_for_capture()?;
        if !Arc::ptr_eq(&self.inventory, &inventory) {
            return Err("FASTPQ prepared D7 inventory owner changed".into());
        }
        if block._curr_block.creation_time_ms != self.creation_time_ms {
            return Err("FASTPQ prepared D7 carrier timestamp changed".into());
        }
        if crate::fastpq::permission_table_root(block.world.roles.iter()) != self.perm_root {
            return Err("FASTPQ prepared D7 permission context changed".into());
        }
        Ok(())
    }
}

impl StateBlock<'_> {
    /// Prepare complete D7 facts from the final execution world and borrowed raw archive.
    ///
    /// The intended caller holds the execution-witness guard after joining all workers and
    /// passes the raw map borrowed by `drain_exec_witness_checked`. The StateBlock transcript
    /// map may already have been moved to the block; ownership comes from the retained exact
    /// source seal, never from an advertised archive. No transcript, private path, ordinary
    /// write or witness is cloned here. Whole-inventory reservation checks all six caps before statement/tree construction;
    /// its public measurement and exact seal precede successful usage publication. Permission-table scanning has its existing separate
    /// cost and is not bounded by transcript caps.
    ///
    /// The result is local only: no D7 write, recorder drain, cached context, sticky error or
    /// inventory mutation is published by this shared-borrow helper. Its future caller must
    /// latch errors and publish only after every capture check succeeds. Mandatory production
    /// wiring remains blocked on authenticated policy, packing/liveness and rollout decisions.
    /// TODO: integrate atomic D7 insertion and retained-context validation only after those
    /// authenticated-policy and deterministic execution/proposal-accounting gates are resolved.
    ///
    /// # Errors
    /// Rejects replay fabrication, absent/failed/stale ownership, changed public archives,
    /// exceeded caller limits, invalid full-domain statements or noncanonical final digests.
    pub(crate) fn prepare_owned_fastpq_d7_capture(
        &self,
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
        limits: FastpqSourceStatementBuildLimits,
    ) -> Result<PreparedOwnedFastpqD7Capture, String> {
        if self.authenticated_replay_commit {
            return Err("authenticated replay cannot prepare new ordinary FASTPQ D7 facts".into());
        }
        let inventory = self.verified_fastpq_source_inventory_for_capture()?;
        let mut budget = self.fastpq_source_statement_budget(limits)?;
        let attempt = budget.prepare(self, transcripts)?;
        let creation_time_ms = self._curr_block.creation_time_ms;
        // Preserve public_inputs_template_from_block's existing saturating timestamp units.
        let slot = creation_time_ms.saturating_mul(1_000_000);
        let perm_root = crate::fastpq::permission_table_root(self.world.roles.iter());
        let (manifest, leaves) = attempt.materialize(self)?;
        Ok(PreparedOwnedFastpqD7Capture {
            context: OwnedFastpqD7CaptureContext {
                inventory,
                creation_time_ms,
                slot,
                perm_root,
                limits,
                manifest,
            },
            leaves,
        })
    }
}

#[cfg(test)]
mod tests;

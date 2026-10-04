//! Test-only local transfer preparation and source-context drift diagnostics.
//!
//! No wire layout, D7 insertion or publication owner exists in this module.
use super::{FastpqSourceInventoryV1, StateBlock};
use crate::fastpq::FastpqSourceStatementBuildLimits;
use crate::fastpq::{TransferArchiveDiagnostic, TransferEntryDiagnostic};
use iroha_crypto::Hash;
use iroha_data_model::fastpq::TransferTranscript;
use mv::storage::StorageReadOnly;
use std::{collections::BTreeMap, sync::Arc};

/// Locally owned context retained independently of the returned leaf allocation.
///
/// Private construction binds the original inventory allocation and the completed execution
/// role-table context. The summary remains here after statements move to the archive owner.
/// This is neither a wire object nor finality, policy or per-transfer authorization evidence.
/// Only transfer diagnostic input context is retained: unrelated carrier-header fields, empty roles and
/// account-role membership are outside this record. Finality must bind the final block hash.
#[derive(Debug)]
pub(crate) struct OwnedTransferDiagnosticContext {
    inventory: Arc<FastpqSourceInventoryV1>,
    creation_time_ms: u64,
    slot: u64,
    perm_root: [u8; 32],
    limits: FastpqSourceStatementBuildLimits,
    summary: TransferArchiveDiagnostic,
}

/// Complete bounded statement output prepared from the execution-owned public seal.
#[derive(Debug)]
pub(crate) struct PreparedOwnedTransferDiagnostic {
    context: OwnedTransferDiagnosticContext,
    statements: Vec<TransferEntryDiagnostic>,
}

impl PreparedOwnedTransferDiagnostic {
    /// Move the complete statements while preserving their summary in the retained context.
    pub(crate) fn into_parts(
        self,
    ) -> (
        TransferArchiveDiagnostic,
        Vec<TransferEntryDiagnostic>,
        OwnedTransferDiagnosticContext,
    ) {
        (self.context.summary, self.statements, self.context)
    }
}

impl OwnedTransferDiagnosticContext {
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

    /// Original complete summary retained for later exact transfer diagnostic value comparison.
    pub(crate) const fn summary(&self) -> &TransferArchiveDiagnostic {
        &self.summary
    }

    /// Check the retained execution source, exact timestamp and role-table context.
    ///
    /// This does not inspect a later witness, authenticate finality, or revalidate caller
    /// archive bytes. Future capture/extraction/commit integration must also compare its transfer diagnostic
    /// write and final transcript contents. A later authenticated-replay bookkeeping flag
    /// does not invalidate a record already prepared by ordinary reexecution.
    pub(crate) fn verify_current(&self, block: &StateBlock<'_>) -> Result<(), String> {
        let inventory = block.verified_fastpq_source_inventory_for_capture()?;
        if !Arc::ptr_eq(&self.inventory, &inventory) {
            return Err("FASTPQ prepared transfer diagnostic inventory owner changed".into());
        }
        if block._curr_block.creation_time_ms != self.creation_time_ms {
            return Err("FASTPQ prepared transfer diagnostic carrier timestamp changed".into());
        }
        if crate::fastpq::permission_table_root(block.world.roles.iter()) != self.perm_root {
            return Err("FASTPQ prepared transfer diagnostic permission context changed".into());
        }
        Ok(())
    }
}

impl StateBlock<'_> {
    /// Test-only live-transfer preparation against the retained public archive seal.
    /// The returned summary cannot be encoded, inserted as D7 or used as proof authority.
    pub(crate) fn prepare_owned_transfer_diagnostic(
        &self,
        transcripts: &BTreeMap<Hash, Vec<TransferTranscript>>,
        limits: FastpqSourceStatementBuildLimits,
    ) -> Result<PreparedOwnedTransferDiagnostic, String> {
        let inventory = self.verified_fastpq_source_inventory_for_capture()?;
        let mut budget = self.fastpq_source_statement_budget(limits)?;
        let attempt = budget.prepare(self, transcripts)?;
        let creation_time_ms = self._curr_block.creation_time_ms;
        // Preserve public_inputs_template_from_block's existing saturating timestamp units.
        let slot = creation_time_ms.saturating_mul(1_000_000);
        let perm_root = crate::fastpq::permission_table_root(self.world.roles.iter());
        let (summary, statements) = attempt.materialize_diagnostic(self)?;
        Ok(PreparedOwnedTransferDiagnostic {
            context: OwnedTransferDiagnosticContext {
                inventory,
                creation_time_ms,
                slot,
                perm_root,
                limits,
                summary,
            },
            statements,
        })
    }
}

#[cfg(test)]
mod tests;

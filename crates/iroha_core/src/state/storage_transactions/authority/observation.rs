//! Nonpublishing committed membership under the original writer and reader cut.
//!
//! No history slot, publication identity, staged tip or pool is created here.
//! The internal block reuses the sole committed visitor implementation; only
//! borrowed cuts and the original surface can leave this observation.

use super::super::{
    MembershipAdmissionError, TransactionsBlock, TransactionsPublicationSurface,
    TransactionsStorage, block::MembershipWriter, history,
};
use super::{TransactionMembershipAuthorityError, TransactionMembershipCut};

/// Same-original-writer cold borrower with no successor-publication interface.
#[must_use = "retain the original membership observation for the complete read"]
pub(in crate::state) struct CommittedMembershipObservation<'storage> {
    block: TransactionsBlock<'storage>,
    // Last: payloads and writer notices retire before original reader callbacks.
    _reader_releases: concread::release::DeferredReleaseBatch,
}

impl<'storage> CommittedMembershipObservation<'storage> {
    pub(super) fn try_new(
        storage: &'storage TransactionsStorage,
    ) -> Result<Self, MembershipAdmissionError> {
        // Declared before the writer, so every failed reader probe first frees
        // the original writer and only then dispatches any acquired reader notice.
        let mut reader_releases = storage.blocks.reader_release_batch();
        let writer_wait = storage.released.observe();
        let guard = storage
            .write_lock
            .try_lock()
            .ok_or(MembershipAdmissionError::Busy(writer_wait))?;
        let writer = MembershipWriter::new(storage.released.guard(guard), None);
        let reader_wait = storage.blocks.observe_reader_release();
        let baseline = storage
            .blocks
            .try_read_retaining(&mut reader_releases)
            .map_err(|error| history::physical_error(error, reader_wait))?;
        Ok(Self {
            block: TransactionsBlock {
                latest_block_ref: &storage.latest_block,
                budget_ref: &storage.budget,
                blocks_ref: &storage.blocks,
                baseline,
                publication_sequence: &storage.publication_sequence,
                _guard: writer,
                revert: false,
                current_block: None,
            },
            _reader_releases: reader_releases,
        })
    }

    /// Admit both complete committed streams before any row encoding.
    pub(in crate::state) fn membership_authority_cut(
        &self,
        max_row_visits: usize,
    ) -> Result<TransactionMembershipCut<'_, 'storage>, TransactionMembershipAuthorityError> {
        self.block.membership_authority_cut(max_row_visits)
    }

    /// Retain original source identity without creating publication authority.
    pub(in crate::state) fn publication_surface(&self) -> TransactionsPublicationSurface {
        self.block.publication_surface()
    }
}

impl Drop for CommittedMembershipObservation<'_> {
    fn drop(&mut self) {
        // Explicit release precedes historical payload refunds and both original
        // notification families, including during codec errors and unwinding.
        self.block.release_writers();
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;

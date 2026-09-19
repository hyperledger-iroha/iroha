//! Publication of the original admitted hash vector under its exact writer.

use super::*;

/// A detached hash journal whose exact predecessor is held for publication.
///
/// Installation admission is released after all physical writers on abort or
/// drop, and is returned to the aggregate owner after successful publication.
pub(crate) struct PreparedBlockHashes<'target, Installation> {
    journal: DetachedBlockHashes,
    guard: mv::ReleaseGuard<'target, parking_lot::RwLockWriteGuard<'target, BlockHashStorage>>,
    next_publication: Arc<BlockHashPublication>,
    committed_height: &'target AtomicUsize,
    installation: Installation,
}

impl DetachedBlockHashes {
    /// Admit installation before taking the writer, retaining the original journal.
    ///
    /// No chain is copied: the original complete visible vector becomes the
    /// committed vector. All refusals return the same journal, including when
    /// admission races another writer. The callback owns resource policy; this
    /// component grants no authority to publish a State or a finalized block.
    pub(crate) fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target BlockHashes,
        admit: impl FnOnce(&Self, &BlockHashes) -> Result<Installation, E>,
    ) -> Result<
        PreparedBlockHashes<'target, Installation>,
        (Self, mv::PublicationPreparationError<E>),
    > {
        if !Arc::ptr_eq(&self.owner, &target.owner) {
            return Err((self, mv::PublicationPreparationError::Changed));
        }
        {
            let wait = target.released.observe();
            let Some(guard) = target.inner.try_read() else {
                return Err((
                    self,
                    mv::PublicationPreparationError::after_failed_acquisition(wait),
                ));
            };
            let guard = target.released.guard(guard);
            if !matches!(&**guard, BlockHashStorage::Owned { publication, .. }
                if Arc::ptr_eq(&self.publication, publication))
            {
                return Err((self, mv::PublicationPreparationError::Changed));
            }
        }
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, mv::PublicationPreparationError::Admission(error))),
        };
        let next_publication = Arc::new(BlockHashPublication);
        let wait = target.released.observe();
        let Some(guard) = target.inner.try_write() else {
            return Err((
                self,
                mv::PublicationPreparationError::after_failed_acquisition(wait),
            ));
        };
        let guard = target.released.guard(guard);
        if !matches!(&**guard, BlockHashStorage::Owned { publication, .. }
            if Arc::ptr_eq(&self.publication, publication))
        {
            drop(guard);
            return Err((self, mv::PublicationPreparationError::Changed));
        }
        Ok(PreparedBlockHashes {
            journal: self,
            guard,
            next_publication,
            committed_height: &target.committed_height,
            installation,
        })
    }
}

impl<Installation> PreparedBlockHashes<'_, Installation> {
    /// Release the writer before returning the exact original journal for retry.
    pub(crate) fn abort(self) -> DetachedBlockHashes {
        let Self {
            journal,
            guard,
            next_publication,
            committed_height: _,
            installation,
        } = self;
        drop(guard);
        drop(next_publication);
        drop(installation);
        journal
    }

    /// Install the original vector once and return its resource admission.
    ///
    /// The caller must retain all other component writers and the complete
    /// State/finality authorization before consuming any prepared component.
    pub(crate) fn publish(self) -> Installation {
        let Self {
            journal,
            mut guard,
            next_publication,
            committed_height,
            installation,
        } = self;
        let height = journal.visible.len();
        **guard = BlockHashStorage::Owned {
            hashes: journal.visible,
            publication: next_publication,
        };
        committed_height.store(height, Ordering::Release);
        drop(guard);
        installation
    }
}

#[cfg(test)]
#[path = "block_hashes_publication_tests.rs"]
mod tests;

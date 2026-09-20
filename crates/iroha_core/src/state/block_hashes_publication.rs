//! Exact predecessor publication of the original shared hash generation.

use super::*;
use concread::bptree::{BptreeMapPreparedCommit, OwnedWriteError};

/// Exact original successor and both physical publication locks.
/// Field order releases locks before notification or caller installation custody.
pub(crate) struct PreparedBlockHashes<'target, Installation> {
    owner: NativeLaneStateOwner,
    prepared: BptreeMapPreparedCommit<'target, usize, HashOf<BlockHeader>, BlockHashMode>,
    notification: mv::ReleaseGuard<'target, ()>,
    mode: mv::BlockMode,
    visible_len: usize,
    height: usize,
    committed_height: &'target AtomicUsize,
    installation: Installation,
}
fn refusal<E>(error: OwnedWriteError, wait: mv::ReleaseWait) -> mv::PublicationPreparationError<E> {
    match error {
        OwnedWriteError::Changed => mv::PublicationPreparationError::Changed,
        OwnedWriteError::Poisoned => mv::PublicationPreparationError::Poisoned,
        OwnedWriteError::Busy => mv::PublicationPreparationError::after_failed_acquisition(wait),
    }
}
impl DetachedBlockHashes {
    /// Admit installation and reacquire the exact original tree predecessor.
    /// Every refusal retains the same private nodes; readers never veto publication.
    pub(crate) fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target BlockHashes,
        admit: impl FnOnce(&Self, &BlockHashes) -> Result<Installation, E>,
    ) -> Result<
        PreparedBlockHashes<'target, Installation>,
        (Self, mv::PublicationPreparationError<E>),
    > {
        if self.reserved_tip.is_some() {
            return Err((self, mv::PublicationPreparationError::Changed));
        }
        let Some(map) = target.map() else {
            return Err((self, mv::PublicationPreparationError::Changed));
        };
        let wait = target.released.observe();
        match self.observe_current(target) {
            Ok(true) => {}
            Ok(false) => return Err((self, mv::PublicationPreparationError::Changed)),
            Err(error) => return Err((self, refusal(error, wait))),
        }
        let installation = match admit(&self, target) {
            Ok(value) => value,
            Err(error) => return Err((self, mv::PublicationPreparationError::Admission(error))),
        };
        let height = self.len();
        let Self {
            work,
            mode,
            visible_len,
            reserved_tip,
        } = self;
        let wait = target.released.observe();
        let writer = match map.try_write_owned(work) {
            Ok(writer) => writer,
            Err((work, error)) => {
                if error == OwnedWriteError::Changed {
                    drop(target.released.guard(()));
                }
                return Err((
                    Self {
                        work,
                        mode,
                        visible_len,
                        reserved_tip,
                    },
                    refusal(error, wait),
                ));
            }
        };
        let notification = target.released.guard(());
        let prepared = match writer.try_prepare_commit() {
            Ok(prepared) => prepared,
            Err((writer, error)) => {
                let work = writer.detach();
                drop(notification);
                return Err((
                    Self {
                        work,
                        mode,
                        visible_len,
                        reserved_tip,
                    },
                    refusal(error, wait),
                ));
            }
        };
        Ok(PreparedBlockHashes {
            owner: NativeLaneStateOwner(map.family()),
            prepared,
            notification,
            mode,
            visible_len,
            height,
            committed_height: &target.committed_height,
            installation,
        })
    }
}
impl<'target, Installation> PreparedBlockHashes<'target, Installation> {
    pub(crate) fn state_owner(&self) -> NativeLaneStateOwner {
        self.owner.clone()
    }
    /// Release physical locks and return the same private tree for retry.
    pub(crate) fn abort(self) -> DetachedBlockHashes {
        let Self {
            prepared,
            notification,
            mode,
            visible_len,
            installation,
            ..
        } = self;
        let work = prepared.abort().detach();
        drop(notification);
        drop(installation);
        DetachedBlockHashes {
            work,
            mode,
            visible_len,
            reserved_tip: None,
        }
    }
    /// Publish without allocation; retain cleanup until aggregate fences release.
    pub(crate) fn publish(self) -> PublishedBlockHashes<'target, Installation> {
        let Self {
            prepared,
            notification,
            height,
            committed_height,
            installation,
            ..
        } = self;
        let published = prepared.publish();
        committed_height.store(height, Ordering::Release);
        let retirement = published.release();
        PublishedBlockHashes {
            _retirement: retirement,
            _notification: notification,
            _installation: installation,
        }
    }
}
/// Released tree cleanup and wake custody. Drop after other publication fences.
pub(crate) struct PublishedBlockHashes<'a, Installation> {
    _retirement:
        concread::bptree::BptreeMapCommitRetirement<usize, HashOf<BlockHeader>, BlockHashMode>,
    _notification: mv::ReleaseGuard<'a, ()>,
    _installation: Installation,
}

#[cfg(test)]
#[path = "block_hashes_publication_tests.rs"]
mod tests;

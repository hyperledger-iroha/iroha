//! Exact predecessor publication of the original shared hash generation.

use super::*;
use concread::bptree::{BptreeMapPreparedCommit, OwnedWriteError};

/// Exact original successor and both physical publication locks.
/// Field order releases locks before notification or caller installation custody.
pub(crate) struct PreparedBlockHashes<'target, Installation> {
    owner: NativeLaneStateOwner,
    prepared: concread::release::ReleaseGuard<
        'target,
        BptreeMapPreparedCommit<'target, usize, HashOf<BlockHeader>, BlockHashMode>,
    >,
    mode: mv::BlockMode,
    visible_len: usize,
    height: usize,
    committed_height: &'target AtomicUsize,
    installation: Installation,
}
/// Original abort notifications and resources after the hash writer unlocks.
pub(crate) struct AbortedBlockHashes<Installation> {
    _owner: NativeLaneStateOwner,
    _release: [concread::release::DeferredRelease; 2],
    _installation: Installation,
}

/// Original writer notification and installation after a local refusal.
/// Retain this cleanup until every enclosing physical fence has unlocked.
#[must_use = "retain hash refusal cleanup through enclosing publication fences"]
pub(crate) struct RefusedBlockHashes<Installation> {
    _release: Option<concread::release::DeferredRelease>,
    _installation: Option<Installation>,
}

fn refusal<E>(
    error: OwnedWriteError,
    wait: concread::release::ReleaseWait,
) -> mv::PublicationPreparationError<E> {
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
        (
            Self,
            mv::PublicationPreparationError<E>,
            RefusedBlockHashes<Installation>,
        ),
    > {
        let mut cleanup = RefusedBlockHashes {
            _release: None,
            _installation: None,
        };
        if self.reserved_tip.is_some() {
            return Err((self, mv::PublicationPreparationError::Changed, cleanup));
        }
        let Some(map) = target.map() else {
            return Err((self, mv::PublicationPreparationError::Changed, cleanup));
        };
        let wait = map.observe_reader_release();
        match self.observe_current(target) {
            Ok(true) => {}
            Ok(false) => return Err((self, mv::PublicationPreparationError::Changed, cleanup)),
            Err(error) => return Err((self, refusal(error, wait), cleanup)),
        }
        let installation = match admit(&self, target) {
            Ok(value) => value,
            Err(error) => {
                return Err((
                    self,
                    mv::PublicationPreparationError::Admission(error),
                    cleanup,
                ));
            }
        };
        cleanup._installation = Some(installation);
        let height = self.len();
        let Self {
            work,
            mode,
            visible_len,
            reserved_tip,
        } = self;
        let wait = target.released.observe();
        let acquired = match map.try_acquire_owned(work) {
            Ok(acquired) => target.released.poisoning_guard(acquired),
            Err((work, error)) => {
                return Err((
                    Self {
                        work,
                        mode,
                        visible_len,
                        reserved_tip,
                    },
                    refusal(error, wait),
                    cleanup,
                ));
            }
        };
        let writer = match acquired.try_map_preserving_release(|acquired| acquired.validate()) {
            Ok(writer) => writer,
            Err((acquired, error)) => {
                let (work, released) = acquired.release_deferred(|acquired| acquired.abort());
                cleanup._release = Some(released);
                return Err((
                    Self {
                        work,
                        mode,
                        visible_len,
                        reserved_tip,
                    },
                    refusal(error, wait),
                    cleanup,
                ));
            }
        };
        let wait = map.observe_reader_release();
        let prepared = match writer.try_map_preserving_release(|writer| writer.try_prepare_commit())
        {
            Ok(prepared) => prepared,
            Err((writer, error)) => {
                let (work, released) = writer.release_deferred(|writer| writer.detach());
                cleanup._release = Some(released);
                return Err((
                    Self {
                        work,
                        mode,
                        visible_len,
                        reserved_tip,
                    },
                    refusal(error, wait),
                    cleanup,
                ));
            }
        };
        Ok(PreparedBlockHashes {
            owner: NativeLaneStateOwner(map.family()),
            prepared,
            mode,
            visible_len,
            height,
            committed_height: &target.committed_height,
            installation: cleanup
                ._installation
                .take()
                .expect("original hash installation"),
        })
    }
}
impl<'target, Installation> PreparedBlockHashes<'target, Installation> {
    pub(crate) fn state_owner(&self) -> NativeLaneStateOwner {
        self.owner.clone()
    }
    /// Release physical locks and return the same private tree for retry.
    pub(crate) fn abort(self) -> (DetachedBlockHashes, AbortedBlockHashes<Installation>) {
        let Self {
            owner,
            prepared,
            mode,
            visible_len,
            installation,
            ..
        } = self;
        let ((work, reader), writer) = prepared.release_deferred(|prepared| {
            let (writer, reader) = prepared.abort_retaining();
            (writer.detach(), reader)
        });
        let retirement = AbortedBlockHashes {
            _owner: owner,
            _release: [reader, writer],
            _installation: installation,
        };
        (
            DetachedBlockHashes {
                work,
                mode,
                visible_len,
                reserved_tip: None,
            },
            retirement,
        )
    }
    /// Publish without allocation; retain cleanup until aggregate fences release.
    pub(crate) fn publish(self) -> PublishedBlockHashes<'target, Installation> {
        let Self {
            prepared,
            height,
            committed_height,
            installation,
            ..
        } = self;
        let published = prepared.map_preserving_release(|prepared| prepared.publish());
        committed_height.store(height, Ordering::Release);
        let retirement = published.release_retaining(|published| published.release());
        PublishedBlockHashes {
            _retirement: retirement,
            _installation: installation,
        }
    }
}
/// Released tree cleanup and wake custody. Drop after other publication fences.
pub(crate) struct PublishedBlockHashes<'a, Installation> {
    _retirement: concread::release::ReleaseGuard<
        'a,
        concread::bptree::BptreeMapCommitRetirement<usize, HashOf<BlockHeader>, BlockHashMode>,
    >,
    _installation: Installation,
}

#[cfg(test)]
#[path = "block_hashes_publication_tests.rs"]
mod tests;

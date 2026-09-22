//! Complete physical preparation and deferred cleanup of one original map pair.

use super::*;
use crate::publication::{IdentityRetirement, PreparedIdentity};
use concread::{
    bptree::{BptreeMapCommitRetirement, BptreeMapCommitSlot},
    release::DeferredRelease,
};

/// Join notifications only to an actually acquired original physical writer.
#[cfg(test)]
pub(super) fn acquire_owned_writer<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>>(
    map: &'a BptreeMap<K, V, M>,
    released: &'a ReleaseNotification,
    owned: BptreeMapOwned<K, V, M>,
) -> Result<
    ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>>,
    (
        BptreeMapOwned<K, V, M>,
        OwnedWriteError,
        Option<DeferredRelease>,
    ),
> {
    let acquired = map
        .try_acquire_owned(owned)
        .map_err(|(owned, error)| (owned, error, None))?;
    released
        .poisoning_guard(acquired)
        .try_map_preserving_release(|acquired| acquired.validate())
        .map_err(|(acquired, error)| {
            let (owned, notification) = acquired.release_deferred(|acquired| acquired.abort());
            (owned, error, Some(notification))
        })
}

enum MapStage<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> {
    Held(ReleaseGuard<'a, BptreeMapCommitSlot<'a, K, V, M>>),
    Released {
        _owner: BptreeMapAbandonment<K, V, M>,
        _releases: MapReleases,
    },
}

pub(super) struct MapReleases {
    _reader: Option<DeferredRelease>,
    _writer: DeferredRelease,
}

impl<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> MapStage<'a, K, V, M> {
    fn new(writer: ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>>) -> Self {
        Self::Held(writer.map_preserving_release(|writer| writer.commit_slot()))
    }

    fn prepare(&mut self) -> Result<(), OwnedWriteError> {
        let Self::Held(slot) = self else {
            panic!("original map was released")
        };
        slot.try_prepare()
    }

    fn prepare_blocking(&mut self) {
        let Self::Held(slot) = self else {
            panic!("original map was released")
        };
        slot.prepare();
    }

    fn is_prepared(&self) -> bool {
        matches!(self, Self::Held(slot) if slot.is_prepared())
    }

    fn into_held(self) -> ReleaseGuard<'a, BptreeMapCommitSlot<'a, K, V, M>> {
        match self {
            Self::Held(slot) => slot,
            Self::Released { .. } => panic!("terminal release grants no publication"),
        }
    }

    fn abort(self) -> (BptreeMapOwned<K, V, M>, MapReleases) {
        let ((owner, reader), writer) = self.into_held().release_deferred(|slot| {
            let (writer, reader) = slot.abort_retaining();
            (writer.detach(), reader)
        });
        (
            owner,
            MapReleases {
                _reader: reader,
                _writer: writer,
            },
        )
    }

    fn release(phase: &mut Option<Self>) {
        if !matches!(phase, Some(Self::Held(_))) {
            return;
        }
        let ((owner, reader), writer) = phase
            .take()
            .expect("original held map")
            .into_held()
            .release_deferred(|slot| {
                let (writer, reader) = slot.abort_retaining();
                // A failed private cursor is cleanup custody, never a journal.
                (writer.abort_retaining(), reader)
            });
        *phase = Some(Self::Released {
            _owner: owner,
            _releases: MapReleases {
                _reader: reader,
                _writer: writer,
            },
        });
    }
}

/// Original published nodes, unchanged private work and physical release signals.
/// It holds no locks or publication authority. Drop only after the enclosing
/// aggregate's physical fences; original admission must outlive this cleanup.
pub struct PublicationRetirement<K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    _blocks: Option<BptreeMapCommitRetirement<K, V, M>>,
    _revert: BptreeMapCommitRetirement<K, Option<V>, M>,
    _unchanged: Option<BptreeMapOwned<K, V, M>>,
    _blocks_release: DeferredRelease,
    _revert_release: DeferredRelease,
    _identity: IdentityRetirement,
    _probe: Option<IdentityRetirement>,
}

pub(super) struct PreparedStorageWriters<'a, K: Key, V: Value, M: StorageMode<K, V>> {
    revert: Option<MapStage<'a, K, Option<V>, M>>,
    blocks: Option<MapStage<'a, K, V, M>>,
    identity: Option<PreparedIdentity<'a>>,
    probe: Option<IdentityRetirement>,
    refused_identity: Option<IdentityRetirement>,
    target: &'a Storage<K, V, M>,
    complete: bool,
    started: bool,
    released: bool,
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> PreparedStorageWriters<'a, K, V, M> {
    pub(super) fn new(
        writers: StorageWriters<'a, K, V, M>,
        probe: Option<IdentityRetirement>,
    ) -> Self {
        let target = writers.target;
        Self::from_original(target, writers.into_original(), probe)
    }

    pub(super) fn from_original(
        target: &'a Storage<K, V, M>,
        original: OriginalWriters<'a, K, V, M>,
        probe: Option<IdentityRetirement>,
    ) -> Self {
        let OriginalWriters { revert, blocks } = original;
        Self {
            revert: Some(MapStage::new(revert)),
            blocks: Some(MapStage::new(blocks)),
            identity: None,
            probe,
            refused_identity: None,
            complete: false,
            started: false,
            released: false,
            target,
        }
    }

    pub(super) fn prepare<E>(
        &mut self,
        predecessor: &CapturedPublication,
        dirty: bool,
    ) -> Result<(), PublicationPreparationError<E>> {
        fn refusal<E>(
            error: OwnedWriteError,
            wait: crate::ReleaseWait,
        ) -> PublicationPreparationError<E> {
            match error {
                OwnedWriteError::Busy => {
                    PublicationPreparationError::after_failed_acquisition(wait)
                }
                OwnedWriteError::Poisoned => PublicationPreparationError::Poisoned,
                OwnedWriteError::Changed => PublicationPreparationError::Changed,
            }
        }
        assert!(
            !self.started && !self.released,
            "original pair prepares once"
        );
        self.started = true;
        let wait = self.target.revert.observe_reader_release();
        self.revert
            .as_mut()
            .expect("original undo")
            .prepare()
            .map_err(|error| refusal(error, wait))?;
        if dirty {
            let wait = self.target.blocks.observe_reader_release();
            self.blocks
                .as_mut()
                .expect("original current")
                .prepare()
                .map_err(|error| refusal(error, wait))?;
        }
        match predecessor.try_prepare_current(&self.target.publication) {
            Ok(identity) => self.identity = Some(identity),
            Err((error, retirement)) => {
                self.refused_identity = retirement;
                return Err(error);
            }
        }
        self.complete = true;
        Ok(())
    }

    /// Block without releasing caller custody; ordinary commit preserves its
    /// original lock policy and performs no new writer acquisition or allocation.
    pub(super) fn prepare_attached(&mut self, predecessor: &CapturedPublication, dirty: bool) {
        assert!(
            !self.started && !self.released,
            "original pair prepares once"
        );
        self.started = true;
        self.revert
            .as_mut()
            .expect("original undo")
            .prepare_blocking();
        if dirty {
            self.blocks
                .as_mut()
                .expect("original current")
                .prepare_blocking();
        }
        predecessor.prepare_current_in(&self.target.publication, &mut self.identity);
        self.complete = true;
    }

    pub(super) fn is_prepared(&self) -> bool {
        self.complete && !self.released
    }

    /// Terminally release physical owners while retaining exact cleanup in place.
    /// Failed cursors use abandonment and cannot become readable detached journals.
    pub(super) fn release(&mut self) {
        if self.released {
            return;
        }
        MapStage::release(&mut self.blocks);
        MapStage::release(&mut self.revert);
        if let Some(identity) = self.identity.take() {
            debug_assert!(self.refused_identity.is_none());
            self.refused_identity = Some(identity.abort());
        }
        self.complete = false;
        self.released = true;
    }

    pub(super) fn abort<I>(
        mut self,
        installation: I,
    ) -> (
        BptreeMapOwned<K, V, M>,
        BptreeMapOwned<K, Option<V>, M>,
        PublicationCleanup<I>,
    ) {
        assert!(!self.released, "terminal release grants no journal");
        let (blocks, block_releases) = self.blocks.take().expect("original current").abort();
        let (revert, revert_releases) = self.revert.take().expect("original undo").abort();
        let identity = self
            .identity
            .take()
            .map(PreparedIdentity::abort)
            .or(self.refused_identity.take());
        // All raw locks have released, including identity. Their original
        // signals remain owned until the enclosing aggregate has unlocked.
        (
            blocks,
            revert,
            PublicationCleanup {
                _readers: [block_releases._reader, revert_releases._reader],
                _writers: [Some(block_releases._writer), Some(revert_releases._writer)],
                writer_batches: [None, None],
                identities: [self.probe.take(), identity],
                installation: Some(installation),
            },
        )
    }

    pub(super) fn publish(mut self, next: NextPublication) -> PublicationRetirement<K, V, M> {
        assert!(
            self.complete && !self.released,
            "original pair preparation must complete"
        );
        let blocks_prepared = self
            .blocks
            .as_ref()
            .expect("original current")
            .is_prepared();
        assert!(self.revert.as_ref().expect("original undo").is_prepared());
        let blocks = self.blocks.take().expect("original current").into_held();
        let revert = self.revert.take().expect("original undo").into_held();
        let identity = self.identity.take().expect("original prepared identity");
        let ((blocks, revert, unchanged, blocks_release, revert_release), identity) = identity
            .publish_retaining(
                next,
                || {
                    let (blocks, unchanged) = if blocks_prepared {
                        (
                            Some(
                                blocks
                                    .map_preserving_release(|slot| slot.into_prepared().publish()),
                            ),
                            None,
                        )
                    } else {
                        (None, Some(blocks))
                    };
                    let revert =
                        revert.map_preserving_release(|slot| slot.into_prepared().publish());
                    (blocks, revert, unchanged)
                },
                |(blocks, revert, unchanged)| {
                    let (blocks, unchanged, blocks_release) = match (blocks, unchanged) {
                        (Some(blocks), None) => {
                            let (retirement, release) = blocks.release_deferred(|p| p.release());
                            (Some(retirement), None, release)
                        }
                        (None, Some(unchanged)) => {
                            let (owner, release) = unchanged.release_deferred(|slot| {
                                let (writer, reader) = slot.abort_retaining();
                                debug_assert!(
                                    reader.is_none(),
                                    "unchanged writer was not prepared"
                                );
                                writer.detach()
                            });
                            (None, Some(owner), release)
                        }
                        _ => unreachable!("one original current owner"),
                    };
                    let (revert, revert_release) = revert.release_deferred(|p| p.release());
                    (blocks, revert, unchanged, blocks_release, revert_release)
                },
            );
        PublicationRetirement {
            _blocks: blocks,
            _revert: revert,
            _unchanged: unchanged,
            _blocks_release: blocks_release,
            _revert_release: revert_release,
            _identity: identity,
            _probe: self.probe.take(),
        }
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> Drop for PreparedStorageWriters<'_, K, V, M> {
    fn drop(&mut self) {
        self.release();
    }
}

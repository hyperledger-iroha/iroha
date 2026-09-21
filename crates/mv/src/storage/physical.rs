//! Complete physical preparation and deferred cleanup of one original map pair.

use super::*;
use crate::publication::{IdentityRetirement, PreparedIdentity};
use concread::{
    bptree::{BptreeMapCommitRetirement, BptreeMapPreparedCommit},
    release::DeferredRelease,
};

enum MapStage<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> {
    Writer(ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>>),
    Prepared(ReleaseGuard<'a, BptreeMapPreparedCommit<'a, K, V, M>>),
}

pub(super) struct MapReleases {
    _reader: Option<DeferredRelease>,
    _writer: DeferredRelease,
}

impl<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> MapStage<'a, K, V, M> {
    fn prepare(self) -> Result<Self, (Self, OwnedWriteError)> {
        let Self::Writer(writer) = self else {
            unreachable!("original writer prepares only once")
        };
        writer
            .try_map_preserving_release(|writer| writer.try_prepare_commit())
            .map(Self::Prepared)
            .map_err(|(writer, error)| (Self::Writer(writer), error))
    }

    fn abort(self) -> (BptreeMapOwned<K, V, M>, MapReleases) {
        let ((owner, reader), writer) = match self {
            Self::Writer(writer) => writer.release_deferred(|writer| (writer.detach(), None)),
            Self::Prepared(prepared) => prepared.release_deferred(|prepared| {
                let (writer, reader) = prepared.abort_retaining();
                (writer.detach(), Some(reader))
            }),
        };
        (
            owner,
            MapReleases {
                _reader: reader,
                _writer: writer,
            },
        )
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
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> PreparedStorageWriters<'a, K, V, M> {
    pub(super) fn new(
        writers: StorageWriters<'a, K, V, M>,
        probe: Option<IdentityRetirement>,
    ) -> Self {
        let target = writers.target;
        let OriginalWriters { revert, blocks } = writers.into_original();
        Self {
            revert: Some(MapStage::Writer(revert)),
            blocks: Some(MapStage::Writer(blocks)),
            identity: None,
            probe,
            refused_identity: None,
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
        let wait = self.target.revert.observe_reader_release();
        self.revert = Some(match self.revert.take().expect("original undo").prepare() {
            Ok(prepared) => prepared,
            Err((writer, error)) => {
                self.revert = Some(writer);
                return Err(refusal(error, wait));
            }
        });
        if dirty {
            let wait = self.target.blocks.observe_reader_release();
            self.blocks = Some(
                match self.blocks.take().expect("original current").prepare() {
                    Ok(prepared) => prepared,
                    Err((writer, error)) => {
                        self.blocks = Some(writer);
                        return Err(refusal(error, wait));
                    }
                },
            );
        }
        match predecessor.try_prepare_current(&self.target.publication) {
            Ok(identity) => self.identity = Some(identity),
            Err((error, retirement)) => {
                self.refused_identity = retirement;
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) fn abort<I>(
        mut self,
        installation: I,
    ) -> (
        BptreeMapOwned<K, V, M>,
        BptreeMapOwned<K, Option<V>, M>,
        PublicationCleanup<I>,
    ) {
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
                writers: [Some(block_releases._writer), Some(revert_releases._writer)],
                identities: [self.probe.take(), identity],
                installation: Some(installation),
            },
        )
    }

    pub(super) fn publish(mut self, next: NextPublication) -> PublicationRetirement<K, V, M> {
        let blocks = self.blocks.take().expect("original current");
        let MapStage::Prepared(revert) = self.revert.take().expect("original undo") else {
            unreachable!("undo preparation precedes publication")
        };
        let identity = self.identity.take().expect("original prepared identity");
        let ((blocks, revert, unchanged, blocks_release, revert_release), identity) = identity
            .publish_retaining(
                next,
                || {
                    let (blocks, unchanged) = match blocks {
                        MapStage::Prepared(blocks) => {
                            (Some(blocks.map_preserving_release(|p| p.publish())), None)
                        }
                        MapStage::Writer(blocks) => (None, Some(blocks)),
                    };
                    let revert = revert.map_preserving_release(|p| p.publish());
                    (blocks, revert, unchanged)
                },
                |(blocks, revert, unchanged)| {
                    let (blocks, unchanged, blocks_release) = match (blocks, unchanged) {
                        (Some(blocks), None) => {
                            let (retirement, release) = blocks.release_deferred(|p| p.release());
                            (Some(retirement), None, release)
                        }
                        (None, Some(unchanged)) => {
                            let (owner, release) = unchanged.release_deferred(|w| w.detach());
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
        // Keep private payloads until every physical lock is gone. Struct-field
        // drop would otherwise notify the first reader while the second is held.
        let blocks = self.blocks.take().map(MapStage::abort);
        let revert = self.revert.take().map(MapStage::abort);
        let identity = self
            .identity
            .take()
            .map(PreparedIdentity::abort)
            .or(self.refused_identity.take());
        drop((blocks, revert, identity, self.probe.take()));
    }
}

//! Prepared EBR pairs retain their exact identity and defer all retirement.

use super::*;
use crate::publication::{IdentityRetirement, PreparedIdentity};
use concread::{
    ebrcell::{EbrCellPreparedCommit, EbrCellRetirement},
    release::DeferredRelease,
};

enum CellStage<'a, V: Value, C: Send + Sync + 'static> {
    Writer(CellWriter<'a, V, C>),
    Prepared(ReleaseGuard<'a, EbrCellPreparedCommit<'a, V, C>>),
}

impl<'a, V: Value, C: Send + Sync + 'static> CellStage<'a, V, C> {
    fn prepare(self) -> Self {
        let Self::Writer(writer) = self else {
            unreachable!("original cell prepares once")
        };
        Self::Prepared(writer.map_preserving_release(|writer| writer.prepare_commit()))
    }

    fn abort(self) -> (EbrCellOwned<V, C>, DeferredRelease) {
        match self {
            Self::Writer(writer) => writer.release_deferred(|writer| writer.detach()),
            Self::Prepared(prepared) => {
                prepared.release_deferred(|prepared| prepared.abort().detach())
            }
        }
    }
}

pub(super) struct PreparedCellWriters<'a, V: Value, C: Send + Sync + 'static> {
    revert: Option<CellStage<'a, Option<V>, C>>,
    blocks: Option<CellStage<'a, V, C>>,
    identity: Option<PreparedIdentity<'a>>,
    probe: Option<IdentityRetirement>,
    refused_identity: Option<IdentityRetirement>,
}

/// Published EBR generations and their original capture/installation resources.
/// It owns no physical lock. Keep it through the enclosing publication fences;
/// collector work and release callbacks run only when this owner is retired.
pub struct PublishedPublication<
    V: Value,
    Admission,
    Installation,
    C: Send + Sync + 'static = Untracked,
> {
    _blocks: Option<EbrCellRetirement<V, C>>,
    _revert: EbrCellRetirement<Option<V>, C>,
    _unchanged: Option<EbrCellOwned<V, C>>,
    _blocks_release: DeferredRelease,
    _revert_release: DeferredRelease,
    _identity: IdentityRetirement,
    _probe: Option<IdentityRetirement>,
    _admission: Admission,
    _installation: Installation,
}

impl<'a, V: Value, C: Send + Sync + 'static> PreparedCellWriters<'a, V, C> {
    pub(super) fn new(
        revert: CellWriter<'a, Option<V>, C>,
        blocks: CellWriter<'a, V, C>,
        probe: Option<IdentityRetirement>,
    ) -> Self {
        Self {
            revert: Some(CellStage::Writer(revert)),
            blocks: Some(CellStage::Writer(blocks)),
            identity: None,
            probe,
            refused_identity: None,
        }
    }

    pub(super) fn prepare<E>(
        &mut self,
        target: &'a Cell<V, C>,
        predecessor: &CapturedPublication,
        dirty: bool,
    ) -> Result<(), PublicationPreparationError<E>> {
        self.revert = Some(self.revert.take().expect("original undo").prepare());
        if dirty {
            self.blocks = Some(self.blocks.take().expect("original current").prepare());
        }
        match predecessor.try_prepare_current(&target.publication) {
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
        EbrCellOwned<V, C>,
        EbrCellOwned<Option<V>, C>,
        PublicationCleanup<I>,
    ) {
        let (blocks, blocks_release) = self.blocks.take().expect("original current").abort();
        let (revert, revert_release) = self.revert.take().expect("original undo").abort();
        let identity = self
            .identity
            .take()
            .map(PreparedIdentity::abort)
            .or(self.refused_identity.take());
        (
            blocks,
            revert,
            PublicationCleanup {
                _readers: [None, None],
                writers: [Some(blocks_release), Some(revert_release)],
                identities: [self.probe.take(), identity],
                installation: Some(installation),
            },
        )
    }

    pub(super) fn publish<A, I>(
        mut self,
        next: NextPublication,
        admission: A,
        installation: I,
    ) -> PublishedPublication<V, A, I, C> {
        let blocks = self.blocks.take().expect("original current");
        let CellStage::Prepared(revert) = self.revert.take().expect("original undo") else {
            unreachable!("original undo prepared")
        };
        let identity = self.identity.take().expect("original identity prepared");
        let ((blocks, revert, unchanged, blocks_release, revert_release), identity) = identity
            .publish_retaining(
                next,
                || {
                    let (blocks, unchanged) = match blocks {
                        CellStage::Prepared(blocks) => {
                            (Some(blocks.map_preserving_release(|p| p.publish())), None)
                        }
                        CellStage::Writer(blocks) => (None, Some(blocks)),
                    };
                    (
                        blocks,
                        revert.map_preserving_release(|p| p.publish()),
                        unchanged,
                    )
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
        PublishedPublication {
            _blocks: blocks,
            _revert: revert,
            _unchanged: unchanged,
            _blocks_release: blocks_release,
            _revert_release: revert_release,
            _identity: identity,
            _probe: self.probe.take(),
            _admission: admission,
            _installation: installation,
        }
    }
}

impl<V: Value, C: Send + Sync + 'static> Drop for PreparedCellWriters<'_, V, C> {
    fn drop(&mut self) {
        let blocks = self.blocks.take().map(CellStage::abort);
        let revert = self.revert.take().map(CellStage::abort);
        let identity = self
            .identity
            .take()
            .map(PreparedIdentity::abort)
            .or(self.refused_identity.take());
        drop((blocks, revert, identity, self.probe.take()));
    }
}

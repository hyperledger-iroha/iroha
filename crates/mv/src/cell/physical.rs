//! Prepared EBR pairs retain their exact identity and defer all retirement.

use super::*;
use crate::publication::{IdentityRetirement, PreparedIdentity};
use concread::{
    ebrcell::{EbrCellCommitSlot, EbrCellRetirement},
    release::DeferredRelease,
};

enum CellStage<'a, V: Value, C: Send + Sync + 'static> {
    Held(ReleaseGuard<'a, EbrCellCommitSlot<'a, V, C>>),
    Released {
        _owner: EbrCellOwned<V, C>,
        _release: DeferredRelease,
    },
}

impl<'a, V: Value, C: Send + Sync + 'static> CellStage<'a, V, C> {
    fn new(writer: CellWriter<'a, V, C>) -> Self {
        Self::Held(writer.map_preserving_release(|writer| writer.commit_slot()))
    }

    fn prepare(&mut self) {
        let Self::Held(slot) = self else {
            panic!("original cell was released")
        };
        slot.prepare();
    }

    fn is_prepared(&self) -> bool {
        matches!(self, Self::Held(slot) if slot.is_prepared())
    }

    fn into_held(self) -> ReleaseGuard<'a, EbrCellCommitSlot<'a, V, C>> {
        match self {
            Self::Held(slot) => slot,
            Self::Released { .. } => panic!("terminal release grants no publication"),
        }
    }

    fn abort(self) -> (EbrCellOwned<V, C>, DeferredRelease) {
        self.into_held()
            .release_deferred(|slot| slot.abort().detach())
    }

    fn release(phase: &mut Option<Self>) {
        if !matches!(phase, Some(Self::Held(_))) {
            return;
        }
        let (owner, release) = phase.take().expect("original held cell").abort();
        *phase = Some(Self::Released {
            _owner: owner,
            _release: release,
        });
    }
}

pub(super) struct PreparedCellWriters<'a, V: Value, C: Send + Sync + 'static> {
    revert: Option<CellStage<'a, Option<V>, C>>,
    blocks: Option<CellStage<'a, V, C>>,
    identity: Option<PreparedIdentity<'a>>,
    probe: Option<IdentityRetirement>,
    refused_identity: Option<IdentityRetirement>,
    complete: bool,
    started: bool,
    released: bool,
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
            revert: Some(CellStage::new(revert)),
            blocks: Some(CellStage::new(blocks)),
            identity: None,
            probe,
            refused_identity: None,
            complete: false,
            started: false,
            released: false,
        }
    }

    pub(super) fn prepare<E>(
        &mut self,
        target: &'a Cell<V, C>,
        predecessor: &CapturedPublication,
        dirty: bool,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(
            !self.started && !self.released,
            "original pair prepares once"
        );
        self.started = true;
        self.revert.as_mut().expect("original undo").prepare();
        if dirty {
            self.blocks.as_mut().expect("original current").prepare();
        }
        match predecessor.try_prepare_current(&target.publication) {
            Ok(identity) => self.identity = Some(identity),
            Err((error, retirement)) => {
                self.refused_identity = retirement;
                return Err(error);
            }
        }
        self.complete = true;
        Ok(())
    }

    /// Prepare the same originals with ordinary blocking identity acquisition.
    pub(super) fn prepare_attached(
        &mut self,
        publication: &'a Publication,
        predecessor: &CapturedPublication,
        dirty: bool,
    ) {
        assert!(
            !self.started && !self.released,
            "original pair prepares once"
        );
        self.started = true;
        self.revert.as_mut().expect("original undo").prepare();
        if dirty {
            self.blocks.as_mut().expect("original current").prepare();
        }
        predecessor.prepare_current_in(publication, &mut self.identity);
        self.complete = true;
    }

    pub(super) fn is_prepared(&self) -> bool {
        self.complete && !self.released
    }

    /// Release physical ownership but keep exact cleanup in this caller-owned pair.
    /// This terminal transition grants no journal or publication authority.
    pub(super) fn release(&mut self) {
        if self.released {
            return;
        }
        CellStage::release(&mut self.blocks);
        CellStage::release(&mut self.revert);
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
        EbrCellOwned<V, C>,
        EbrCellOwned<Option<V>, C>,
        PublicationCleanup<I>,
    ) {
        assert!(!self.released, "terminal release grants no journal");
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
                _writers: [Some(blocks_release), Some(revert_release)],
                writer_batches: [None, None],
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
        let identity = self.identity.take().expect("original identity prepared");
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
                    (
                        blocks,
                        revert.map_preserving_release(|slot| slot.into_prepared().publish()),
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
                            let (owner, release) =
                                unchanged.release_deferred(|slot| slot.abort().detach());
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
        self.release();
    }
}

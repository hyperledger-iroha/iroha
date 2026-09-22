//! Complete caller-owned carrier acquisition before the first physical probe.

use super::super::super::runtime_journals::RuntimePublicationSlot;
use super::*;
use crate::state::{
    block_hashes_publication::retained_hash_slot::RetainedHashSlot,
    storage_transactions::DetachedTransactionsPublicationSlot,
    world_journals::publication::WorldPublicationSlot,
};

struct ReleasedCarrierFences {
    _state: [concread::release::DeferredRelease; 3],
    _queue: Option<ReleasedCarrierQueue>,
    _kura: KuraPublicationCleanup,
}

pub(super) struct CarrierPreparation<'target> {
    world: Option<WorldPublicationSlot<'target, (), ()>>,
    runtime: Option<RuntimePublicationSlot<'target, (), ()>>,
    transactions: Option<DetachedTransactionsPublicationSlot<'target, ()>>,
    block_hashes: RetainedHashSlot<'target, ()>,
    effect_locks: Option<crate::state::effect_publication::StateEffectLocks<'target>>,
    fences: Option<CarrierFences<'target>>,
    attempted: bool,
    retryable: bool,
    complete: bool,
    released: bool,
    retired_fences: Option<ReleasedCarrierFences>,
}

impl<'target> CarrierPreparation<'target> {
    pub(super) fn new(
        original: DetachedCarrierComponents,
        target: &'target State,
        fences: CarrierFences<'target>,
    ) -> Self {
        let DetachedCarrierComponents {
            world,
            runtime,
            transactions,
            block_hashes,
        } = original;
        Self {
            world: Some(world.publication_slot(&target.world, None)),
            runtime: Some(runtime.publication_slot(target)),
            transactions: Some(transactions.publication_slot(&target.transactions)),
            block_hashes: RetainedHashSlot::new(block_hashes, &target.block_hashes),
            effect_locks: Some(crate::state::effect_publication::StateEffectLocks::new(
                target,
            )),
            fences: Some(fences),
            attempted: false,
            retryable: true,
            complete: false,
            released: false,
            retired_fences: None,
        }
    }

    pub(super) fn try_prepare<E>(&mut self) -> Result<(), CarrierPhysicalPreparationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "carrier preparation is one-shot"
        );
        self.attempted = true;
        self.retryable = false;
        let result = self.prepare_inner();
        self.retryable = true;
        self.complete = result.is_ok();
        result
    }

    fn prepare_inner<E>(&mut self) -> Result<(), CarrierPhysicalPreparationError<E>> {
        self.block_hashes
            .try_prepare(|_, _| Ok::<_, Infallible>(()))
            .map_err(|cause| CarrierPhysicalPreparationError::Component {
                field: "block_hashes",
                cause,
            })?;
        self.transactions
            .as_mut()
            .expect("original membership slot")
            .try_prepare(|_, _| Ok::<_, Infallible>(()))
            .map_err(|cause| CarrierPhysicalPreparationError::Component {
                field: "transactions",
                cause,
            })?;
        self.runtime
            .as_mut()
            .expect("original runtime slot")
            .try_prepare(|_, _| Ok::<_, Infallible>(()))
            .map_err(CarrierPhysicalPreparationError::Runtime)?;
        self.world
            .as_mut()
            .expect("original World slot")
            .try_prepare(|_, _| Ok::<_, Infallible>(()))
            .map_err(CarrierPhysicalPreparationError::World)?;
        self.effect_locks
            .as_mut()
            .expect("original effect lock slot")
            .try_prepare()
            .map_err(|(field, wait)| CarrierPhysicalPreparationError::Fence { field, wait })?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn try_prepare_hash<E>(
        &mut self,
        admit: impl FnOnce(
            &crate::state::DetachedBlockHashes,
            &crate::state::BlockHashes,
        ) -> Result<(), E>,
    ) -> Result<(), mv::PublicationPreparationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "carrier preparation is one-shot"
        );
        self.attempted = true;
        self.retryable = false;
        let result = self.block_hashes.try_prepare(admit);
        self.retryable = true;
        result
    }

    fn release_fences(&mut self) {
        if let Some(fences) = self.fences.take() {
            let CarrierFences {
                _state: state,
                _queue: queue,
                _kura: kura,
            } = fences;
            self.retired_fences = Some(ReleasedCarrierFences {
                _state: state.release_deferred(),
                _queue: queue.map(CarrierQueueRetirement::release_deferred),
                _kura: kura.release_deferred(),
            });
        }
    }

    pub(super) fn recover_original(&mut self) -> DetachedCarrierComponents {
        assert!(
            self.retryable && !self.released,
            "unwound or released carrier grants no retry authority"
        );
        self.complete = false;
        self.released = true;
        // Normal recovery moves the original journals and retains every release
        // in the installed slots. It invokes no callback or new admission.
        let original = DetachedCarrierComponents {
            world: self
                .world
                .as_mut()
                .expect("original World slot")
                .recover_original(),
            runtime: self
                .runtime
                .as_mut()
                .expect("original runtime slot")
                .recover_original(),
            transactions: self
                .transactions
                .as_mut()
                .expect("original membership slot")
                .recover_original(),
            block_hashes: self.block_hashes.recover_original(),
        };
        self.effect_locks
            .as_mut()
            .expect("original effect locks")
            .release_writers();
        self.release_fences();
        original
    }

    pub(super) fn into_prepared(mut self) -> AcquiredCarrierComponents<'target> {
        assert!(
            self.complete && !self.released,
            "complete original carrier preparation"
        );
        self.released = true;
        AcquiredCarrierComponents {
            original: Some(AcquiredCarrierParticipants {
                world: self
                    .world
                    .take()
                    .expect("complete original World")
                    .into_prepared(),
                runtime: self
                    .runtime
                    .take()
                    .expect("complete original runtime")
                    .into_prepared(),
                transactions: self
                    .transactions
                    .take()
                    .expect("complete original membership")
                    .into_prepared(),
                block_hashes: self.block_hashes.take_prepared(),
                effect_locks: self.effect_locks.take().expect("prepared effect locks"),
                _fences: self.fences.take().expect("original carrier fences"),
            }),
        }
    }
}

impl Drop for CarrierPreparation<'_> {
    fn drop(&mut self) {
        if let Some(world) = self.world.as_mut() {
            world.release_writers();
        }
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.release_writers();
        }
        if let Some(transactions) = self.transactions.as_mut() {
            transactions.release_writers();
        }
        self.block_hashes.release_writers();
        if let Some(indexes) = self.effect_locks.as_mut() {
            indexes.release_writers();
        }
        self.release_fences();
        // Automatic payload, admission and callback destruction starts only
        // after every original participant and enclosing fence is physically free.
    }
}

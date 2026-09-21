//! Caller-owned EBR acquisition and exact retirement through enclosing aggregates.

use super::*;
use concread::{
    ebrcell::{EbrCellWriterAcquisition, EbrCellWriterAdmissionError},
    release::{DeferredRelease, DeferredReleaseBatch},
};

/// Inert original cell slot, initialized only after its aggregate owns every slot.
/// A caught initialization panic allows only release and destruction, not retry.
#[must_use = "initialize or abandon the original acquisition slot"]
pub struct BlockAcquisitionSlot<'a, V: Value, C: Send + Sync + 'static = Untracked> {
    target: &'a Cell<V, C>,
    phase: AcquisitionPhase<'a, V, C>,
    started: bool,
    complete: bool,
    // Last: original payload/charge cleanup precedes original notification.
    undo_release: DeferredReleaseBatch,
    current_release: DeferredReleaseBatch,
}

// These are mutually exclusive physical phases, not simultaneous owners. Keep
// their storage overlapped so a World-sized acquisition does not retain a second
// full block alongside its raw/partly constructed pair on the thread stack.
enum AcquisitionPhase<'a, V: Value, C: Send + Sync + 'static> {
    Empty,
    Pending(PendingPair<'a, V, C>),
    Writers(CellWriters<'a, V, C>),
    Block(Block<'a, V, C>),
}

struct PendingPair<'a, V: Value, C: Send + Sync + 'static> {
    revert: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, Option<V>, C>>>,
    blocks: Option<ReleaseGuard<'a, EbrCellWriterAcquisition<'a, V, C>>>,
    undo_value: Option<EbrCellOwned<Option<V>, C>>,
    current_value: Option<EbrCellOwned<V, C>>,
    undo_charge: Option<C>,
    current_charge: Option<C>,
}

impl<'a, V: Value, C: Send + Sync + 'static> BlockAcquisitionSlot<'a, V, C> {
    pub(super) fn new(target: &'a Cell<V, C>, charges: CellAllocationCharges<C>) -> Self {
        let CellAllocationCharges { current, undo } = charges;
        Self {
            target,
            phase: AcquisitionPhase::Pending(PendingPair {
                revert: None,
                blocks: None,
                undo_value: None,
                current_value: None,
                undo_charge: Some(undo),
                current_charge: Some(current),
            }),
            started: false,
            complete: false,
            undo_release: target.revert_released.deferred_batch(),
            current_release: target.blocks_released.deferred_batch(),
        }
    }

    fn take_writers(&mut self) -> CellWriters<'a, V, C> {
        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {
            AcquisitionPhase::Writers(writers) => writers,
            other => {
                self.phase = other;
                panic!("original initialized pair")
            }
        }
    }

    fn initialize_writers(&mut self) {
        assert!(!self.started, "original cell acquisition is one-shot");
        self.started = true;
        let target = self.target;
        let AcquisitionPhase::Pending(pending) = &mut self.phase else {
            panic!("original pending acquisition");
        };
        pending.revert = Some(
            target
                .revert_released
                .poisoning_guard(target.revert.acquire_writer()),
        );
        assert!(
            !pending
                .revert
                .as_ref()
                .expect("original undo")
                .is_poisoned(),
            "original undo writer is poisoned",
        );
        pending.blocks = Some(
            target
                .blocks_released
                .poisoning_guard(target.blocks.acquire_writer()),
        );
        assert!(
            !pending
                .blocks
                .as_ref()
                .expect("original current")
                .is_poisoned(),
            "original current writer is poisoned",
        );
        // The original slot owns both guards and both charges before either clone.
        let undo = pending.revert.take().expect("original undo");
        let undo_value = &mut pending.undo_value;
        let undo_charge = &mut pending.undo_charge;
        let result = undo
            .try_map_preserving_release_into(
                &mut self.undo_release,
                |undo| match undo.try_clone_charged(|_, _| {
                    Ok::<_, std::convert::Infallible>(
                        undo_charge.take().expect("original undo charge"),
                    )
                }) {
                    Ok((undo, value)) => {
                        *undo_value = Some(value);
                        Ok(undo)
                    }
                    Err((undo, error)) => Err((undo, error)),
                },
                || target.revert.is_poisoned(),
            )
            .unwrap_or_else(|_| unreachable!("original undo release source"));
        match result {
            Ok(undo) => pending.revert = Some(undo),
            Err((undo, error)) => {
                pending.revert = Some(undo);
                match error {
                    EbrCellWriterAdmissionError::Poisoned => {
                        panic!("original undo writer is poisoned")
                    }
                    EbrCellWriterAdmissionError::Refused(never) => match never {},
                }
            }
        }
        let current = pending.blocks.take().expect("original current");
        let current_value = &mut pending.current_value;
        let current_charge = &mut pending.current_charge;
        let result = current
            .try_map_preserving_release_into(
                &mut self.current_release,
                |current| match current.try_clone_charged(|_, _| {
                    Ok::<_, std::convert::Infallible>(
                        current_charge.take().expect("original current charge"),
                    )
                }) {
                    Ok((current, value)) => {
                        *current_value = Some(value);
                        Ok(current)
                    }
                    Err((current, error)) => Err((current, error)),
                },
                || target.blocks.is_poisoned(),
            )
            .unwrap_or_else(|_| unreachable!("original current release source"));
        match result {
            Ok(current) => pending.blocks = Some(current),
            Err((current, error)) => {
                pending.blocks = Some(current);
                match error {
                    EbrCellWriterAdmissionError::Poisoned => {
                        panic!("original current writer is poisoned")
                    }
                    EbrCellWriterAdmissionError::Refused(never) => match never {},
                }
            }
        }
        // No new lock or user operation occurs during either attachment. Keep the
        // original values in the caller slot if the native attachment refuses.
        let undo = pending.revert.take().expect("original undo");
        let undo_value = &mut pending.undo_value;
        let undo = match undo
            .try_map_preserving_release_into(
                &mut self.undo_release,
                |undo| match undo.try_write_owned(undo_value.take().expect("original undo value")) {
                    Ok(writer) => Ok(writer),
                    Err((undo, value)) => {
                        *undo_value = Some(value);
                        Err((undo, ()))
                    }
                },
                || target.revert.is_poisoned(),
            )
            .unwrap_or_else(|_| unreachable!("original undo release source"))
        {
            Ok(writer) => writer,
            Err((undo, ())) => {
                pending.revert = Some(undo);
                panic!("original healthy undo stays acquired");
            }
        };
        // Attachment cannot panic or refuse after the same exclusive healthy
        // acquisitions above; no payload code runs between these two transfers.
        let current = pending.blocks.take().expect("original current");
        let current_value = pending
            .current_value
            .take()
            .expect("original current value");
        let current = current.map_preserving_release(|current| {
            match current.try_write_owned(current_value) {
                Ok(writer) => writer,
                Err(_) => unreachable!("original healthy current stays acquired"),
            }
        });
        self.phase = AcquisitionPhase::Writers(CellWriters::new(undo, current));
    }
}

impl<'a, V: Value, C: Send + Sync + 'static> crate::BlockAcquisition
    for BlockAcquisitionSlot<'a, V, C>
{
    type Block = Block<'a, V, C>;

    fn initialize(&mut self, mode: BlockMode) {
        self.initialize_writers();
        let predecessor = self.target.publication.capture();
        let writers = self.take_writers();
        self.phase = AcquisitionPhase::Block(Block::new(
            writers,
            mode == BlockMode::Replace,
            &self.target.publication,
            predecessor,
            mode,
        ));
        let AcquisitionPhase::Block(block) = &mut self.phase else {
            unreachable!("original block");
        };
        let OriginalCellWriters { revert, blocks } = block.writers.as_mut();
        match mode {
            BlockMode::Ordinary => *revert.get_mut() = None,
            BlockMode::Replace => {
                if let Some(value) = core::mem::take(revert.get_mut()) {
                    *blocks.get_mut() = value;
                }
            }
        }
        self.complete = true;
    }

    fn release(&mut self) {
        self.complete = false;
        self.started = true;
        match &mut self.phase {
            AcquisitionPhase::Empty => {}
            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),
            AcquisitionPhase::Writers(writers) => writers.release(),
            AcquisitionPhase::Pending(pending) => {
                let target = self.target;
                if let Some(current) = pending.blocks.take() {
                    current
                        .try_release_into_observed(&mut self.current_release, drop, || {
                            target.blocks.is_poisoned()
                        })
                        .unwrap_or_else(|_| unreachable!("original current release source"));
                }
                if let Some(undo) = pending.revert.take() {
                    undo.try_release_into_observed(&mut self.undo_release, drop, || {
                        target.revert.is_poisoned()
                    })
                    .unwrap_or_else(|_| unreachable!("original undo release source"));
                }
            }
        }
    }

    fn into_block(mut self) -> Self::Block {
        assert!(
            self.complete,
            "original cell initialization did not complete"
        );
        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {
            AcquisitionPhase::Block(block) => block,
            other => {
                self.phase = other;
                panic!("original completed cell block")
            }
        }
    }
}

impl<V: Value, C: Send + Sync + 'static> Drop for BlockAcquisitionSlot<'_, V, C> {
    fn drop(&mut self) {
        crate::BlockAcquisition::release(self);
    }
}

pub(super) struct OriginalCellWriters<'a, V: Value, C: Send + Sync + 'static> {
    pub(super) revert: CellWriter<'a, Option<V>, C>,
    pub(super) blocks: CellWriter<'a, V, C>,
}

enum CellWriterState<'a, V: Value, C: Send + Sync + 'static> {
    Attached(OriginalCellWriters<'a, V, C>),
    Released {
        _undo: EbrCellOwned<Option<V>, C>,
        _current: EbrCellOwned<V, C>,
        _undo_release: DeferredRelease,
        _current_release: DeferredRelease,
    },
}

pub(super) struct CellWriters<'a, V: Value, C: Send + Sync + 'static> {
    state: Option<CellWriterState<'a, V, C>>,
}

impl<'a, V: Value, C: Send + Sync + 'static> CellWriters<'a, V, C> {
    fn new(revert: CellWriter<'a, Option<V>, C>, blocks: CellWriter<'a, V, C>) -> Self {
        Self {
            state: Some(CellWriterState::Attached(OriginalCellWriters {
                revert,
                blocks,
            })),
        }
    }

    pub(super) fn acquire(target: &'a Cell<V, C>, charges: CellAllocationCharges<C>) -> Self {
        let mut slot = BlockAcquisitionSlot::new(target, charges);
        slot.initialize_writers();
        slot.take_writers()
    }

    pub(super) fn as_ref(&self) -> &OriginalCellWriters<'a, V, C> {
        match self.state.as_ref() {
            Some(CellWriterState::Attached(original)) => original,
            _ => panic!("original cell pair was released"),
        }
    }

    pub(super) fn as_mut(&mut self) -> &mut OriginalCellWriters<'a, V, C> {
        match self.state.as_mut() {
            Some(CellWriterState::Attached(original)) => original,
            _ => panic!("original cell pair was released"),
        }
    }

    pub(super) fn into_original(mut self) -> OriginalCellWriters<'a, V, C> {
        match self.state.take() {
            Some(CellWriterState::Attached(original)) => original,
            other => {
                self.state = other;
                panic!("original cell pair was released")
            }
        }
    }

    pub(super) fn release(&mut self) {
        let Some(CellWriterState::Attached(original)) = self.state.as_ref() else {
            return;
        };
        let _ = original;
        let Some(CellWriterState::Attached(OriginalCellWriters { revert, blocks })) =
            self.state.take()
        else {
            unreachable!()
        };
        let (undo, undo_release) = revert.release_deferred(|writer| writer.detach());
        let (current, current_release) = blocks.release_deferred(|writer| writer.detach());
        self.state = Some(CellWriterState::Released {
            _undo: undo,
            _current: current,
            _undo_release: undo_release,
            _current_release: current_release,
        });
    }

    pub(super) fn detach_retaining(
        self,
    ) -> (
        EbrCellOwned<Option<V>, C>,
        EbrCellOwned<V, C>,
        crate::CaptureCleanup,
    ) {
        let OriginalCellWriters { revert, blocks } = self.into_original();
        // The capture slot checked attachment while still owning its Block.
        // Native detachment only moves these exact allocations and unlocks.
        let (revert, undo_release) = revert.release_deferred(|writer| writer.detach());
        let (blocks, current_release) = blocks.release_deferred(|writer| writer.detach());
        (
            revert,
            blocks,
            crate::CaptureCleanup::new(current_release, undo_release),
        )
    }
}

impl<V: Value, C: Send + Sync + 'static> Drop for CellWriters<'_, V, C> {
    fn drop(&mut self) {
        self.release();
    }
}

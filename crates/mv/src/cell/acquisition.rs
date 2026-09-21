//! Original EBR pair custody from raw acquisition through block abandonment.

use super::*;
use concread::ebrcell::{EbrCellWriterAcquisition, EbrCellWriterAdmissionError};

struct AcquiringWriters<'a, V: Value, C: Send + Sync + 'static> {
    revert: Option<EbrCellWriterAcquisition<'a, Option<V>, C>>,
    blocks: Option<EbrCellWriterAcquisition<'a, V, C>>,
    undo_value: Option<EbrCellOwned<Option<V>, C>>,
    current_value: Option<EbrCellOwned<V, C>>,
    undo_charge: Option<C>,
    current_charge: Option<C>,
}

impl<V: Value, C: Send + Sync + 'static> Drop for AcquiringWriters<'_, V, C> {
    fn drop(&mut self) {
        // Completed generations and unused reservations remain in this owner
        // until both original physical locks have released, including unwind.
        drop(self.blocks.take());
        drop(self.revert.take());
    }
}

pub(super) struct OriginalCellWriters<'a, V: Value, C: Send + Sync + 'static> {
    pub(super) revert: CellWriter<'a, Option<V>, C>,
    pub(super) blocks: CellWriter<'a, V, C>,
}

pub(super) struct CellWriters<'a, V: Value, C: Send + Sync + 'static> {
    original: Option<OriginalCellWriters<'a, V, C>>,
    target: &'a Cell<V, C>,
}

impl<'a, V: Value, C: Send + Sync + 'static> CellWriters<'a, V, C> {
    pub(super) fn acquire(target: &'a Cell<V, C>, charges: CellAllocationCharges<C>) -> Self {
        let revert = target
            .revert_released
            .poisoning_guard(target.revert.acquire_writer());
        // A terminal undo failure must not wait for an unrelated current lock.
        // The two unused original charges remain outside the raw guard's drop.
        if revert.is_poisoned() {
            revert.release_with_observed_poison(drop, || target.revert.is_poisoned());
            panic!("original undo writer is poisoned");
        }
        let blocks = target
            .blocks_released
            .poisoning_guard(target.blocks.acquire_writer());
        let (revert, blocks) = revert
            .try_map_pair_preserving_release(
                blocks,
                |revert, blocks| {
                    let CellAllocationCharges { current, undo } = charges;
                    let mut pending = AcquiringWriters {
                        revert: Some(revert),
                        blocks: Some(blocks),
                        undo_value: None,
                        current_value: None,
                        undo_charge: Some(undo),
                        current_charge: Some(current),
                    };
                    assert!(
                        !pending
                            .blocks
                            .as_ref()
                            .expect("original current")
                            .is_poisoned(),
                        "original current writer is poisoned",
                    );
                    let undo = pending.revert.take().expect("original undo");
                    let (undo, value) = match undo.try_clone_charged(|_, _| {
                        Ok::<_, std::convert::Infallible>(
                            pending.undo_charge.take().expect("original undo charge"),
                        )
                    }) {
                        Ok(owners) => owners,
                        Err((original, EbrCellWriterAdmissionError::Poisoned)) => {
                            pending.revert = Some(original);
                            panic!("original undo writer is poisoned");
                        }
                        Err((_, EbrCellWriterAdmissionError::Refused(never))) => match never {},
                    };
                    pending.revert = Some(undo);
                    pending.undo_value = Some(value);
                    let current = pending.blocks.take().expect("original current");
                    let (current, value) = match current.try_clone_charged(|_, _| {
                        Ok::<_, std::convert::Infallible>(
                            pending
                                .current_charge
                                .take()
                                .expect("original current charge"),
                        )
                    }) {
                        Ok(owners) => owners,
                        Err((original, EbrCellWriterAdmissionError::Poisoned)) => {
                            pending.blocks = Some(original);
                            panic!("original current writer is poisoned");
                        }
                        Err((_, EbrCellWriterAdmissionError::Refused(never))) => match never {},
                    };
                    pending.blocks = Some(current);
                    pending.current_value = Some(value);
                    // These same guards excluded every intervening writer, so
                    // attachment cannot newly fail after successful cloning.
                    let undo = pending.revert.take().expect("original undo");
                    let undo_value = pending.undo_value.take().expect("original undo value");
                    let undo = match undo.try_write_owned(undo_value) {
                        Ok(writer) => writer,
                        Err(_) => unreachable!("original healthy undo stays acquired"),
                    };
                    let current = pending.blocks.take().expect("original current");
                    let current_value = pending
                        .current_value
                        .take()
                        .expect("original current value");
                    let current = match current.try_write_owned(current_value) {
                        Ok(writer) => writer,
                        Err(_) => unreachable!("original healthy current stays acquired"),
                    };
                    Ok::<_, std::convert::Infallible>((undo, current))
                },
                || (target.revert.is_poisoned(), target.blocks.is_poisoned()),
            )
            .unwrap_or_else(|never| match never {});
        Self {
            original: Some(OriginalCellWriters { revert, blocks }),
            target,
        }
    }

    pub(super) fn as_ref(&self) -> &OriginalCellWriters<'a, V, C> {
        self.original.as_ref().expect("original cell pair")
    }

    pub(super) fn as_mut(&mut self) -> &mut OriginalCellWriters<'a, V, C> {
        self.original.as_mut().expect("original cell pair")
    }

    pub(super) fn into_original(mut self) -> OriginalCellWriters<'a, V, C> {
        self.original.take().expect("original cell pair")
    }

    pub(super) fn detach(mut self) -> (EbrCellOwned<Option<V>, C>, EbrCellOwned<V, C>) {
        let OriginalCellWriters { revert, blocks } =
            self.original.take().expect("original cell pair");
        revert.release_pair_with(
            blocks,
            |revert, blocks| {
                let revert = revert.detach();
                let blocks = blocks.detach();
                (revert, blocks)
            },
            || {
                (
                    self.target.revert.is_poisoned(),
                    self.target.blocks.is_poisoned(),
                )
            },
        )
    }
}

impl<V: Value, C: Send + Sync + 'static> Drop for CellWriters<'_, V, C> {
    fn drop(&mut self) {
        if let Some(OriginalCellWriters { revert, blocks }) = self.original.take() {
            revert.release_pair_with(
                blocks,
                |revert, blocks| {
                    let revert = revert.detach();
                    let blocks = blocks.detach();
                    drop((revert, blocks));
                },
                || {
                    (
                        self.target.revert.is_poisoned(),
                        self.target.blocks.is_poisoned(),
                    )
                },
            );
        }
    }
}

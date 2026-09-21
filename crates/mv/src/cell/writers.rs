//! One original executing EBR pair from completed acquisition to consumption.
//!
//! Both physical writers release before either native notification runs. This
//! does not defer allocation refunds or enclose other State participants, and it
//! begins only after both original acquisitions have succeeded.

use super::*;

pub(super) struct OriginalCellWriters<'target, V: Value, Charge: Send + Sync + 'static> {
    pub(super) revert: CellWriter<'target, Option<V>, Charge>,
    pub(super) blocks: CellWriter<'target, V, Charge>,
}

pub(super) struct CellWriters<'target, V: Value, Charge: Send + Sync + 'static> {
    original: Option<OriginalCellWriters<'target, V, Charge>>,
    target: &'target Cell<V, Charge>,
}

impl<'target, V: Value, Charge: Send + Sync + 'static> CellWriters<'target, V, Charge> {
    pub(super) fn new(
        target: &'target Cell<V, Charge>,
        revert: CellWriter<'target, Option<V>, Charge>,
        blocks: CellWriter<'target, V, Charge>,
    ) -> Self {
        Self {
            original: Some(OriginalCellWriters { revert, blocks }),
            target,
        }
    }

    pub(super) fn target(&self) -> &'target Cell<V, Charge> {
        self.target
    }

    pub(super) fn as_ref(&self) -> &OriginalCellWriters<'target, V, Charge> {
        self.original.as_ref().expect("original cell pair")
    }

    pub(super) fn as_mut(&mut self) -> &mut OriginalCellWriters<'target, V, Charge> {
        self.original.as_mut().expect("original cell pair")
    }

    pub(super) fn into_original(mut self) -> OriginalCellWriters<'target, V, Charge> {
        self.original.take().expect("original cell pair")
    }

    /// Move both exact generations out before either release notification runs.
    pub(super) fn detach(self) -> (EbrCellOwned<V, Charge>, EbrCellOwned<Option<V>, Charge>) {
        let target = self.target;
        let OriginalCellWriters { revert, blocks } = self.into_original();
        blocks.release_pair_with(
            revert,
            |blocks, revert| (blocks.detach(), revert.detach()),
            || (target.blocks.is_poisoned(), target.revert.is_poisoned()),
        )
    }
}

impl<V: Value, Charge: Send + Sync + 'static> Drop for CellWriters<'_, V, Charge> {
    fn drop(&mut self) {
        if let Some(OriginalCellWriters { revert, blocks }) = self.original.take() {
            revert.release_pair_with(
                blocks,
                |revert, blocks| drop((revert, blocks)),
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

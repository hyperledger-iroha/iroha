//! Keep the original World, runtime and membership captures in one owner.

use super::runtime_journals::RuntimeCapture;
use super::*;
use crate::state::world_journals::WorldJournalCapture;

#[derive(Debug, thiserror::Error)]
pub(super) enum StateCaptureError {
    #[error(transparent)]
    World(#[from] world_journals::CaptureError<std::convert::Infallible>),
    #[error(transparent)]
    Membership(#[from] storage_transactions::TransactionsBlockError),
}

/// Original slots, with no duplicated World inventory or additional allocation.
pub(super) struct StateJournalCapture<'state, WorldCapture: WorldJournalCapture> {
    world: Option<WorldCapture>,
    runtime: Option<RuntimeCapture<'state, ()>>,
    transactions: Option<storage_transactions::TransactionsCaptureSlot<'state>>,
    block_hashes: Option<BlockHashesBlock<'state>>,
    complete: bool,
}

impl<'state, WorldCapture: WorldJournalCapture> StateJournalCapture<'state, WorldCapture> {
    /// Install every original before calling any capture operation.
    pub(super) fn new(
        world: WorldCapture,
        runtime: RuntimeCapture<'state, ()>,
        transactions: storage_transactions::TransactionsCaptureSlot<'state>,
        block_hashes: BlockHashesBlock<'state>,
    ) -> Self {
        Self {
            world: Some(world),
            runtime: Some(runtime),
            transactions: Some(transactions),
            block_hashes: Some(block_hashes),
            complete: false,
        }
    }

    /// A refusal or unwind leaves every partial original in this caller owner.
    pub(super) fn try_capture(&mut self) -> Result<(), StateCaptureError> {
        self.world
            .as_mut()
            .expect("original World capture")
            .capture()?;
        match self
            .runtime
            .as_mut()
            .expect("original runtime capture")
            .try_capture(|_| Ok::<(), std::convert::Infallible>(()))
        {
            Ok(()) => {}
            Err(impossible) => match impossible {},
        }
        self.transactions
            .as_mut()
            .expect("original membership capture")
            .try_capture()?;
        self.complete = true;
        Ok(())
    }

    fn release(&mut self) {
        self.complete = false;
        if let Some(world) = self.world.as_mut() {
            world.release();
        }
        if let Some(runtime) = self.runtime.as_mut() {
            runtime.release();
        }
        if let Some(transactions) = self.transactions.as_mut() {
            transactions.release();
        }
    }

    /// All physical writers are free before the first original notification.
    pub(super) fn into_components(mut self) -> DetachedCarrierComponents {
        assert!(self.complete, "original State capture did not complete");
        let (transactions, cleanup) = self
            .transactions
            .take()
            .expect("original membership capture")
            .into_detached();
        let components = DetachedCarrierComponents {
            world: self
                .world
                .take()
                .expect("original World capture")
                .into_journals(()),
            runtime: self
                .runtime
                .take()
                .expect("original runtime capture")
                .into_journals(),
            transactions,
            block_hashes: self
                .block_hashes
                .take()
                .expect("original block hashes")
                .detach(),
        };
        drop(cleanup);
        components
    }
}

impl<WorldCapture: WorldJournalCapture> Drop for StateJournalCapture<'_, WorldCapture> {
    fn drop(&mut self) {
        self.release();
    }
}

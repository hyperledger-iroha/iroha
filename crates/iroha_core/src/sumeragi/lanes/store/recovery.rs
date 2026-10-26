//! A startup owner retains its lock, its tip authentication and every original recovery allocation.

use super::*;
use crate::execution_attempt::ExecutionAttemptError as Attempt;

/// Exclusive initialization in progress; no not-ready store or unvalidated tip is exposed.
///
/// Opening authenticates only the durable tip frame. Every lower frame carries its own CommitQC
/// and availability evidence, and the read that first uses it authenticates it, so opening a
/// store costs one frame instead of its whole history.
pub struct LaneStoreOpen {
    pub(super) store: FileLaneBlockStore,
    target: u64,
    pub(super) tip_authenticated: bool,
}
impl LaneStoreOpen {
    pub(super) fn new(store: FileLaneBlockStore, target: u64) -> Self {
        Self {
            store,
            target,
            tip_authenticated: target == 0,
        }
    }
    /// Complete recovery, returning this exact owner on every refusal or error.
    /// A caller may retry `WouldBlock` after resources/authority become available; semantic or I/O
    /// failures remain errors. No retry spins or alternate proof/source replacement occur here.
    ///
    /// # Errors
    /// Full certificate/availability validation of the tip, original-pool resource refusal or
    /// durability.
    #[allow(
        clippy::result_large_err,
        reason = "return original startup ownership without allocating"
    )]
    pub fn complete(mut self) -> Result<FileLaneBlockStore, (Self, Attempt<io::Error>)> {
        let budget = self.store.budget.clone();
        // Borrow the original startup owner through cleanup; the scope returns only the
        // small attempt result before this function moves the complete owner once.
        let outcome = FileLaneBlockStore::with_original_refunds(&budget, || {
            self.progress()?;
            *self.store.state.lock() = StoreState {
                read: None,
                write: None,
            };
            *self.store.tip.lock() = self.target;
            Ok(())
        });
        match outcome {
            Ok(()) => Ok(self.store),
            Err(error) => Err((self, error)),
        }
    }
    fn progress(&mut self) -> Result<(), Attempt<io::Error>> {
        self.store.revalidate_custody()?;
        if !self.tip_authenticated {
            let mut state = self.store.state.lock();
            let prepared = self.store.read_prepared(
                &mut state,
                self.target,
                &super::ReadInterest::Historical,
            )?;
            let bytes = prepared.prepare(&self.store.budget).map_err(record_error)?;
            // Synchronizes the tip frame and its directory entry. Each lower frame finished
            // its own synchronized publication before its successor could be appended.
            self.store.publish_frame(self.target, bytes)?;
            state.read = None;
            self.tip_authenticated = true;
        }
        self.store.revalidate_custody()?;
        Ok(())
    }
}

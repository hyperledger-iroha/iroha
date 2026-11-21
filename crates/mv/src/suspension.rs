//! Distinct original execution suspension; immutable publication capture never regains writes.

use crate::{BlockRetirement, CaptureCleanup, PublicationPreparationError};

/// Inert aggregate slot releasing the exact original execution writers.
/// Install every sibling slot before suspending any; retain all cleanup until every
/// physical writer and enclosing fence is free. No callback or allocation is needed.
pub trait BlockSuspension: BlockRetirement + Sized {
    /// The exact execution block consumed by this slot.
    type Block: BlockRetirement;
    /// Original private generations and all mutable-execution metadata, without writers.
    type Suspended;
    /// Suspend once, retaining original payloads and actual release notifications inline.
    /// An unwind grants terminal abandonment only, never resumed execution.
    fn suspend(&mut self);
    /// Move completed original custody and deferred cleanup out of this inert slot.
    fn into_suspended(self) -> (Self::Suspended, CaptureCleanup);
}

/// Reacquire a suspension-created execution owner, never an ordinary frozen journal.
/// Install all siblings before trying any acquisition, and complete all acquisitions
/// before taking any Block. Keep emptied slots until every resumed writer is released:
/// they retain actual reader/identity release notices from validation.
pub trait BlockResumption: BlockRetirement {
    /// Exact original mutable execution block after original predecessor validation.
    type Block: BlockRetirement;
    /// Original suspended custody, returned unchanged after normal refusal or abort.
    type Suspended;
    /// Try one nonblocking acquisition of the original writers and predecessor.
    /// A local refusal remains the actual physical owner error, never a protocol verdict.
    fn try_resume(&mut self) -> Result<(), PublicationPreparationError<core::convert::Infallible>>;
    /// Whether all original writers and predecessor checks completed in this slot.
    fn is_resumed(&self) -> bool;
    /// Transfer completed execution authority while retaining deferred cleanup in this slot.
    /// Call only after every sibling in the enclosing aggregate is ready.
    fn take_block(&mut self) -> Self::Block;
    /// Release acquired writers and recover the original suspension after normal failure
    /// or complete abort. Cleanup stays here until every sibling and fence is released.
    fn recover_suspended(&mut self) -> Self::Suspended;
}

#[cfg(test)]
mod tests;

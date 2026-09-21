//! Multi-version storage and cells used by Iroha's production State owners.
//!
//! Current values, block undo preimages and retained readers share the original
//! Concread generations. Explicit prepaid storage admits node, writer and copied
//! payload and publication identity custody through a finite allocation pool. This
//! is not complete World admission: native mutex/release storage and the remaining mutation
//! families require their own allocation owners. Borrowed ordered scans retain
//! traversal state inline without allocating.
use core::fmt::Debug;
/// Finite prepaid custody for explicitly enumerated allocation layouts.
pub mod allocation;
mod publication;
use concread::release::{ReleaseGuard, ReleaseNotification, ReleaseWait};
pub use publication::{
    BlockPublicationIdentity, PublicationCleanup, PublicationPreparationError,
    PublicationPreparationResult,
};
#[cfg(test)]
#[path = "release_tests.rs"]
mod release_tests;

/// How a block acquired its exact published predecessor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockMode {
    /// Extend current state, replacing the prior block's undo with this block's undo.
    Ordinary,
    /// First undo the published tip, then stage a replacement at that cut.
    Replace,
}
/// Caller-owned construction of one original block's physical owners.
///
/// An aggregate creates every slot before initialization. On success or unwind
/// it releases every slot before allowing any slot's payload or notification to
/// drop. Initialization is one-shot; a caught panic permits abandonment only.
pub trait BlockAcquisition: Sized {
    /// Fully initialized block using the same original physical owners.
    type Block: BlockRetirement;
    /// Acquire and initialize this slot, retaining partial custody on unwind.
    fn initialize(&mut self, mode: BlockMode);
    /// Unlock all physical owners in place, retaining cleanup until slot drop.
    fn release(&mut self);
    /// Transfer a successfully initialized slot without cloning or allocation.
    fn into_block(self) -> Self::Block;
}

/// Terminal abandonment of a completed original block inside an aggregate.
pub trait BlockRetirement {
    /// Unlock in place, retaining every original private owner and notification.
    /// This is idempotent. The block cannot be read, edited, committed or detached
    /// afterward; only its eventual destruction is permitted.
    fn release_writers(&mut self);
}

/// MVCC cell primitives (versioned slots and helpers).
pub mod cell;
/// Norito JSON helpers for MV types.
pub mod json;
/// Simple MV storage backend abstractions.
pub mod storage;
/// Marker trait for keys stored in MV containers.
///
/// Keys must be totally ordered to resolve version order and implement common
/// concurrency-safe bounds.
pub trait Key: Clone + Ord + Debug + Send + Sync + 'static {}
/// Marker trait for values stored in MV containers.
pub trait Value: Clone + Send + Sync + 'static {}
impl<T: Clone + Ord + Debug + Send + Sync + 'static> Key for T {}
impl<T: Clone + Send + Sync + 'static> Value for T {}

//! Multi-version storage for deterministic State execution and retained publication.
//!
//! Cells and ordered maps preserve their original current and undo generations
//! through private edits, rollback, detached publication and retained readers.
//! Explicit prepaid modes attach layout credits to their actual allocation owners;
//! callers must separately admit nested payloads and aggregate execution work.
use core::fmt::Debug;
/// Finite prepaid custody for explicitly enumerated allocation layouts.
pub mod allocation;
mod publication;
pub use publication::{
    BlockPublicationIdentity, PublicationPreparationError, PublicationPreparationResult,
};
mod release;
pub use release::{ReleaseFuture, ReleaseGuard, ReleaseNotification, ReleaseWait};

/// How a block acquired its exact published predecessor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockMode {
    /// Extend current state, replacing the prior block's undo with this block's undo.
    Ordinary,
    /// First undo the published tip, then stage a replacement at that cut.
    Replace,
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

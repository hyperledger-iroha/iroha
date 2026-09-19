//! Minimal MV (multi-version) storage traits used in tests and lightweight
//! components. This crate defines the `Key` and `Value` marker traits and
//! exposes simple cell and storage modules for multi-version concurrency.
//!
//! The abstractions here are intentionally small to avoid pulling heavy dependencies. They are
//! suitable for in-memory testing or thin adapters in higher-level crates.
use core::fmt::Debug;
/// Finite prepaid custody for explicitly enumerated allocation layouts.
pub mod allocation;
mod publication;
pub use publication::{PublicationPreparationError, PublicationPreparationResult};
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

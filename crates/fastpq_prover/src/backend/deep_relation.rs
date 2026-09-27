//! Closed immutable bridge from prepared public relations to the DEEP arithmetic.
//!
//! Transcript binding consumes the outer FixedAir identity and exact statement;
//! polynomial/AIR arithmetic borrows its original complete CompactTransferAir.
//! The three implementations are the raw SMT owner and the two typed batch
//! segment wrappers. Public callers cannot replace either side of this pair.

use super::{compact_protocol::FixedAir, compact_transfer_air::CompactTransferAir};

// Only the backend's three prepared relation owners implement this marker.
pub(super) mod sealed {
    /// Marker restricted to the backend's prepared relation implementations.
    pub trait Sealed {}
}

/// Immutable prepared statement and its matching full reference AIR.
pub(super) trait DeepRelation: FixedAir + sealed::Sealed {
    /// Borrow the original AIR, preserving the wrapper's complete statement bytes.
    fn deep_relation(&self) -> &CompactTransferAir;
}

impl sealed::Sealed for CompactTransferAir {}

impl DeepRelation for CompactTransferAir {
    fn deep_relation(&self) -> &CompactTransferAir {
        self
    }
}

#[cfg(test)]
#[path = "deep_relation/tests.rs"]
pub(super) mod tests;

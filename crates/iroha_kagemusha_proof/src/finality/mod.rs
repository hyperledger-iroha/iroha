//! Circuit components for ordinary validator-finalized Load receipts.
//!
//! Receipt parsing and inclusion are only parts of the finality relation. They
//! grant no Load authority without a source-qualified proof of the genesis-rooted
//! validator schedule, exact quorum certificate and certified event commitment.
//!
//! TODO: compose and qualify that complete recursive relation and replace the
//! issuer-based Load source in the immutable operation catalog.

mod receipt;
pub use receipt::LoadReceiptCells;

mod event;
pub use event::LoadEventCells;

pub mod result;

pub mod consensus;

pub mod roster;

pub mod aggregate;

pub mod continuity;

pub mod schedule;

pub mod bls;

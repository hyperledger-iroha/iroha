//! Deterministic transitions for the native stream-token gateway authority.
//!
//! Permanent admission and replay rows remain separate from bounded live quota and lease
//! indexes. All fallible reads and calculations produce one compare-and-swap delta before the
//! caller may change authoritative World state. Request times never choose quota window clocks.
//!
//! The governed native instruction applies these deltas atomically to protected World rows.
//! Challenged opaque readback authenticates exact native history and current serving eligibility.
//! TODO: wire this owner into the production daemon gateway provider and capture boundary.
//! A successful native mutation is not proof of finalized or currently eligible serving.

pub(crate) mod check;
pub(crate) mod commitment;
/// One-use challenged native gateway execution and current readback.
pub mod observation;
pub(crate) mod read;
pub(crate) mod rows;
pub(crate) mod storage;
pub(crate) mod transition;

#[cfg(test)]
mod tests;

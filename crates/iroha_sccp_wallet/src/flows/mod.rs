//! Resumable, journaled SCCP wallet flows (spec §7).
//!
//! Each flow records its raw evidence and state transitions in the operation
//! journal, keyed by Taira `NetworkId`, before it submits anything, so a
//! crashed flow resumes where it stopped.

pub mod inbound;
pub mod outbound;
pub mod refund;

// TODO(ws51): define the shared flow driver, resume logic and step records.

//! Canonical KAGEMUSHA V1 wire and authenticated release data model.
//!
//! KAGEMUSHA V1 is the sole public hardware-backed cash protocol. Its public model
//! contains only aggregate-balance state, hardware-bound proofs, and pooled
//! reserve settlement.

pub mod kagemusha_device_v1;
pub mod kagemusha_release_v1;
pub mod kagemusha_v1;

pub use self::{kagemusha_device_v1::*, kagemusha_release_v1::*, kagemusha_v1::*};

/// Prefix embedded into KAGEMUSHA V1 instruction rejection messages.
///
/// Torii extracts the label following this prefix as a stable machine-readable
/// error code.
pub const KAGEMUSHA_V1_REJECTION_REASON_PREFIX: &str = "kagemusha_v1_reason::";

//! Canonical Offline Cash V1 wire and authenticated release data model.
//!
//! Offline Cash V1 is the only public offline-money protocol. Its public model
//! contains only aggregate-balance state, hardware-bound proofs, and pooled
//! reserve settlement.

pub mod offline_cash_release_v1;
pub mod offline_cash_v1;

pub use self::{offline_cash_release_v1::*, offline_cash_v1::*};

/// Prefix embedded into Offline Cash V1 instruction rejection messages.
///
/// Torii extracts the label following this prefix as a stable machine-readable
/// error code.
pub const OFFLINE_CASH_V1_REJECTION_REASON_PREFIX: &str = "offline_cash_v1_reason::";

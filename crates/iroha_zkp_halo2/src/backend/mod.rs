//! Backend abstractions for IPA polynomial commitments.
//!
//! This module defines the traits that each backend must implement and exposes
//! concrete backends (Pallas and BN254).
pub mod bn254;
pub mod pallas;
pub mod traits;
pub use traits::{IpaBackend, IpaGroup, IpaScalar, product};

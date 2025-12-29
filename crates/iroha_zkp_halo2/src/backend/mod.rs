//! Backend abstractions for IPA polynomial commitments.
//!
//! This module defines the traits that each backend must implement and exposes
//! concrete backends (Pallas, Goldilocks).

pub mod bn254;
#[cfg(feature = "goldilocks_backend")]
pub mod goldilocks;
pub mod pallas;
pub mod traits;

pub use traits::{IpaBackend, IpaGroup, IpaScalar, product};

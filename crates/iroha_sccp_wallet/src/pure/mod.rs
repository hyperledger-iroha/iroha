//! Network-free SCCP wallet primitives (spec §7, §8).
//!
//! Everything here is deterministic and free of I/O, so SDK bridges can
//! export it over FFI: attestation bundle and roster rotation verification,
//! Parliament control checks and per-chain destination encodings and signing.

pub mod bundle;
pub mod control;
pub mod evm;
pub mod rotation;
pub mod ton;
pub mod tron;

// TODO(ws22): define the FFI-stable entry points shared by the SDK bridges.

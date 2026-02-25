//! FASTPQ Instruction Set - canonical STARK parameter descriptors.
//!
//! This crate publishes the canonical FASTPQ lane prover/verifier
//! parameters.  The constants exported here are referenced by design
//! documents and eventually by the prover implementation when the
//! execution engine lands.  They are kept in a dedicated crate so
//! workspace members can depend on a single source of truth without
//! duplicating values in code or documentation.
//!
//! The parameter table mirrors the one rendered in `nexus.md`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod params;
pub mod poseidon;

/// Canonical recursive merge circuit identifier for offline multi-allowance bundles.
pub const OFFLINE_MERGE_CIRCUIT_ID: &str = "offline-merge-v1";
/// Canonical transparent backend used by offline recursive merge proofs.
pub const OFFLINE_MERGE_BACKEND: &str = "stark/fri-v1/poseidon2-goldilocks-v1";
/// Default maximum number of certificates merged into a single offline proof bundle.
pub const OFFLINE_MERGE_MAX_CERTIFICATES: usize = 16;

pub use params::{
    CANONICAL_PARAMETER_SETS, FieldDescriptor, HashDescriptor, StarkParameterSet, find_by_name,
};

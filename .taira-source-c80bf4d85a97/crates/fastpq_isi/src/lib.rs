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

pub use params::{
    CANONICAL_PARAMETER_SETS, FieldDescriptor, HashDescriptor, StarkParameterSet, find_by_name,
};

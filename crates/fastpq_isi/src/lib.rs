//! FASTPQ Instruction Set - canonical STARK parameter descriptors.
//!
//! This crate publishes the canonical FASTPQ lane prover/verifier parameter
//! set. The constants exported here are used by the prover implementation and
//! kept in a dedicated crate so workspace members share one source of truth.
//!
//! The parameter descriptor mirrors the implementation-coupled specification.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
pub mod params;
pub mod poseidon;
pub mod poseidon_digest384;
pub use params::{CANONICAL_PARAMETER_SETS, StarkParameterSet, find_by_name};
pub use poseidon_digest384::{
    GOLDILOCKS_DIGEST384_BYTES_V1, GOLDILOCKS_DIGEST384_LANES_V1,
    GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1, GoldilocksDigest384V1, GoldilocksDigestDomainV1,
    hash_bytes_384_v1,
};

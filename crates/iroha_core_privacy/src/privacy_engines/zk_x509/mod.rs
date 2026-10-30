//! Native transparent proof engine for the closed Iroha X.509 profile.
//!
//! This module is intentionally not an adapter for the unpublished zk-X509
//! prototype.  The first release is an original, versioned Iroha relation:
//! strict DER, RFC 5280 path processing within a closed P-256/SHA-256 profile,
//! private certificate and CRL witnesses, fixed SHA-256 Merkle accumulators,
//! and a purpose-built Goldilocks STARK with opaque SHA3-384 outer commitments.
//! The SHA-256 compiled manifest binds the exact SHA3-384 schedule bytes; its
//! independent pin and deterministic proof vectors describe this sole protocol.
//! No compatibility or fallback proof format is accepted.
//!
//! [`profile`] fixes the intended relation and AIR resource envelope, while [`merkle`] implements
//! accumulator semantics shared by the native reference relation and constrained execution.
//! Consensus use remains unsupported until the concrete joint round-by-round/Fiat–Shamir
//! security reduction is complete. Reviewer and resource receipts never authorize validity;
//! no projection-only or unbound collection of subproofs is treated as a credential proof.
pub(crate) mod accumulator_air;
pub(crate) mod accumulator_stark;
pub(crate) mod air;
pub(crate) mod allocation_payload;
mod certificate;
pub(crate) mod codec;
mod composition_masking;
mod credential_joint;
pub(crate) mod credential_pre_aux;
#[doc(hidden)]
pub mod credential_stark;
pub(crate) mod der;
pub(crate) mod der_air;
pub(crate) mod der_limits;
pub(crate) mod der_stark;
#[doc(hidden)]
pub mod engine;
/// Typed failures of strict X.509 parsing and reference validation.
pub mod errors;
pub(crate) mod fixed_algebraic;
pub(crate) mod fixed_algebraic_p256;
pub(crate) mod fixed_algebraic_sha;
/// Holder-owned private input and ordinary native credential proof construction.
pub mod holder;
pub(crate) mod io_air;
pub(crate) mod main_assembly;
pub(crate) mod main_io;
pub(crate) mod merkle;
pub(crate) mod p256_aggregate_adapter;
pub(crate) mod p256_air;
pub(crate) mod p256_cross_trace_bus;
pub(crate) mod p256_ecdsa_air;
pub(crate) mod p256_external_binding_air;
pub(crate) mod p256_group_air;
pub(crate) mod p256_reduction_air;
pub(crate) mod p256_scalar_bit_bus;
pub(crate) mod p256_trace;
pub(crate) mod p256_value_bus;
pub(crate) mod p256_window_air;
/// Ordinary holder-side bounds derived from exact signed DER intervals.
pub mod presentation;
mod private_table;
#[doc(hidden)]
pub mod profile;
pub(crate) mod projection_air;
mod proof_instance;
#[cfg(test)]
mod prover_observation;
#[doc(hidden)]
pub mod relation;
pub(crate) mod rfc5280_stark;
#[cfg(test)]
pub(crate) mod sha256_air;
pub(crate) mod sha256_word_air;
pub(crate) mod sha_call_bus_stark;
pub(crate) mod sha_word_stark;
pub(crate) mod stark;
pub(crate) mod verifier_profile;

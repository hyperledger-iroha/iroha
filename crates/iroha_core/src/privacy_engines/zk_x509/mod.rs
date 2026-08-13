//! Native transparent proof engine for the closed Iroha X.509 profile.
//!
//! This module is intentionally not an adapter for the unpublished zk-X509
//! prototype.  The first release is an original, versioned Iroha relation:
//! strict DER, RFC 5280 path processing within a closed P-256/SHA-256 profile,
//! private certificate and CRL witnesses, fixed SHA-256 Merkle accumulators,
//! and a purpose-built Goldilocks STARK.  No compatibility or fallback proof
//! format is accepted.
//!
//! [`profile`] fixes the intended relation and AIR resource envelope, while
//! [`merkle`] implements accumulator semantics shared by the native reference
//! relation and constrained execution. Consensus activation is controlled by
//! the frozen soundness, resource, and governance release gates; no
//! projection-only or unbound collection of subproofs is treated as a
//! credential proof.
pub(crate) mod accumulator_air;
pub(crate) mod accumulator_stark;
pub(crate) mod air;
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) mod codec;
pub(crate) mod credential_pre_aux;
pub(crate) mod credential_stark;
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) mod der;
pub(crate) mod der_air;
pub(crate) mod der_limits;
pub(crate) mod der_stark;
pub(crate) mod engine;
pub(crate) mod fixed_algebraic;
pub(crate) mod fixed_algebraic_p256;
pub(crate) mod fixed_algebraic_sha;
pub(crate) mod io_air;
#[cfg(any(test, feature = "privacy-release-evidence"))]
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
pub(crate) mod profile;
pub(crate) mod projection_air;
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) mod relation;
pub(crate) mod rfc5280_stark;
#[cfg(test)]
pub(crate) mod sha256_air;
pub(crate) mod sha256_word_air;
pub(crate) mod sha_call_bus_stark;
pub(crate) mod sha_word_stark;
pub(crate) mod stark;
pub(crate) mod verifier_profile;

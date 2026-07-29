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
//! relation and constrained execution. The implementation remains fail-closed
//! until [`air::ZK_X509_AIR_GAPS_V1`] is empty; no projection-only or unbound
//! collection of subproofs is treated as a credential proof.

pub(crate) mod accumulator_air;
pub(crate) mod accumulator_stark;
pub(crate) mod air;
pub(crate) mod codec;
pub(crate) mod credential_pre_aux;
pub(crate) mod credential_stark;
pub(crate) mod der;
pub(crate) mod der_air;
pub(crate) mod der_stark;
pub(crate) mod engine;
pub(crate) mod io_air;
pub(crate) mod main_assembly;
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
pub(crate) mod preprocessed_fixed;
pub(crate) mod profile;
pub(crate) mod projection_air;
pub(crate) mod relation;
pub(crate) mod rfc5280_stark;
pub(crate) mod sha256_air;
pub(crate) mod sha256_word_air;
pub(crate) mod sha_call_bus_stark;
pub(crate) mod sha_word_stark;
pub(crate) mod stark;

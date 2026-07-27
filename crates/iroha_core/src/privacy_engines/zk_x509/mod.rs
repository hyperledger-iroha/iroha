//! Native transparent proof engine for the closed Iroha X.509 profile.
//!
//! This module is intentionally not an adapter for the unpublished zk-X509
//! prototype.  The first release is an original, versioned Iroha relation:
//! strict DER, RFC 5280 path processing within a closed P-256/SHA-256 profile,
//! private certificate and CRL witnesses, fixed SHA-256 Merkle accumulators,
//! and a purpose-built Goldilocks STARK.  No compatibility or fallback proof
//! format is accepted.
//!
//! The implementation is staged fail-closed.  [`profile`] fixes the complete
//! relation and AIR resource envelope, while [`merkle`] implements accumulator
//! semantics shared by the native reference relation and the future AIR
//! gadgets.  Consensus activation must remain unavailable until
//! [`profile::ZK_X509_ENGINE_ACTIVATION_READY_V1`] is true and all named
//! readiness requirements validate.

pub(crate) mod merkle;
pub(crate) mod profile;
